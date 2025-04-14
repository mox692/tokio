use crate::{
    fs::{Kind, Uring},
    runtime::context::{Completable, Op},
};
use std::io;

pub(crate) struct Open {}

impl Op<Open> {}

impl Completable for Open {
    type Output = io::Result<crate::fs::File>;
    fn complete(self, cqe: crate::runtime::context::CqeResult) -> Self::Output {
        let fd = cqe.result? as i32;
        let file = crate::fs::File {
            inner: Kind::Uring(Uring::from_raw_fd(fd)),
        };
        Ok(file)
    }
}
