use std::io;

use crate::runtime::context::{Completable, Op};

pub(crate) struct Read {}

impl Op<Read> {}

impl Completable for Read {
    type Output = io::Result<i32>;
    fn complete(self, cqe: crate::runtime::context::CqeResult) -> Self::Output {
        let n = cqe.result? as i32;
        Ok(n)
    }
}
