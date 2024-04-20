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

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::spawn;
use tokio::task::JoinSet;

use std::error::Error;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use tracing_subscriber::layer::SubscriberExt;

#[tokio::main]
pub async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .init();

    basic().await;
    // sleep_little_task().await;
}

/// expected event:
/// * RunTask(a, b)
/// * Park(a) // because there are no tasks to run.
/// * RunTask(c, b) // task might run on a different worker.
async fn sleep_little_task() {
    spawn(async {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    })
    .await
    .unwrap();
}

#[inline(never)]
async fn foo() {
    bar().await
}

#[inline(never)]
async fn bar() {
    baz().await
}

#[inline(never)]
async fn baz() {
    tokio::time::sleep(std::time::Duration::from_nanos(1)).await;
}

/// expected event:
/// * RunTask(a, b) // sould happen 10 times. (These often seem to be scheduled concentrating on one or two workers.)
async fn basic() {
    let mut set = JoinSet::new();

    let count = Arc::new(AtomicUsize::new(0));
    for _ in 0..10 {
        let count = count.clone();
        let _ = set.spawn(async move {
            foo().await;
            count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        });
    }

    while let Some(s) = set.join_next().await {
        s.unwrap();
    }

    assert_eq!(count.load(std::sync::atomic::Ordering::Relaxed), 10)
}
