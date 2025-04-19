use io_uring::{opcode, types};

use crate::{
    fs::{Kind, OpenOptions, Uring},
    runtime::driver::op::{Completable, CqeResult, Op},
};
use std::{ffi::CString, io, path::Path};

pub(crate) struct Open {
    path: CString,
}

impl Op<Open> {}

impl Completable for Open {
    type Output = io::Result<crate::fs::File>;
    fn complete(self, cqe: CqeResult) -> Self::Output {
        let fd = cqe.result? as i32;
        let file = crate::fs::File {
            inner: Kind::Uring(Uring::from_raw_fd(fd)),
        };
        Ok(file)
    }
}

impl Op<Open> {
    /// Submit a request to open a file.
    pub(crate) fn open(path: &Path, options: &OpenOptions) -> io::Result<Op<Open>> {
        let inner_opt = options.uring.as_ref().unwrap();
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

        Ok(Op::new(open_op, Open { path }))
    }
}

pub(crate) fn cstr(p: &Path) -> io::Result<CString> {
    use std::os::unix::ffi::OsStrExt;
    Ok(CString::new(p.as_os_str().as_bytes())?)
}
