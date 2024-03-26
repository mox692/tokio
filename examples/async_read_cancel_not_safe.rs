use std::{
    pin::Pin,
    task::{Context, Poll},
};

use pin_project_lite::pin_project;
use tokio::io::{self, AsyncReadExt, AsyncWrite};

struct Connection();

pin_project! {
    struct MyFile {
        // 外界とのアクセスを実施して、そのcurを変更するとする。
        cur : usize
    }
}

impl MyFile {
    fn new() -> Self {
        Self { cur: 0 }
    }
}

impl AsyncWrite for MyFile {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let cur = self.project().cur;

        if false
        /* some condition */
        {
            // but, mutate other state.
            *cur += 4;

            return Poll::Pending;
        }

        Poll::Ready(Ok(3))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        todo!()
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        todo!()
    }
}

#[tokio::main]
async fn main() {
    let mut file = tokio::fs::File::open("/dev/zero").await.unwrap();
    let mut buf = vec![0; 1024];
    let n = file.read(&mut buf).await.unwrap();
}
