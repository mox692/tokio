#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unreachable_pub)]

use std::{io, path::Path};

#[cfg(test)]
use crate::fs::mocks::MockFile;
#[cfg(not(test))]
use std::fs::File as MockFile;

/// docs
#[derive(Debug, Clone)]
pub(crate) struct UringOpenOptions {
    pub(crate) read: bool,
    pub(crate) write: bool,
    pub(crate) append: bool,
    pub(crate) truncate: bool,
    pub(crate) create: bool,
    pub(crate) create_new: bool,
    pub(crate) mode: libc::mode_t,
    pub(crate) custom_flags: libc::c_int,
}

impl UringOpenOptions {
    /// docs
    pub(crate) fn new() -> Self {
        Self {
            read: false,
            write: false,
            append: false,
            truncate: false,
            create: false,
            create_new: false,
            mode: 0o666,
            custom_flags: 0,
        }
    }

    pub fn append(&mut self, append: bool) -> &mut Self {
        todo!()
    }

    pub fn create(&mut self, create: bool) -> &mut Self {
        todo!()
    }

    pub fn create_new(&mut self, create_new: bool) -> &mut Self {
        todo!()
    }

    pub fn open<P: AsRef<Path> + 'static>(&self, path: P) -> io::Result<MockFile> {
        todo!()
    }

    pub fn read(&mut self, read: bool) -> &mut Self {
        todo!()
    }

    pub fn truncate(&mut self, truncate: bool) -> &mut Self {
        todo!()
    }

    pub fn write(&mut self, write: bool) -> &mut Self {
        todo!()
    }
}
