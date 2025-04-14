use std::io;

use crate::runtime::driver::op::{Completable, CqeResult};

pub(crate) struct Read {}

impl Completable for Read {
    type Output = io::Result<i32>;
    fn complete(self, cqe: CqeResult) -> Self::Output {
        let n = cqe.result? as i32;
        Ok(n)
    }
}
