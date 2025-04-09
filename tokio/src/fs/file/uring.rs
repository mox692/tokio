#[cfg(test)]
use super::super::mocks::MockFile as StdFile;
use mio::{event::Source, unix::SourceFd};
#[cfg(not(test))]
use std::fs::File as StdFile;
use std::{
    io,
    os::fd::{AsFd, AsRawFd, BorrowedFd, RawFd},
};

#[derive(Debug)]
pub(super) struct Uring {
    inner: UringInner,
}

#[derive(Debug)]
struct UringInner {
    fd: RawFd,
}

impl UringInner {
    pub(super) fn new(fd: StdFile) -> Self {
        Self { fd: fd.as_raw_fd() }
    }

    fn read_inner(&self) -> io::Result<()> {
        Ok(())
    }
}

impl AsRawFd for UringInner {
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.fd.as_raw_fd()
    }
}

impl AsFd for UringInner {
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(self.as_raw_fd()) }
    }
}

impl Source for UringInner {
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
