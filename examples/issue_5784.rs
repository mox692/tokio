//! A simple client that opens a TCP stream, writes "hello world\n", and closes
//! the connection.
//!
//! To start a server that this client can talk to on port 6142, you can use this command:
//!
//!     ncat -l 6142
//!
//! And then in another terminal run:
//!
//!     cargo run --example hello_world

#![warn(rust_2018_idioms)]

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use tokio::{io::Interest, net::TcpListener};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    let listener = TcpListener::bind(&addr).await?;

    println!("Awaiting listener");
    let (stream, _) = listener.accept().await?;

    println!("Awaiting priority");
    stream.ready(Interest::WRITABLE).await?;

    println!("Received");

    Ok(())
}
