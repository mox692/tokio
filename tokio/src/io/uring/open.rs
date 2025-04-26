use super::utils::cstr;
use crate::{
    fs::OpenOptions,
    runtime::driver::op::{Completable, CqeResult, Op},
};
use io_uring::{opcode, types};
use std::{ffi::CString, io, os::fd::FromRawFd, path::Path};

pub(crate) struct Open {
    #[allow(dead_code)]
    path: CString,
}

impl Op<Open> {}

impl Completable for Open {
    type Output = io::Result<crate::fs::File>;
    fn complete(self, cqe: CqeResult) -> Self::Output {
        let fd = cqe.result? as i32;
        let file = unsafe { crate::fs::File::from_raw_fd(fd) };
        Ok(file)
    }
}

impl Op<Open> {
    /// Submit a request to open a file.
    pub(crate) fn open(path: &Path, options: &OpenOptions) -> io::Result<Op<Open>> {
        let inner_opt = options;
        let path = cstr(path)?;

        let custom_flags = inner_opt.0.custom_flags;
        let flags = libc::O_CLOEXEC
            | options.0.access_mode()?
            | options.0.creation_mode()?
            | (custom_flags & !libc::O_ACCMODE);

        let open_op = opcode::OpenAt::new(types::Fd(libc::AT_FDCWD), path.as_ptr())
            .flags(flags)
            .mode(inner_opt.0.mode)
            .build();

        Ok(Op::new(open_op, Open { path }))
    }
}
