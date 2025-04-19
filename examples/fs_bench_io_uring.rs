/// cargo build --release --package examples --example fs_bench_io_uring
/// ./target/release/examples/fs_bench_io_uring
use std::{hint::black_box, time::Instant};
use tokio::{
    fs::{read, OpenOptions},
    task::{yield_now, JoinSet},
};

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        // .worker_threads(1)
        .build()
        .unwrap()
}

fn main() {
    let rt = rt();
    let num_files = 1;
    let mut dur = None;
    let iters = 1;

    let mut set = JoinSet::new();

    rt.block_on(async {
        let start = Instant::now();
        for _ in 0..iters {
            for i in 1..=num_files {
                set.spawn(async move {
                    let path = format!("/home/mox692/work/tokio/test_file/{i}.txt");

                    // let file = OpenOptions::new()
                    //     .read(true)
                    //     .use_io_uring(UringOption::new())
                    //     .open(&path)
                    //     .await
                    //     .unwrap();
                    // black_box(file);

                    let res = read(&path).await.unwrap();
                    // println!("res: {:?}", &res);
                    black_box(res);
                });
            }
        }

        while let Some(Ok(_)) = set.join_next().await {}

        dur = Some(start.elapsed())
    });

    println!("took: {}", dur.unwrap().as_millis());
}
