use io_uring::{opcode, types};

#[cfg(test)]
use super::super::mocks::MockFile as StdFile;
use super::File;
use crate::{
    fs::OpenOptions,
    io::{blocking::Buf, uring::read::Read, AsyncRead, AsyncSeek, AsyncWrite, ReadBuf},
    runtime::context::Op,
    sync::Mutex,
};
#[cfg(not(test))]
use std::fs::File as StdFile;
use std::{
    future::Future,
    io::{self, SeekFrom},
    os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd},
    path::Path,
    pin::Pin,
    sync::Arc,
    task::{ready, Context, Poll},
};

pub(crate) struct Uring {
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

pub(super) enum State {
    Idle(Option<Buf>),
    Busy(OpFuture),
}

pub(super) enum OpFuture {
    Read((Op<Read>, Option<Buf>)),
}

impl Future for OpFuture {
    type Output = io::Result<(Operation, Buf)>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        match this {
            OpFuture::Read((s, b)) => {
                let res = ready!(Pin::new(s).poll(cx));
                let buf = b.take().expect("should not be None");

                return Poll::Ready(Ok((Operation::Read(res), buf)));
            }
        };

        Poll::Pending
    }
}

#[derive(Debug)]
pub(super) enum Operation {
    Read(io::Result<i32>),
    Write(io::Result<()>),
    Seek(io::Result<u64>),
}

impl Uring {
    pub async fn open(path: impl AsRef<Path>) -> io::Result<File> {
        // let op = Open::new();
        // let res = op.await;
        unimplemented!()
    }

    pub async fn create(path: impl AsRef<Path>) -> io::Result<File> {
        unimplemented!()
    }

    pub async fn create_new<P: AsRef<Path>>(path: P) -> std::io::Result<File> {
        unimplemented!()
    }

    #[must_use]
    pub fn options() -> OpenOptions {
        unimplemented!()
    }

    pub fn from_std(std: StdFile) -> File {
        unimplemented!()
    }

    pub async fn sync_all(&self) -> io::Result<()> {
        // acquire a lock, ensuring all in-flight operations have done
        // let guard = self.state.lock().await;

        // match &mut *guard {
        //     State::Idle(_) => {
        //         // just perform the op
        //     }
        //     State::Busy(_) => {}
        // };

        // let sync_op = Fsync::new();

        // // wait until sync_op done
        // sync_op.await;

        unimplemented!()
    }

    pub async fn sync_data(&self) -> io::Result<()> {
        unimplemented!()
    }

    pub async fn set_len(&self, size: u64) -> io::Result<()> {
        unimplemented!()
    }
}

impl AsyncRead for Uring {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        dst: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        ready!(crate::trace::trace_leaf(cx));

        let me = self.get_mut();
        let fd = me.as_raw_fd();
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

                    let ptr = buf.bytes_mut().as_mut_ptr();

                    let read_op = opcode::Read::new(types::Fd(fd), ptr, buf.len() as u32).build();

                    inner.state =
                        State::Busy(OpFuture::Read((Op::new(read_op, Read::new()), Some(buf))));
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

impl AsyncSeek for Uring {
    fn start_seek(self: Pin<&mut Self>, pos: SeekFrom) -> io::Result<()> {
        let _ = (self, pos);
        unimplemented!()
    }

    fn poll_complete(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
        let _ = (self, cx);
        unimplemented!()
    }
}

impl AsyncWrite for Uring {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        src: &[u8],
    ) -> Poll<io::Result<usize>> {
        unimplemented!()
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        let _ = (self, cx, bufs);
        unimplemented!()
    }

    fn is_write_vectored(&self) -> bool {
        unimplemented!()
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let _ = (self, cx);
        unimplemented!()
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let _ = (self, cx);
        unimplemented!()
    }
}

impl From<StdFile> for Uring {
    fn from(std: StdFile) -> Self {
        Self {
            std: Arc::new(std),
            inner: Mutex::new(Inner {
                state: State::Idle(Some(Buf::with_capacity(0))),
                last_write_err: None,
                pos: 0,
            }),
            max_buf_size: 0,
        }
    }
}

impl Uring {
    pub(crate) fn from_raw_fd(fd: i32) -> Uring {
        Self {
            std: Arc::new(unsafe { StdFile::from_raw_fd(fd) }),
            inner: Mutex::new(Inner {
                state: State::Idle(Some(Buf::with_capacity(0))),
                last_write_err: None,
                pos: 0,
            }),
            max_buf_size: 0,
        }
    }
}

impl AsRawFd for Uring {
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.std.as_raw_fd()
    }
}

impl AsFd for Uring {
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(self.as_raw_fd()) }
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
