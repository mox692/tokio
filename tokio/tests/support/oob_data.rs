use std::io;
use std::os::unix::io::AsRawFd;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn send_oob_data<S: AsRawFd>(stream: &S, data: &[u8]) -> io::Result<usize> {
    unsafe {
        let res = libc::send(
            stream.as_raw_fd(),
            data.as_ptr().cast(),
            data.len(),
            libc::MSG_OOB,
        );
        if res == -1 {
            Err(io::Error::last_os_error())
        } else {
            Ok(res as usize)
        }
    }
}
