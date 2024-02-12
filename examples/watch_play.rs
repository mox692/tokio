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
use tokio::pin;
use tokio::runtime::Builder;
use tokio::sync::watch::error::RecvError;
use tokio::task::futures::TaskLocalFuture;
use tokio_stream::{self as stream, StreamExt};

use std::error::Error;
use std::future::IntoFuture;
use std::ops::Deref;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch::channel;

async fn change() {
    let (tx, mut rx) = channel("hello");

    tokio::spawn(async move {
        tx.send("goodbye").unwrap();
        tokio::time::sleep(Duration::from_secs(3)).await;
        tx.send("goodbye2").unwrap();

        // The `tx` has been dropped here
    });

    // closeされる(sendがdropされる)まで、新しい通知を待つことができる.
    // 多分こういう感じでloopで使うことが想定されている.
    while let Ok(_) = rx.changed().await {
        println!("rx: {:?}", rx.borrow());

        // changedはsennに変えるので、has_changedが必ずfalseになるはず
        let r: Result<bool, RecvError> = rx.has_changed().or(Ok(false));
        assert!(!r.unwrap())
    }
}

async fn has_change() {
    let (tx, mut rx) = channel("hello");

    // 初期値(これはseen判定になるらしい)のまま.
    assert!(!rx.has_changed().unwrap());

    tokio::spawn(async move {
        tx.send("goodbye").unwrap();
        // to prevent the tx from being dropped.
        tokio::time::sleep(Duration::from_millis(1200)).await;
    });
    tokio::time::sleep(Duration::from_secs(1)).await;

    assert!(rx.has_changed().unwrap());
}

async fn mark_change() {
    let (tx, mut rx) = channel(3);
    // 最初の値は見られた判定.
    assert!(!rx.has_changed().unwrap());
    assert_eq!(*rx.borrow(), 3);

    rx.mark_changed();

    // 実態は変わってないけど、mark_changed()が呼ばれたので見られた判定がtrueになる.
    assert!(rx.has_changed().unwrap());
    assert_eq!(*rx.borrow(), 3);
}

async fn clone_sender_exp() {
    let (tx1, mut rx) = channel(3);
    assert_eq!(tx1.sender_count(), 1);

    let tx2 = tx1.clone();
    assert_eq!(tx1.sender_count(), 2);
    assert_eq!(tx2.sender_count(), 2);

    drop(tx1);

    assert_eq!(tx2.sender_count(), 1);
}

#[tokio::main]
async fn main() {
    // change().await;
    // mark_change().await;
    // has_change().await;

    clone_sender_exp().await;
}
