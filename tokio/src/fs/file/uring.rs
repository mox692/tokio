use mio::{event::Source, unix::SourceFd};

#[cfg(test)]
use super::super::mocks::MockFile as StdFile;
#[cfg(not(test))]
use std::fs::File as StdFile;
use std::{
    io,
    os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd},
};

#[derive(Debug)]
pub(crate) struct Uring {
    inner: StdFile,
}

impl From<StdFile> for Uring {
    fn from(std: StdFile) -> Self {
        Self { inner: std }
    }
}

impl Uring {
    pub(crate) fn from_raw_fd(fd: i32) -> Uring {
        Self {
            inner: unsafe { StdFile::from_raw_fd(fd) },
        }
    }
}

impl AsRawFd for Uring {
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.inner.as_raw_fd()
    }
}

impl AsFd for Uring {
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(self.as_raw_fd()) }
    }
}

impl Source for Uring {
    fn register(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interest: mio::Interest,
    ) -> io::Result<()> {
        SourceFd(&self.as_raw_fd()).register(registry, token, interest)
    }

    fn reregister(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interest: mio::Interest,
    ) -> io::Result<()> {
        SourceFd(&self.as_raw_fd()).reregister(registry, token, interest)
    }

    fn deregister(&mut self, registry: &mio::Registry) -> io::Result<()> {
        SourceFd(&self.as_raw_fd()).deregister(registry)
    }
}
