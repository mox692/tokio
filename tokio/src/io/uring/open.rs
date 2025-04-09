use crate::{
    fs::{Kind, Uring},
    runtime::context::{Completable, Op},
};
use std::io;

// TODO: should be placed elsewhere.
pub(crate) struct Open {}

// TODO: should be placed elsewhere.
impl Op<Open> {}

impl Completable for Open {
    type Output = io::Result<crate::fs::File>;
    fn complete(self, cqe: crate::runtime::context::CqeResult) -> Self::Output {
        let fd = cqe.result? as i32;
        // TODO: pass the flag
        let file = crate::fs::File {
            inner: Kind::Uring(Uring::from_raw_fd(fd)),
        };
        Ok(file)
    }
}
