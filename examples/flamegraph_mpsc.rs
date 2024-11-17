use std::hint::black_box;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::task::JoinSet;
use tokio::{sync::Semaphore, task};

fn multi_rt() -> Runtime {
    tokio::runtime::Builder::new_multi_thread()
        // .worker_threads(100)
        .enable_all()
        .build()
        .unwrap()
}

async fn runnnnnnnnnnnnnn() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<usize>(1_000_000_000_000);

    let mut set = JoinSet::new();
    for _ in 0..10 {
        println!("111111");
        let tx = tx.clone();
        set.spawn(async move {
            println!("22222222222");
            for i in 0..100000000 {
                if i % 10000 == 0 {
                    println!("i: {}", i);
                }
                tx.send(i).await.unwrap();
            }
        });
    }

    println!("iiiiiiiii: {:?}", set.len());
    let mut sum = 0;
    black_box(for _ in 0..100_000000 {
        println!("444444444");
        sum += rx.recv().await.unwrap() % 1000000;
    });
    while let Some(res) = set.join_next().await {
        println!("3333333");
        let _ = res;
    }
}
fn main() {
    let rt = multi_rt();
    rt.block_on(async {
        tokio::spawn(runnnnnnnnnnnnnn()).await;
    });
    println!("end");

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
