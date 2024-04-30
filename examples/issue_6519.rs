use pin_project_lite::pin_project;
use std::task::Poll;
use tokio::io::{copy_bidirectional, AsyncRead, AsyncWrite, AsyncWriteExt};

enum State {
    Start,
    Pending1,
    Finish,
}

pin_project! {
    struct MyStream {
        buf_data: Option<String>,
        dest: String,
        state: State,
    }
}

impl MyStream {
    fn new(buf_data: Option<String>) -> Self {
        Self {
            buf_data: buf_data,
            dest: String::new(),
            state: State::Start,
        }
    }

    fn print_dest(&self) {
        println!("dest: {}", &self.dest);
    }

    fn print_buf_data(&self) {
        println!("buf_data: {:?}", &self.buf_data);
    }
}

impl AsyncRead for MyStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = unsafe { self.get_unchecked_mut() };
        if let Some(data) = this.buf_data.take() {
            buf.put_slice(data.as_bytes());
            Poll::Ready(Ok(()))
        } else {
            Poll::Ready(Ok(()))
        }
    }
}

impl AsyncWrite for MyStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        let this = unsafe { self.get_unchecked_mut() };

        let data = std::str::from_utf8(buf).unwrap().to_string();
        let len = data.len();

        this.buf_data = Some(data);

        Poll::Ready(Ok(len))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let this = self.project();

        match this.state {
            State::Pending1 => {
                let buf = this.buf_data.take().unwrap_or_default();
                let cur_dest = this.dest;
                cur_dest.push_str(buf.as_str());
                *this.state = State::Finish;

                return Poll::Ready(Ok(()));
            }
            State::Start => {
                *this.state = State::Pending1;
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
            State::Finish => return Poll::Ready(Ok(())),
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Poll::Ready(Ok(()))
    }
}

async fn simple_example() {
    let h = tokio::spawn(async {
        let mut st = MyStream::new(None);
        // empty
        st.print_dest();

        st.write_all(b"world").await.unwrap();

        // empty
        st.print_dest();

        st.flush().await.unwrap();
        // st.flush().await.unwrap();

        // empty
        st.print_dest();
    });
    h.await.unwrap();
}

async fn bidirectional_example() {
    let mut st1 = MyStream::new(None);
    let mut st2 = MyStream::new(None);

    st1.write(b"hello").await.unwrap();

    st1.print_buf_data();
    st2.print_buf_data();

    // 2つのstreamのbuf_dataを, 同じにする.
    // st1 -> st2
    //   st1(reader), st2(writer)で, st1のデータ(hello)をst2に書き込む.
    // st2 -> st1
    //   st1(writer), st2(reader)で, st2のデータ(None)をst1に書き込む.
    let f = copy_bidirectional(&mut st1, &mut st2).await;

    st1.print_buf_data();
    st2.print_buf_data();
}

#[tokio::main]
async fn main() {
    simple_example().await;

    // bidirectional_example().await;
}
