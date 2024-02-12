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

// #[tokio::main]
// pub async fn main() -> Result<(), Box<dyn Error>> {
//     // Open a TCP stream to the socket address.
//     //
//     // Note that this is the Tokio TcpStream, which is fully async.
//     let mut stream = TcpStream::connect("127.0.0.1:6142").await?;
//     println!("created stream");

//     let result = stream.write_all(b"hello world\n").await;
//     println!("wrote to stream; success={:?}", result.is_ok());

//     Ok(())
// }

// tokio::task_local! {
//     static TASK_LOCAL_ALLOCATED_BYTES: AtomicUsize;
// }

// problem
// #[tokio::main]
// async fn main() {
//     let allocated_bytes = AtomicUsize::new(0);
//     let byte = 0;
//     let mut input = stream::iter(vec![Ok(1), Err("")]);

//     TASK_LOCAL_ALLOCATED_BYTES.scope(allocated_bytes, input.try_next())
//     while let Ok(message) = TASK_LOCAL_ALLOCATED_BYTES
//         .scope(allocated_bytes, input.try_next())
//         .await
//     {}
// }

// solve
// #[tokio::main]
// async fn main() {
//     let mut allocated_bytes = Some(AtomicUsize::new(11));
//     let mut input: stream::Iter<std::vec::IntoIter<Result<i32, &str>>> = stream::iter(vec![Ok(1)]);
//     loop {
//         // これは try_nextが返すFutureをwrapしたものを返す. 1つ目の引数は使われていないような気もするが...
//         let mut f =
//             TASK_LOCAL_ALLOCATED_BYTES.scope(allocated_bytes.take().unwrap(), input.try_next());

//         // we can use `take_value` aganist not pinned TaskLocalFuture.
//         let s = f.take_value_unpinned();

//         let mut f = Box::pin(f);

//         let message = (&mut f).await.unwrap();
//         if let Some(message) = message {
//             // yield message;
//             // allocated_bytes = Some(f.into_value());
//             allocated_bytes = Some(AtomicUsize::new(22));
//             // println!("{:?}", (&mut f).await);
//             // println!("{:?}", (&mut f).await);
//             // println!("{:?}", (&mut f).await);

//             let s = (&mut f).as_mut().take_value();
//             println!("s: {:?}", &s);
//             // take_valueはBoxFutureをunwrapして中のvalueを取り出す
//         }
//         break;
//     }
// }

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

#[tokio::main]
async fn main() {
    // change().await;
    // mark_change().await;
    has_change().await;
}
