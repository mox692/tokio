//! Types for working with [`File`].
//!
//! [`File`]: File

use thread_pool::{Inner, State, ThreadPool};
use uring::Uring;

use crate::fs::{asyncify, OpenOptions};
use crate::io::blocking::{Buf, DEFAULT_MAX_BUF_SIZE};
use crate::io::{AsyncRead, AsyncSeek, AsyncWrite, ReadBuf};
use crate::sync::Mutex;

use std::fmt;
use std::fs::{Metadata, Permissions};
use std::io::{self, SeekFrom};
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

#[cfg(test)]
use super::mocks::MockFile as StdFile;
#[cfg(not(test))]
use std::fs::File as StdFile;

pub(crate) mod thread_pool;
pub(crate) mod uring;

/// A reference to an open file on the filesystem.
///
/// This is a specialized version of [`std::fs::File`] for usage from the
/// Tokio runtime.
///
/// An instance of a `File` can be read and/or written depending on what options
/// it was opened with. Files also implement [`AsyncSeek`] to alter the logical
/// cursor that the file contains internally.
///
/// A file will not be closed immediately when it goes out of scope if there
/// are any IO operations that have not yet completed. To ensure that a file is
/// closed immediately when it is dropped, you should call [`flush`] before
/// dropping it. Note that this does not ensure that the file has been fully
/// written to disk; the operating system might keep the changes around in an
/// in-memory buffer. See the [`sync_all`] method for telling the OS to write
/// the data to disk.
///
/// Reading and writing to a `File` is usually done using the convenience
/// methods found on the [`AsyncReadExt`] and [`AsyncWriteExt`] traits.
///
/// [`AsyncSeek`]: trait@crate::io::AsyncSeek
/// [`flush`]: fn@crate::io::AsyncWriteExt::flush
/// [`sync_all`]: fn@crate::fs::File::sync_all
/// [`AsyncReadExt`]: trait@crate::io::AsyncReadExt
/// [`AsyncWriteExt`]: trait@crate::io::AsyncWriteExt
///
/// # Examples
///
/// Create a new file and asynchronously write bytes to it:
///
/// ```no_run
/// use tokio::fs::File;
/// use tokio::io::AsyncWriteExt; // for write_all()
///
/// # async fn dox() -> std::io::Result<()> {
/// let mut file = File::create("foo.txt").await?;
/// file.write_all(b"hello, world!").await?;
/// # Ok(())
/// # }
/// ```
///
/// Read the contents of a file into a buffer:
///
/// ```no_run
/// use tokio::fs::File;
/// use tokio::io::AsyncReadExt; // for read_to_end()
///
/// # async fn dox() -> std::io::Result<()> {
/// let mut file = File::open("foo.txt").await?;
///
/// let mut contents = vec![];
/// file.read_to_end(&mut contents).await?;
///
/// println!("len = {}", contents.len());
/// # Ok(())
/// # }
/// ```
pub struct File {
    pub(crate) inner: Kind,
}

pub(crate) enum Kind {
    ThreadPool(ThreadPool),

    // TODO cfg gate
    Uring(Uring),
}

impl File {
    /// Attempts to open a file in read-only mode.
    ///
    /// See [`OpenOptions`] for more details.
    ///
    /// # Errors
    ///
    /// This function will return an error if called from outside of the Tokio
    /// runtime or if path does not already exist. Other errors may also be
    /// returned according to `OpenOptions::open`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    /// use tokio::io::AsyncReadExt;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let mut file = File::open("foo.txt").await?;
    ///
    /// let mut contents = vec![];
    /// file.read_to_end(&mut contents).await?;
    ///
    /// println!("len = {}", contents.len());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// The [`read_to_end`] method is defined on the [`AsyncReadExt`] trait.
    ///
    /// [`read_to_end`]: fn@crate::io::AsyncReadExt::read_to_end
    /// [`AsyncReadExt`]: trait@crate::io::AsyncReadExt
    pub async fn open(path: impl AsRef<Path>) -> io::Result<File> {
        let path = path.as_ref().to_owned();
        let std = asyncify(|| StdFile::open(path)).await?;

        Ok(Self {
            inner: Kind::ThreadPool(ThreadPool::from_std(std)),
        })
    }

    /// Opens a file in write-only mode.
    ///
    /// This function will create a file if it does not exist, and will truncate
    /// it if it does.
    ///
    /// See [`OpenOptions`] for more details.
    ///
    /// # Errors
    ///
    /// Results in an error if called from outside of the Tokio runtime or if
    /// the underlying [`create`] call results in an error.
    ///
    /// [`create`]: std::fs::File::create
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    /// use tokio::io::AsyncWriteExt;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let mut file = File::create("foo.txt").await?;
    /// file.write_all(b"hello, world!").await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// The [`write_all`] method is defined on the [`AsyncWriteExt`] trait.
    ///
    /// [`write_all`]: fn@crate::io::AsyncWriteExt::write_all
    /// [`AsyncWriteExt`]: trait@crate::io::AsyncWriteExt
    pub async fn create(path: impl AsRef<Path>) -> io::Result<File> {
        let path = path.as_ref().to_owned();
        let std_file = asyncify(move || StdFile::create(path)).await?;
        Ok(Self {
            inner: Kind::ThreadPool(ThreadPool::from_std(std_file)),
        })
    }

    /// Opens a file in read-write mode.
    ///
    /// This function will create a file if it does not exist, or return an error
    /// if it does. This way, if the call succeeds, the file returned is guaranteed
    /// to be new.
    ///
    /// This option is useful because it is atomic. Otherwise between checking
    /// whether a file exists and creating a new one, the file may have been
    /// created by another process (a TOCTOU race condition / attack).
    ///
    /// This can also be written using `File::options().read(true).write(true).create_new(true).open(...)`.
    ///
    /// See [`OpenOptions`] for more details.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    /// use tokio::io::AsyncWriteExt;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let mut file = File::create_new("foo.txt").await?;
    /// file.write_all(b"hello, world!").await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// The [`write_all`] method is defined on the [`AsyncWriteExt`] trait.
    ///
    /// [`write_all`]: fn@crate::io::AsyncWriteExt::write_all
    /// [`AsyncWriteExt`]: trait@crate::io::AsyncWriteExt
    pub async fn create_new<P: AsRef<Path>>(path: P) -> std::io::Result<File> {
        Self::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
            .await
    }

    /// Returns a new [`OpenOptions`] object.
    ///
    /// This function returns a new `OpenOptions` object that you can use to
    /// open or create a file with specific options if `open()` or `create()`
    /// are not appropriate.
    ///
    /// It is equivalent to `OpenOptions::new()`, but allows you to write more
    /// readable code. Instead of
    /// `OpenOptions::new().append(true).open("example.log")`,
    /// you can write `File::options().append(true).open("example.log")`. This
    /// also avoids the need to import `OpenOptions`.
    ///
    /// See the [`OpenOptions::new`] function for more details.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    /// use tokio::io::AsyncWriteExt;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let mut f = File::options().append(true).open("example.log").await?;
    /// f.write_all(b"new line\n").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn options() -> OpenOptions {
        OpenOptions::new()
    }

    /// Converts a [`std::fs::File`] to a [`tokio::fs::File`](File).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // This line could block. It is not recommended to do this on the Tokio
    /// // runtime.
    /// let std_file = std::fs::File::open("foo.txt").unwrap();
    /// let file = tokio::fs::File::from_std(std_file);
    /// ```
    pub fn from_std(std: StdFile) -> File {
        Self {
            inner: Kind::ThreadPool(ThreadPool {
                std: Arc::new(std),
                inner: Mutex::new(Inner {
                    state: State::Idle(Some(Buf::with_capacity(0))),
                    last_write_err: None,
                    pos: 0,
                }),
                max_buf_size: DEFAULT_MAX_BUF_SIZE,
            }),
        }
    }

    /// Attempts to sync all OS-internal metadata to disk.
    ///
    /// This function will attempt to ensure that all in-core data reaches the
    /// filesystem before returning.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    /// use tokio::io::AsyncWriteExt;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let mut file = File::create("foo.txt").await?;
    /// file.write_all(b"hello, world!").await?;
    /// file.sync_all().await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// The [`write_all`] method is defined on the [`AsyncWriteExt`] trait.
    ///
    /// [`write_all`]: fn@crate::io::AsyncWriteExt::write_all
    /// [`AsyncWriteExt`]: trait@crate::io::AsyncWriteExt
    pub async fn sync_all(&self) -> io::Result<()> {
        match &self.inner {
            Kind::ThreadPool(p) => p.sync_all().await,
            Kind::Uring(_) => unimplemented!(),
        }
    }

    /// This function is similar to `sync_all`, except that it may not
    /// synchronize file metadata to the filesystem.
    ///
    /// This is intended for use cases that must synchronize content, but don't
    /// need the metadata on disk. The goal of this method is to reduce disk
    /// operations.
    ///
    /// Note that some platforms may simply implement this in terms of `sync_all`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    /// use tokio::io::AsyncWriteExt;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let mut file = File::create("foo.txt").await?;
    /// file.write_all(b"hello, world!").await?;
    /// file.sync_data().await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// The [`write_all`] method is defined on the [`AsyncWriteExt`] trait.
    ///
    /// [`write_all`]: fn@crate::io::AsyncWriteExt::write_all
    /// [`AsyncWriteExt`]: trait@crate::io::AsyncWriteExt
    pub async fn sync_data(&self) -> io::Result<()> {
        match &self.inner {
            Kind::ThreadPool(p) => p.sync_data().await,
            Kind::Uring(_) => unimplemented!(),
        }
    }

    /// Truncates or extends the underlying file, updating the size of this file to become size.
    ///
    /// If the size is less than the current file's size, then the file will be
    /// shrunk. If it is greater than the current file's size, then the file
    /// will be extended to size and have all of the intermediate data filled in
    /// with 0s.
    ///
    /// # Errors
    ///
    /// This function will return an error if the file is not opened for
    /// writing.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    /// use tokio::io::AsyncWriteExt;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let mut file = File::create("foo.txt").await?;
    /// file.write_all(b"hello, world!").await?;
    /// file.set_len(10).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// The [`write_all`] method is defined on the [`AsyncWriteExt`] trait.
    ///
    /// [`write_all`]: fn@crate::io::AsyncWriteExt::write_all
    /// [`AsyncWriteExt`]: trait@crate::io::AsyncWriteExt
    pub async fn set_len(&self, size: u64) -> io::Result<()> {
        match &self.inner {
            Kind::ThreadPool(p) => p.set_len(size).await,
            Kind::Uring(_) => unimplemented!(),
        }
    }

    /// Queries metadata about the underlying file.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let file = File::open("foo.txt").await?;
    /// let metadata = file.metadata().await?;
    ///
    /// println!("{:?}", metadata);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn metadata(&self) -> io::Result<Metadata> {
        match &self.inner {
            Kind::ThreadPool(p) => p.metadata().await,
            Kind::Uring(_) => unimplemented!(),
        }
    }

    /// Creates a new `File` instance that shares the same underlying file handle
    /// as the existing `File` instance. Reads, writes, and seeks will affect both
    /// File instances simultaneously.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let file = File::open("foo.txt").await?;
    /// let file_clone = file.try_clone().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn try_clone(&self) -> io::Result<File> {
        match &self.inner {
            Kind::ThreadPool(p) => Ok(File {
                inner: Kind::ThreadPool(p.try_clone().await?),
            }),
            Kind::Uring(_) => unimplemented!(),
        }
    }

    /// Destructures `File` into a [`std::fs::File`]. This function is
    /// async to allow any in-flight operations to complete.
    ///
    /// Use `File::try_into_std` to attempt conversion immediately.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let tokio_file = File::open("foo.txt").await?;
    /// let std_file = tokio_file.into_std().await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn into_std(self) -> StdFile {
        match self.inner {
            Kind::ThreadPool(p) => p.into_std().await,
            Kind::Uring(_) => unimplemented!(),
        }
    }

    /// Tries to immediately destructure `File` into a [`std::fs::File`].
    ///
    /// # Errors
    ///
    /// This function will return an error containing the file if some
    /// operation is in-flight.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let tokio_file = File::open("foo.txt").await?;
    /// let std_file = tokio_file.try_into_std().unwrap();
    /// # Ok(())
    /// # }
    /// ```
    pub fn try_into_std(self) -> Result<StdFile, Self> {
        match self.inner {
            Kind::ThreadPool(p) => p.try_into_std().map_err(|p| File {
                inner: Kind::ThreadPool(p),
            }),
            Kind::Uring(_) => unimplemented!(),
        }
    }

    /// Changes the permissions on the underlying file.
    ///
    /// # Platform-specific behavior
    ///
    /// This function currently corresponds to the `fchmod` function on Unix and
    /// the `SetFileInformationByHandle` function on Windows. Note that, this
    /// [may change in the future][changes].
    ///
    /// [changes]: https://doc.rust-lang.org/std/io/index.html#platform-specific-behavior
    ///
    /// # Errors
    ///
    /// This function will return an error if the user lacks permission change
    /// attributes on the underlying file. It may also return an error in other
    /// os-specific unspecified cases.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let file = File::open("foo.txt").await?;
    /// let mut perms = file.metadata().await?.permissions();
    /// perms.set_readonly(true);
    /// file.set_permissions(perms).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn set_permissions(&self, perm: Permissions) -> io::Result<()> {
        match &self.inner {
            Kind::ThreadPool(p) => p.set_permissions(perm).await,
            Kind::Uring(_) => unimplemented!(),
        }
    }

    /// Set the maximum buffer size for the underlying [`AsyncRead`] / [`AsyncWrite`] operation.
    ///
    /// Although Tokio uses a sensible default value for this buffer size, this function would be
    /// useful for changing that default depending on the situation.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    /// use tokio::io::AsyncWriteExt;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let mut file = File::open("foo.txt").await?;
    ///
    /// // Set maximum buffer size to 8 MiB
    /// file.set_max_buf_size(8 * 1024 * 1024);
    ///
    /// let mut buf = vec![1; 1024 * 1024 * 1024];
    ///
    /// // Write the 1 GiB buffer in chunks up to 8 MiB each.
    /// file.write_all(&mut buf).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_max_buf_size(&mut self, max_buf_size: usize) {
        match &mut self.inner {
            Kind::ThreadPool(p) => p.set_max_buf_size(max_buf_size),
            Kind::Uring(_) => unimplemented!(),
        }
    }
}

impl AsyncRead for File {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        dst: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut().inner {
            Kind::ThreadPool(ref mut p) => Pin::new(p).poll_read(cx, dst),
            Kind::Uring(_) => unimplemented!(),
        }
    }
}

impl AsyncSeek for File {
    fn start_seek(self: Pin<&mut Self>, pos: SeekFrom) -> io::Result<()> {
        match self.get_mut().inner {
            Kind::ThreadPool(ref mut p) => Pin::new(p).start_seek(pos),
            Kind::Uring(_) => unimplemented!(),
        }
    }

    fn poll_complete(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
        match self.get_mut().inner {
            Kind::ThreadPool(ref mut p) => Pin::new(p).poll_complete(cx),
            Kind::Uring(_) => unimplemented!(),
        }
    }
}

impl AsyncWrite for File {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        src: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut().inner {
            Kind::ThreadPool(ref mut p) => Pin::new(p).poll_write(cx, src),
            Kind::Uring(_) => unimplemented!(),
        }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        match self.get_mut().inner {
            Kind::ThreadPool(ref mut p) => Pin::new(p).poll_write_vectored(cx, bufs),
            Kind::Uring(_) => unimplemented!(),
        }
    }

    fn is_write_vectored(&self) -> bool {
        true
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        match self.get_mut().inner {
            Kind::ThreadPool(ref mut p) => Pin::new(p).poll_flush(cx),
            Kind::Uring(_) => unimplemented!(),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        match self.get_mut().inner {
            Kind::ThreadPool(ref mut p) => Pin::new(p).poll_shutdown(cx),
            Kind::Uring(_) => unimplemented!(),
        }
    }
}

impl From<StdFile> for File {
    fn from(std: StdFile) -> Self {
        Self::from_std(std)
    }
}

impl fmt::Debug for File {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            Kind::ThreadPool(p) => p.fmt(fmt),
            Kind::Uring(_) => unimplemented!(),
        }
    }
}

#[cfg(unix)]
impl std::os::unix::io::AsRawFd for File {
    fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
        match &self.inner {
            Kind::ThreadPool(inner) => inner.std.as_raw_fd(),
            Kind::Uring(inner) => inner.as_raw_fd(),
        }
    }
}

#[cfg(unix)]
impl std::os::unix::io::AsFd for File {
    fn as_fd(&self) -> std::os::unix::io::BorrowedFd<'_> {
        unsafe {
            std::os::unix::io::BorrowedFd::borrow_raw(std::os::unix::io::AsRawFd::as_raw_fd(self))
        }
    }
}

#[cfg(unix)]
impl std::os::unix::io::FromRawFd for File {
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

#[cfg(test)]
mod tests;
