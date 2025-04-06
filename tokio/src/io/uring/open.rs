use std::{io, os::fd::FromRawFd};

use crate::runtime::context::{Completable, Op};

// TODO: should be placed elsewhere.
pub(crate) struct Open {}

// TODO: should be placed elsewhere.
impl Op<Open> {}

impl Completable for Open {
    type Output = io::Result<crate::fs::File>;
    fn complete(self, cqe: crate::runtime::context::CqeResult) -> Self::Output {
        let fd = cqe.result? as i32;
        Ok(unsafe { crate::fs::File::from_raw_fd(fd) })
    }
}
