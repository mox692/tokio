use crate::{fs::OpenOptions, io::uring::read::Read};

use std::{io, os::fd::AsRawFd, path::Path};

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
    read_inner(path).await
}

cfg_not_uring_fs! {
    async fn read_inner(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
        let path = path.as_ref().to_owned();
        crate::fs::asyncify(move || std::fs::read(path)).await
    }
}

cfg_uring_fs! {
    async fn read_inner(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
        use io_uring::{opcode, types};

        // In the future, code would be something like this. (once we implement statx)
        // By usign metadata, we can get a file size and pre-allocate the buffer.

        // let file = crate::fs::File::open(path.as_ref()).await?;
        // let metadata = Op::metadata(path.as_ref())?.await?;

        // let len = metadata.len();
        // let mut buf = vec![0u8; len as usize];

        // let read_op = opcode::Read::new(
        //     types::Fd(file.as_raw_fd()),
        //     buf.as_mut_ptr(),
        //     buf.len() as u32,
        // )
        // .build();

        // Op::new(read_op, Read {}).await?;

        // Ok(buf)


        let mut buf = vec![0u8; 1024];
        let file = OpenOptions::new()
            .read(true)
            .open(path)
            .await?;
        let read_op = opcode::Read::new(
            types::Fd(file.as_raw_fd()),
            buf.as_mut_ptr(),
            buf.len() as u32,
        )
        .build();

        let _ = crate::runtime::driver::op::Op::new(read_op, Read {}).await?;

        Ok(buf)
    }
}
