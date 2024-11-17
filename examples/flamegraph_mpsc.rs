use std::hint::black_box;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{reclaim_called_count, reuse_failed_count};
use tokio::task::JoinSet;

fn multi_rt() -> Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}

async fn run() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<usize>(100_000);

    let mut set = JoinSet::new();
    for _ in 0..10 {
        let tx = tx.clone();
        set.spawn(async move {
            for i in 0..100_ {
                tx.send(i).await.unwrap();
            }
        });
    }

    black_box(for _ in 0..100_0 {
        rx.recv().await.unwrap();
    });
    while let Some(res) = set.join_next().await {
        let _ = res;
    }
}
fn main() {
    let rt = multi_rt();
    rt.block_on(async {
        tokio::spawn(async {
            run().await;
        })
        .await
        .unwrap();
    });
    println!("{} / {}", reuse_failed_count(), reclaim_called_count())
}
