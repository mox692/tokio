use crate::{fs::asyncify, runtime::context::with_ringcontext_mut};

use io_uring::{opcode, types};
use std::{ffi::c_int, io, os::unix::ffi::OsStrExt, path::Path};

/// Reads the entire contents of a file into a bytes vector.
///
/// This is an async version of [`std::fs::read`].
///
/// This is a convenience function for using [`File::open`] and [`read_to_end`]
/// with fewer imports and without an intermediate variable. It pre-allocates a
/// buffer based on the file size when available, so it is generally faster than
/// reading into a vector created with `Vec::new()`.
///
/// This operation is implemented by running the equivalent blocking operation
/// on a separate thread pool using [`spawn_blocking`].
///
/// [`File::open`]: super::File::open
/// [`read_to_end`]: crate::io::AsyncReadExt::read_to_end
/// [`spawn_blocking`]: crate::task::spawn_blocking
///
/// # Errors
///
/// This function will return an error if `path` does not already exist.
/// Other errors may also be returned according to [`OpenOptions::open`].
///
/// [`OpenOptions::open`]: super::OpenOptions::open
///
/// It will also return an error if it encounters while reading an error
/// of a kind other than [`ErrorKind::Interrupted`].
///
/// [`ErrorKind::Interrupted`]: std::io::ErrorKind::Interrupted
///
/// # Examples
///
/// ```no_run
/// use tokio::fs;
/// use std::net::SocketAddr;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error + 'static>> {
///     let contents = fs::read("address.txt").await?;
///     let foo: SocketAddr = String::from_utf8_lossy(&contents).parse()?;
///     Ok(())
/// }
/// ```
pub async fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    let path = path.as_ref().to_owned();
    asyncify(move || std::fs::read(path)).await
}

#[test]
fn test_read2() {
    let rt = crate::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        read2("test.txt").await.unwrap();
    });
}

async fn read2(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    // doing io_uring stuff

    use std::ffi::CString;

    with_ringcontext_mut(|ctx| {
        let ring = &mut ctx.ring;
        // open file
        let p_ref = path.as_ref().as_os_str().as_bytes();
        let s = CString::new(p_ref).unwrap();
        let s = s.as_ptr();
        let flags = libc::O_CLOEXEC | libc::O_RDWR | libc::O_APPEND | libc::O_CREAT;

        let open_op = opcode::OpenAt::new(types::Fd(libc::AT_FDCWD), s)
            .flags(flags)
            .mode(0o666)
            .build();

        unsafe { ring.submission().push(&open_op).unwrap() };

        let n = ring.submit_and_wait(1).unwrap();

        let mut fd = 0;
        for cqe in ring.completion() {
            fd = cqe.result();
        }

        // read a file
        let mut buf = [0u8; 32];
        let ptr = buf.as_mut_ptr();
        let len = buf.len();
        let entry = opcode::Read::new(types::Fd(fd as c_int), ptr, len as _)
            .offset(0)
            .build();

        unsafe { ring.submission().push(&entry).unwrap() };

        let n = ring.submit_and_wait(1).unwrap();

        println!("buf content:::::::: {:?}", &buf);
    });

    Ok(vec![])
}
