/// cargo build --release --package examples --example fs_bench_io_uring
/// ./target/release/examples/fs_bench_io_uring
use std::{hint::black_box, time::Instant};
use tokio::{
    fs::{read, read3, OpenOptions},
    task::JoinSet,
};

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(8)
        .build()
        .unwrap()
}

fn main() {
    let rt = rt();
    let num_files = 10;
    let mut dur = None;
    let iters = 1;

    rt.block_on(async {
        let mut set = JoinSet::new();
        let start = Instant::now();
        for _ in 0..iters {
            for i in 1..=num_files {
                set.spawn(async move {
                    let path = format!("/home/mox692/work/tokio/test_file/{i}.txt");

                    let file = OpenOptions::new().read(true).open3(&path).await.unwrap();
                    black_box(file);

                    // let res = read3(&path).await.unwrap();
                    // black_box(res);
                });
            }
        }

        while let Some(h) = set.join_next().await {
            h.unwrap()
        }

        dur = Some(start.elapsed())
    });

    println!("took: {}", dur.unwrap().as_millis());
}
