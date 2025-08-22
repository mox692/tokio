use super::utils::cstr;
use crate::{
    fs::{File, UringOpenOptions},
    runtime::driver::op::{CancelData, Cancellable, Completable, CqeResult, Op},
};
use io_uring::{opcode, types};
use std::{
    ffi::CString,
    io,
    os::fd::{AsRawFd, BorrowedFd, FromRawFd},
    path::Path,
    sync::Arc,
};

#[derive(Debug)]
pub(crate) struct Write {}

impl Completable for Write {
    type Output = usize;
    fn complete(self, cqe: CqeResult) -> io::Result<Self::Output> {
        // TODO: check
        Ok(cqe.result? as usize)
    }
}

impl Cancellable for Write {
    fn cancel(self) -> CancelData {
        CancelData::Write(self)
    }
}

impl Op<Write> {
    /// Submit a request to open a file.
    /// TODO: consider fd type
    pub(crate) fn write(file: Arc<File>, contents: impl AsRef<[u8]>) -> io::Result<Op<Write>> {
        let ptr = contents.as_ref().as_ptr();
        let len = contents.as_ref().len();
        let write_op = opcode::Write::new(types::Fd(file.as_raw_fd()), ptr, len)
            .offset(0)
            .build();
        let op = unsafe { Op::new(write_op, Write {}) };

        // let inner_opt = options;
        // let path = cstr(path)?;

        // let custom_flags = inner_opt.custom_flags;
        // let flags = libc::O_CLOEXEC
        //     | options.access_mode()?
        //     | options.creation_mode()?
        //     | (custom_flags & !libc::O_ACCMODE);

        // let open_op = opcode::OpenAt::new(types::Fd(libc::AT_FDCWD), path.as_ptr())
        //     .flags(flags)
        //     .mode(inner_opt.mode)
        //     .build();

        // // SAFETY: Parameters are valid for the entire duration of the operation
        // let op = unsafe { Op::new(open_op, Open { path }) };
        // Ok(op)

        todo!()
    }
}
