use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use bytes::BufMut;
use futures::SinkExt;
use futures_sink::Sink;
use pin_project_lite::pin_project;
use tokio::{
    io::{AsyncRead, AsyncReadExt, ReadBuf},
    net::TcpListener,
};
use tokio_stream::StreamExt;
use tokio_util::codec::{FramedRead, LengthDelimitedCodec};

#[tokio::main]
async fn main() {
    async_read().await;
}

async fn async_read() {
    pin_project! {
        #[derive(Debug)]
        struct MyAsyncReader {
            #[pin]
            buf: Vec<u8>,
            stream_pos: usize,
            sink_pos: usize
        }
    }
    impl MyAsyncReader {
        fn new(buf: Vec<u8>) -> Self {
            MyAsyncReader {
                buf,
                stream_pos: 0,
                sink_pos: 0,
            }
        }
    }
    impl AsyncRead for MyAsyncReader {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<Result<(), io::Error>> {
            let me = self.project();
            let current_pos = *me.stream_pos;

            if let Some(current_value) = me.buf.get(current_pos) {
                // If there is still value, then writes to the buffer.
                let v = [*current_value];
                let v = v.as_slice();

                buf.put(v);

                // Update the position.
                *me.stream_pos += 1;

                Poll::Ready(Ok(()))
            } else {
                // Nothing to do.
                Poll::Ready(Ok(()))
            }
        }
    }
    // 適当なSinkを実装してみる.
    impl Sink<u8> for MyAsyncReader {
        type Error = &'static str;
        fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            // Check if there are still space in the buffer to be send to sink.
            let sink_pos = self.sink_pos;
            let buf_len = self.buf.len();
            if buf_len <= sink_pos {
                Poll::Ready(Err("No more space in the buffer"))
            } else {
                // There are still space in the buffer.
                Poll::Ready(Ok(()))
            }
        }
        fn start_send(self: Pin<&mut Self>, item: u8) -> Result<(), Self::Error> {
            let me = self.project();
            println!("write an item to sink!");
            *me.sink_pos += 1;

            Ok(())
        }
        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            println!("flush done!");

            Poll::Ready(Ok(()))
        }
        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            println!("close done!");

            Poll::Ready(Ok(()))
        }
    }

    let mut reader = MyAsyncReader::new(vec![
        0x00, 0x00, 0x00, 0x02, 0xFF, 0x07, 0x00, 0x00, 0x00, 0x02, 0xFF, 0x08,
    ]);

    // Now, lets create FramedRead with MyAsyncReader.
    // Here, magic happens, where reader is converted to Stream from AsyncRead.
    let mut frame_reader = FramedRead::new(reader, LengthDelimitedCodec::new());

    // Poll the first frame.
    let v = frame_reader.next().await.unwrap().unwrap();
    println!("received: {:?}", v);

    // Poll the second frame.
    let v = frame_reader.next().await.unwrap().unwrap();
    println!("received: {:?}", v);

    // And, we can do sink operations as well if underlying inner type implements Sink.
    let _ = frame_reader.send(0x01).await.unwrap();

    println!("frame_reader state: {:?}", frame_reader);
}

async fn tcp_example() {
    let listener = TcpListener::bind("127.0.0.1:12345").await.unwrap();
    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let mut frame_reader = FramedRead::new(stream, LengthDelimitedCodec::new());
        while let Some(frame) = frame_reader.next().await {
            match frame {
                Ok(data) => println!("received: {:?}", data),
                Err(err) => eprintln!("error: {:?}", err),
            }
        }
    }
}
