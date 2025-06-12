use super::utils::cstr;
use crate::{
    fs::UringOpenOptions,
    runtime::driver::op::{CancelData, Cancellable, Completable, CqeResult, Op},
};
use io_uring::{opcode, types};
use std::{ffi::CString, io, os::fd::FromRawFd, path::Path};

pub(crate) struct Open {
    #[allow(dead_code)]
    path: CString,
}

impl Completable for Open {
    type Output = crate::fs::File;
    fn complete(self, cqe: CqeResult) -> io::Result<Self::Output> {
        let fd = cqe.result? as i32;
        let file = unsafe { crate::fs::File::from_raw_fd(fd) };
        Ok(file)
    }
}

impl Cancellable for Open {
    fn cancel(self) -> CancelData {
        todo!()
    }
}

impl Op<Open> {
    /// Submit a request to open a file.
    pub(crate) fn open(
        shard_id: usize,
        path: &Path,
        options: &UringOpenOptions,
    ) -> io::Result<Op<Open>> {
        let inner_opt = options;
        let path = cstr(path)?;

        let custom_flags = inner_opt.custom_flags;
        let flags = libc::O_CLOEXEC
            | options.access_mode()?
            | options.creation_mode()?
            | (custom_flags & !libc::O_ACCMODE);

        let open_op = opcode::OpenAt::new(types::Fd(libc::AT_FDCWD), path.as_ptr())
            .flags(flags)
            .mode(inner_opt.mode)
            .build();

        // SAFETY: Parameters are valid for the entire duration of the operation
        let op = unsafe { Op::new(shard_id, open_op, Open { path }) };
        Ok(op)
    }
}
