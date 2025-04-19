use super::utils::cstr;
use crate::runtime::driver::op::{Completable, CqeResult, Op};
use io_uring::{opcode, types};
use std::{ffi::CString, io, path::Path};

/// Perform statx(2)
pub(crate) struct Metadata {
    #[allow(dead_code)]
    path: CString,
    statx_buf: Box<libc::statx>,
}

impl Completable for Metadata {
    type Output = io::Result<std::fs::Metadata>;
    fn complete(self, cqe: CqeResult) -> Self::Output {
        cqe.result?;
        let _ = self.statx_buf;

        // Well, we need to convert the statx_buf into std::fs::Metadata, which requires
        // a bit work.
        todo!()
    }
}

impl Op<Metadata> {
    /// Submit a request to open a file.
    pub(crate) fn metadata(path: &Path) -> io::Result<Op<Metadata>> {
        let path = cstr(path)?;
        let mut statx_buf: Box<libc::statx> = Box::new(unsafe { std::mem::zeroed() });
        // TODO: check
        let flags = libc::AT_EMPTY_PATH;
        // TODO: check
        let mask = libc::STATX_ALL;
        let statx_op = opcode::Statx::new(
            types::Fd(libc::AT_FDCWD),
            path.as_ptr(),
            &mut *statx_buf as *mut libc::statx as *mut types::statx,
        )
        .flags(flags)
        .mask(mask)
        .build();

        Ok(Op::new(statx_op, Metadata { path, statx_buf }))
    }
}
