use crate::fs::asyncify;
use crate::io::blocking::{Buf, DEFAULT_MAX_BUF_SIZE};
use crate::io::{AsyncRead, AsyncSeek, AsyncWrite, ReadBuf};
use crate::sync::Mutex;

use std::cmp;
use std::fmt;
use std::fs::{Metadata, Permissions};
use std::future::Future;
use std::io::{self, Seek, SeekFrom};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{ready, Context, Poll};

#[cfg(test)]
use super::super::mocks::JoinHandle;
#[cfg(test)]
use super::super::mocks::MockFile as StdFile;
#[cfg(test)]
use super::super::mocks::{spawn_blocking, spawn_mandatory_blocking};
#[cfg(not(test))]
use crate::blocking::JoinHandle;
#[cfg(not(test))]
use crate::blocking::{spawn_blocking, spawn_mandatory_blocking};
#[cfg(not(test))]
use std::fs::File as StdFile;

pub(crate) struct ThreadPool {
    pub(super) std: Arc<StdFile>,
    pub(super) inner: Mutex<Inner>,
    pub(super) max_buf_size: usize,
}

pub(crate) struct Inner {
    pub(super) state: State,

    /// Errors from writes/flushes are returned in write/flush calls. If a write
    /// error is observed while performing a read, it is saved until the next
    /// write / flush call.
    pub(super) last_write_err: Option<io::ErrorKind>,

    pub(super) pos: u64,
}

#[derive(Debug)]
pub(super) enum State {
    Idle(Option<Buf>),
    Busy(JoinHandle<(Operation, Buf)>),
}

#[derive(Debug)]
pub(super) enum Operation {
    Read(io::Result<usize>),
    Write(io::Result<()>),
    Seek(io::Result<u64>),
}

impl ThreadPool {
    pub(crate) fn from_std(std: StdFile) -> ThreadPool {
        ThreadPool {
            std: Arc::new(std),
            inner: Mutex::new(Inner {
                state: State::Idle(Some(Buf::with_capacity(0))),
                last_write_err: None,
                pos: 0,
            }),
            max_buf_size: DEFAULT_MAX_BUF_SIZE,
        }
    }

    pub(crate) async fn sync_all(&self) -> io::Result<()> {
        let mut inner = self.inner.lock().await;
        inner.complete_inflight().await;

        let std = self.std.clone();
        asyncify(move || std.sync_all()).await
    }

    pub(crate) async fn sync_data(&self) -> io::Result<()> {
        let mut inner = self.inner.lock().await;
        inner.complete_inflight().await;

        let std = self.std.clone();
        asyncify(move || std.sync_data()).await
    }

    pub(crate) async fn set_len(&self, size: u64) -> io::Result<()> {
        let mut inner = self.inner.lock().await;
        inner.complete_inflight().await;

        let mut buf = match inner.state {
            State::Idle(ref mut buf_cell) => buf_cell.take().unwrap(),
            _ => unreachable!(),
        };

        let seek = if !buf.is_empty() {
            Some(SeekFrom::Current(buf.discard_read()))
        } else {
            None
        };

        let std = self.std.clone();

        inner.state = State::Busy(spawn_blocking(move || {
            let res = if let Some(seek) = seek {
                (&*std).seek(seek).and_then(|_| std.set_len(size))
            } else {
                std.set_len(size)
            }
            .map(|()| 0); // the value is discarded later

            // Return the result as a seek
            (Operation::Seek(res), buf)
        }));

        let (op, buf) = match inner.state {
            State::Idle(_) => unreachable!(),
            State::Busy(ref mut rx) => rx.await?,
        };

        inner.state = State::Idle(Some(buf));

        match op {
            Operation::Seek(res) => res.map(|pos| {
                inner.pos = pos;
            }),
            _ => unreachable!(),
        }
    }

    pub(crate) async fn metadata(&self) -> io::Result<Metadata> {
        let std = self.std.clone();
        asyncify(move || std.metadata()).await
    }

    pub(crate) async fn try_clone(&self) -> io::Result<ThreadPool> {
        self.inner.lock().await.complete_inflight().await;
        let std = self.std.clone();
        let std_file = asyncify(move || std.try_clone()).await?;
        Ok(ThreadPool::from_std(std_file))
    }

    pub(crate) async fn into_std(mut self) -> StdFile {
        self.inner.get_mut().complete_inflight().await;
        Arc::try_unwrap(self.std).expect("Arc::try_unwrap failed")
    }

    pub(crate) async fn set_permissions(&self, perm: Permissions) -> io::Result<()> {
        let std = self.std.clone();
        asyncify(move || std.set_permissions(perm)).await
    }

    pub(crate) fn set_max_buf_size(&mut self, max_buf_size: usize) {
        self.max_buf_size = max_buf_size;
    }
}

impl AsyncRead for ThreadPool {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        dst: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        ready!(crate::trace::trace_leaf(cx));

        let me = self.get_mut();
        let inner = me.inner.get_mut();

        loop {
            match inner.state {
                State::Idle(ref mut buf_cell) => {
                    let mut buf = buf_cell.take().unwrap();

                    if !buf.is_empty() || dst.remaining() == 0 {
                        buf.copy_to(dst);
                        *buf_cell = Some(buf);
                        return Poll::Ready(Ok(()));
                    }

                    let std = me.std.clone();

                    let max_buf_size = cmp::min(dst.remaining(), me.max_buf_size);
                    inner.state = State::Busy(spawn_blocking(move || {
                        // SAFETY: the `Read` implementation of `std` does not
                        // read from the buffer it is borrowing and correctly
                        // reports the length of the data written into the buffer.
                        let res = unsafe { buf.read_from(&mut &*std, max_buf_size) };
                        (Operation::Read(res), buf)
                    }));
                }
                State::Busy(ref mut rx) => {
                    let (op, mut buf) = ready!(Pin::new(rx).poll(cx))?;

                    match op {
                        Operation::Read(Ok(_)) => {
                            buf.copy_to(dst);
                            inner.state = State::Idle(Some(buf));
                            return Poll::Ready(Ok(()));
                        }
                        Operation::Read(Err(e)) => {
                            assert!(buf.is_empty());

                            inner.state = State::Idle(Some(buf));
                            return Poll::Ready(Err(e));
                        }
                        Operation::Write(Ok(())) => {
                            assert!(buf.is_empty());
                            inner.state = State::Idle(Some(buf));
                            continue;
                        }
                        Operation::Write(Err(e)) => {
                            assert!(inner.last_write_err.is_none());
                            inner.last_write_err = Some(e.kind());
                            inner.state = State::Idle(Some(buf));
                        }
                        Operation::Seek(result) => {
                            assert!(buf.is_empty());
                            inner.state = State::Idle(Some(buf));
                            if let Ok(pos) = result {
                                inner.pos = pos;
                            }
                            continue;
                        }
                    }
                }
            }
        }
    }
}

impl AsyncSeek for ThreadPool {
    fn start_seek(self: Pin<&mut Self>, mut pos: SeekFrom) -> io::Result<()> {
        let me = self.get_mut();
        let inner = me.inner.get_mut();

        match inner.state {
            State::Busy(_) => Err(io::Error::new(
                io::ErrorKind::Other,
                "other file operation is pending, call poll_complete before start_seek",
            )),
            State::Idle(ref mut buf_cell) => {
                let mut buf = buf_cell.take().unwrap();

                // Factor in any unread data from the buf
                if !buf.is_empty() {
                    let n = buf.discard_read();

                    if let SeekFrom::Current(ref mut offset) = pos {
                        *offset += n;
                    }
                }

                let std = me.std.clone();

                inner.state = State::Busy(spawn_blocking(move || {
                    let res = (&*std).seek(pos);
                    (Operation::Seek(res), buf)
                }));
                Ok(())
            }
        }
    }

    fn poll_complete(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
        ready!(crate::trace::trace_leaf(cx));
        let inner = self.inner.get_mut();

        loop {
            match inner.state {
                State::Idle(_) => return Poll::Ready(Ok(inner.pos)),
                State::Busy(ref mut rx) => {
                    let (op, buf) = ready!(Pin::new(rx).poll(cx))?;
                    inner.state = State::Idle(Some(buf));

                    match op {
                        Operation::Read(_) => {}
                        Operation::Write(Err(e)) => {
                            assert!(inner.last_write_err.is_none());
                            inner.last_write_err = Some(e.kind());
                        }
                        Operation::Write(_) => {}
                        Operation::Seek(res) => {
                            if let Ok(pos) = res {
                                inner.pos = pos;
                            }
                            return Poll::Ready(res);
                        }
                    }
                }
            }
        }
    }
}

impl AsyncWrite for ThreadPool {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        src: &[u8],
    ) -> Poll<io::Result<usize>> {
        ready!(crate::trace::trace_leaf(cx));
        let me = self.get_mut();
        let inner = me.inner.get_mut();

        if let Some(e) = inner.last_write_err.take() {
            return Poll::Ready(Err(e.into()));
        }

        loop {
            match inner.state {
                State::Idle(ref mut buf_cell) => {
                    let mut buf = buf_cell.take().unwrap();

                    let seek = if !buf.is_empty() {
                        Some(SeekFrom::Current(buf.discard_read()))
                    } else {
                        None
                    };

                    let n = buf.copy_from(src, me.max_buf_size);
                    let std = me.std.clone();

                    let blocking_task_join_handle = spawn_mandatory_blocking(move || {
                        let res = if let Some(seek) = seek {
                            (&*std).seek(seek).and_then(|_| buf.write_to(&mut &*std))
                        } else {
                            buf.write_to(&mut &*std)
                        };

                        (Operation::Write(res), buf)
                    })
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::Other, "background task failed")
                    })?;

                    inner.state = State::Busy(blocking_task_join_handle);

                    return Poll::Ready(Ok(n));
                }
                State::Busy(ref mut rx) => {
                    let (op, buf) = ready!(Pin::new(rx).poll(cx))?;
                    inner.state = State::Idle(Some(buf));

                    match op {
                        Operation::Read(_) => {
                            // We don't care about the result here. The fact
                            // that the cursor has advanced will be reflected in
                            // the next iteration of the loop
                            continue;
                        }
                        Operation::Write(res) => {
                            // If the previous write was successful, continue.
                            // Otherwise, error.
                            res?;
                            continue;
                        }
                        Operation::Seek(_) => {
                            // Ignore the seek
                            continue;
                        }
                    }
                }
            }
        }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        ready!(crate::trace::trace_leaf(cx));
        let me = self.get_mut();
        let inner = me.inner.get_mut();

        if let Some(e) = inner.last_write_err.take() {
            return Poll::Ready(Err(e.into()));
        }

        loop {
            match inner.state {
                State::Idle(ref mut buf_cell) => {
                    let mut buf = buf_cell.take().unwrap();

                    let seek = if !buf.is_empty() {
                        Some(SeekFrom::Current(buf.discard_read()))
                    } else {
                        None
                    };

                    let n = buf.copy_from_bufs(bufs, me.max_buf_size);
                    let std = me.std.clone();

                    let blocking_task_join_handle = spawn_mandatory_blocking(move || {
                        let res = if let Some(seek) = seek {
                            (&*std).seek(seek).and_then(|_| buf.write_to(&mut &*std))
                        } else {
                            buf.write_to(&mut &*std)
                        };

                        (Operation::Write(res), buf)
                    })
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::Other, "background task failed")
                    })?;

                    inner.state = State::Busy(blocking_task_join_handle);

                    return Poll::Ready(Ok(n));
                }
                State::Busy(ref mut rx) => {
                    let (op, buf) = ready!(Pin::new(rx).poll(cx))?;
                    inner.state = State::Idle(Some(buf));

                    match op {
                        Operation::Read(_) => {
                            // We don't care about the result here. The fact
                            // that the cursor has advanced will be reflected in
                            // the next iteration of the loop
                            continue;
                        }
                        Operation::Write(res) => {
                            // If the previous write was successful, continue.
                            // Otherwise, error.
                            res?;
                            continue;
                        }
                        Operation::Seek(_) => {
                            // Ignore the seek
                            continue;
                        }
                    }
                }
            }
        }
    }

    fn is_write_vectored(&self) -> bool {
        true
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        ready!(crate::trace::trace_leaf(cx));
        let inner = self.inner.get_mut();
        inner.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        ready!(crate::trace::trace_leaf(cx));
        self.poll_flush(cx)
    }
}

impl From<StdFile> for ThreadPool {
    fn from(std: StdFile) -> Self {
        Self::from_std(std)
    }
}

impl fmt::Debug for ThreadPool {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("tokio::fs::File")
            .field("std", &self.std)
            .finish()
    }
}

#[cfg(unix)]
impl std::os::unix::io::AsRawFd for ThreadPool {
    fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
        self.std.as_raw_fd()
    }
}

#[cfg(unix)]
impl std::os::unix::io::AsFd for ThreadPool {
    fn as_fd(&self) -> std::os::unix::io::BorrowedFd<'_> {
        unsafe {
            std::os::unix::io::BorrowedFd::borrow_raw(std::os::unix::io::AsRawFd::as_raw_fd(self))
        }
    }
}

#[cfg(unix)]
impl std::os::unix::io::FromRawFd for ThreadPool {
    unsafe fn from_raw_fd(fd: std::os::unix::io::RawFd) -> Self {
        StdFile::from_raw_fd(fd).into()
    }
}

cfg_windows! {
    use crate::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle, AsHandle, BorrowedHandle};

    impl AsRawHandle for File {
        fn as_raw_handle(&self) -> RawHandle {
            self.std.as_raw_handle()
        }
    }

    impl AsHandle for File {
        fn as_handle(&self) -> BorrowedHandle<'_> {
            unsafe {
                BorrowedHandle::borrow_raw(
                    AsRawHandle::as_raw_handle(self),
                )
            }
        }
    }

    impl FromRawHandle for File {
        unsafe fn from_raw_handle(handle: RawHandle) -> Self {
            StdFile::from_raw_handle(handle).into()
        }
    }
}

impl Inner {
    async fn complete_inflight(&mut self) {
        use std::future::poll_fn;

        poll_fn(|cx| self.poll_complete_inflight(cx)).await;
    }

    fn poll_complete_inflight(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        ready!(crate::trace::trace_leaf(cx));
        match self.poll_flush(cx) {
            Poll::Ready(Err(e)) => {
                self.last_write_err = Some(e.kind());
                Poll::Ready(())
            }
            Poll::Ready(Ok(())) => Poll::Ready(()),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_flush(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        if let Some(e) = self.last_write_err.take() {
            return Poll::Ready(Err(e.into()));
        }

        let (op, buf) = match self.state {
            State::Idle(_) => return Poll::Ready(Ok(())),
            State::Busy(ref mut rx) => ready!(Pin::new(rx).poll(cx))?,
        };

        // The buffer is not used here
        self.state = State::Idle(Some(buf));

        match op {
            Operation::Read(_) => Poll::Ready(Ok(())),
            Operation::Write(res) => Poll::Ready(res),
            Operation::Seek(_) => Poll::Ready(Ok(())),
        }
    }
}
