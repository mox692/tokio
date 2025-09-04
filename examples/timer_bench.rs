use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use axum::{routing::get, Router};
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tokio_util::future::FutureExt;

#[tokio::main]
async fn main() {
    let jh: JoinHandle<()> = tokio::spawn(async move {
        let app: Router = Router::new().route("/", get(root));

        let listener = tokio::net::TcpListener::bind("0.0.0.0:1111").await.unwrap();
        axum::serve(listener, app).await.unwrap();
    });
    jh.await.unwrap();
}

async fn root() -> &'static str {
    // Here, we would create 10 timers and drop them immediately,
    // causing a lot of timer wheel accesses.
    for _ in 0..10 {
        let _ = ReadyOnSecondPoll::default()
            .timeout(Duration::from_secs(1))
            .await;
    }
    "Hello, World!"
}

#[derive(Debug, Default)]
// 3 implementations
struct ReadyOnSecondPoll {
    counter: usize,
}

impl Future for ReadyOnSecondPoll {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this: &mut ReadyOnSecondPoll = self.get_mut();
        this.counter += 1;

        if this.counter == 2 {
            Poll::Ready(())
        } else {
            cx.waker().clone().wake();
            Poll::Pending
        }
    }
}
