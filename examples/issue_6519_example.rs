// Standard imports
use std::future::Future;
use std::io::Error;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

// Tokio imports
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpListener;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::{Receiver, Sender};

// --------------------------------------------------------
// Declarations

enum Data {
    Network(Vec<u8>),
}

struct Stream {
    flushing_future:
        Option<Pin<Box<dyn Send + Sync + Future<Output = Result<(), SendError<Data>>>>>>,
    receiver: Receiver<Data>,
    sender: Sender<Data>,
    write_buffer: Vec<u8>,
    read_bytes: usize,
    written_bytes: usize,
    flushed_bytes: usize,
}

// --------------------------------------------------------
// Implementations

#[tokio::main]
async fn main() {
    let server = TcpListener::bind("127.0.0.1:8000").await.unwrap();
    let (server_tx, mut server_rx) = tokio::sync::mpsc::channel::<Data>(64);

    println!("TCP server listening on 127.0.0.1:8000!");

    loop {
        // Accept a new client and creates its own channel
        let (mut client, _) = server.accept().await.unwrap();
        let (client_tx, client_rx) = tokio::sync::mpsc::channel::<Data>(64);

        println!("TCP client connected, bidirectional copy will now start.");

        // Starts the bidirectional copy between the client and our stream concurrently
        let mut stream = Stream::new(client_rx, server_tx.clone());

        tokio::spawn(async move {
            if let Err(e) = tokio::io::copy_bidirectional(&mut client, &mut stream).await {
                eprintln!("Bidirectional copy failed: {e}.");
            }
        });

        // Get client packets and answer to them while the client is opened
        while !client_tx.is_closed() {
            let Some(_data) = server_rx.recv().await else {
                break;
            };

            // Process `_data` and get a response, here hardcoded

            let response = vec![0; 512];
            let response = Data::Network(response);

            if let Err(e) = client_tx.send(response).await {
                eprintln!("Failed to send response to the client: {e}.");
                break;
            }
        }
    }
}

impl Stream {
    pub fn new(receiver: Receiver<Data>, sender: Sender<Data>) -> Self {
        Self {
            flushing_future: None,
            receiver,
            sender,
            write_buffer: Vec::new(),
            written_bytes: 0,
            read_bytes: 0,
            flushed_bytes: 0,
        }
    }

    pub fn is_stopped(&self) -> bool {
        self.sender.is_closed()
    }
}

impl AsyncRead for Stream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if self.is_stopped() {
            return Poll::Ready(Err(std::io::ErrorKind::Other.into()));
        }

        // Get some data from the server when it's ready
        let data = match self.receiver.poll_recv(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(None) => return Poll::Ready(Err(std::io::ErrorKind::UnexpectedEof.into())),
            Poll::Ready(Some(Data::Network(data))) => data,
        };

        // For the sake of simplicity, we assume that the reading buffer is always
        // large enough to hold all of `data` at once
        assert!(buf.remaining() >= data.len());

        self.read_bytes += data.len();

        println!(
            "Reading {} bytes | {} bytes read totally",
            data.len(),
            self.read_bytes
        );

        // Extend the provided buffer with server's data
        Poll::Ready(Ok(buf.put_slice(&data)))
    }
}

impl AsyncWrite for Stream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, Error>> {
        if self.is_stopped() {
            return Poll::Ready(Err(std::io::ErrorKind::Other.into()));
        }

        // Bufferize written data
        self.written_bytes += buf.len();
        self.write_buffer.extend_from_slice(buf);

        println!(
            "Writing {} bytes | {} bytes written overall.",
            buf.len(),
            self.written_bytes
        );

        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        if self.is_stopped() {
            return Poll::Ready(Err(std::io::ErrorKind::Other.into()));
        }

        // Take the existing flushing future if any, or create a new one otherwise
        let mut flushing_future = self.flushing_future.take().unwrap_or_else(|| {
            let mut data = Vec::new();

            // Take all data out of the writing buffer
            data.append(&mut self.write_buffer);
            self.flushed_bytes += data.len();

            println!(
                "Flushing {} bytes ({}/{} bytes flushed so far)",
                data.len(),
                self.flushed_bytes,
                self.written_bytes,
            );

            let data = Data::Network(data);
            let sender = self.sender.clone();

            // Send flushed data to the serve
            Box::pin(async move {
                // Simulating some slight back-pressure
                tokio::time::sleep(Duration::from_millis(1)).await;

                sender.send(data).await
            })
        });

        // Returns the flushing result if available, or store the future otherwise
        match flushing_future.as_mut().poll(cx) {
            Poll::Pending => {
                self.flushing_future = Some(flushing_future);
                println!("Flushing is pending.");
                Poll::Pending
            }
            Poll::Ready(Err(_)) => Poll::Ready(Err(std::io::ErrorKind::Other.into())),
            Poll::Ready(Ok(_)) => {
                println!(
                    "Flushing done ({}/{} bytes flushed so far)",
                    self.flushed_bytes, self.written_bytes,
                );

                if self.flushed_bytes != self.written_bytes {
                    panic!("Flushing done but not all bytes have been flushed.");
                }

                Poll::Ready(Ok(()))
            }
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Error>> {
        if self.is_stopped() {
            return Poll::Ready(Err(std::io::ErrorKind::Other.into()));
        }

        Poll::Ready(Ok(self.receiver.close()))
    }
}
