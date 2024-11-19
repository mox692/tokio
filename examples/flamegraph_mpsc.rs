use std::hint::black_box;
use tokio::runtime::Runtime;
use tokio::task::JoinSet;

fn multi_rt() -> Runtime {
    tokio::runtime::Builder::new_multi_thread()
        // .worker_threads(100)
        .enable_all()
        .build()
        .unwrap()
}

async fn runnnnnnnnnnnnnn() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<usize>(10000);

    let mut set = JoinSet::new();
    for _ in 0..10 {
        let tx = tx.clone();
        set.spawn(async move {
            for i in 0..100 {
                tx.send(i).await.unwrap();
            }
        });
    }

    let mut sum = 0;
    black_box(for _ in 0..100 {
        rx.recv().await.unwrap();
    });
    while let Some(res) = set.join_next().await {
        let _ = res;
    }
}
fn main() {
    let rt = multi_rt();
    rt.block_on(async {
        tokio::spawn(runnnnnnnnnnnnnn()).await;
    });

    // g.bench_function("concurrent_multi", |b| {
    //     b.iter(|| {
    //         let s = s.clone();
    //         rt.block_on(async move {
    //             let j = tokio::try_join! {
    //                 task::spawn(task(s.clone())),
    //                 task::spawn(task(s.clone())),
    //                 task::spawn(task(s.clone())),
    //                 task::spawn(task(s.clone())),
    //                 task::spawn(task(s.clone())),
    //                 task::spawn(task(s.clone()))
    //             };
    //             j.unwrap();
    //         })
    //     })
    // });
}
