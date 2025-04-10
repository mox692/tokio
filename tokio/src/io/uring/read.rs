use std::io;

use crate::runtime::context::Completable;

pub(crate) struct Read {}

impl Read {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl Completable for Read {
    type Output = io::Result<i32>;
    fn complete(self, cqe: crate::runtime::context::CqeResult) -> Self::Output {
        let n = cqe.result? as i32;
        Ok(n)
    }
}
