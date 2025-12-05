#![cfg(unix)]

use tokio::task::{spawn_blocking, JoinSet};
use tokio_stream::StreamExt;

use tokio::fs::File;
use tokio::io::{AsyncReadExt, Join};
use tokio_util::codec::{BytesCodec, FramedRead /*FramedWrite*/};

use criterion::{criterion_group, criterion_main, Criterion};

use std::fs::File as StdFile;
use std::io::Read as StdRead;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::time::Duration;
use std::vec;

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .build()
        .unwrap()
}

const BLOCK_COUNT: usize = 1_000;

const BUFFER_SIZE: usize = 4096;
const DEV_ZERO: &str = "/dev/zero";

fn async_read_codec(c: &mut Criterion) {
    let rt = rt();

    c.bench_function("async_read_codec", |b| {
        b.iter(|| {
            let task = || async {
                let file = File::open(DEV_ZERO).await.unwrap();
                let mut input_stream =
                    FramedRead::with_capacity(file, BytesCodec::new(), BUFFER_SIZE);

                for _i in 0..BLOCK_COUNT {
                    let _bytes = input_stream.next().await.unwrap();
                }
            };

            rt.block_on(task());
        })
    });
}

fn async_read_buf(c: &mut Criterion) {
    let rt = rt();

    c.bench_function("async_read_buf", |b| {
        b.iter(|| {
            let task = || async {
                let mut file = File::open(DEV_ZERO).await.unwrap();
                let mut buffer = [0u8; BUFFER_SIZE];

                for _i in 0..BLOCK_COUNT {
                    let count = file.read(&mut buffer).await.unwrap();
                    if count == 0 {
                        break;
                    }
                }
            };

            rt.block_on(task());
        });
    });
}

fn spawn_blocking_many(c: &mut Criterion) {
    let rt = rt();

    let spawn_count = 10_000;
    c.bench_function("spawn_blocking_many", |b| {
        b.iter(|| {
            rt.block_on(async {
                let count = Arc::new(AtomicUsize::new(0));
                let mut vec = vec![];
                let mut set = JoinSet::new();
                for _ in 0..spawn_count {
                    let count = count.clone();
                    let h = set.spawn_blocking(move || {
                        count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        std::thread::sleep(Duration::from_millis(100))
                    });
                    vec.push(h);
                }
                while let Some(v) = set.join_next().await {
                    v.unwrap();
                }
                assert_eq!(
                    count.load(std::sync::atomic::Ordering::Relaxed),
                    spawn_count
                );
            });
        });
    });
}

fn many_spawn_blocking_thread(c: &mut Criterion) {
    const BLOCK_COUNT: usize = 1024;
    const DATA_SIZE_1G: usize = 1024 * 1024 * 1024;
    const DATA_SIZE_1M: usize = 1024 * 1024;
    const DATA_SIZE_1K: usize = 1024;
    const TASK_COUNT_1: usize = 1;
    const TASK_COUNT_4: usize = 4;
    const TASK_COUNT_1K: usize = 1000;
    const TASK_COUNT_8K: usize = 8000;
    const TASK_COUNT_10K: usize = 1000 * 10;
    const TASK_COUNT_100K: usize = 1000 * 100;
    const TASK_COUNT_1M: usize = 1000 * 1000;
    const DEV_ZERO: &str = "/dev/zero";
    const TMP_FILE: &str = "./examples/tmp/foo";
    const TMP_DIR: &str = "./examples/tmp/";
    const DEV_NULL: &str = "/dev/null";

    let rt = rt();

    c.bench_function("many_spawn_blocking_thread", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut set = JoinSet::new();
                let mut files = vec![];

                for _ in 0..TASK_COUNT_1K {
                    let file = File::open(DEV_ZERO).await.unwrap();
                    files.push(file);
                }
                for mut file in files.into_iter() {
                    set.spawn(async move {
                        let mut buf = vec![77; DATA_SIZE_1K];
                        file.read_exact(&mut buf[..]).await.unwrap()
                    });
                }

                while let Some(res) = set.join_next().await {
                    res.unwrap();
                }
            })
        });
    });
}

fn async_read_std_file(c: &mut Criterion) {
    let rt = rt();

    c.bench_function("async_read_std_file", |b| {
        b.iter(|| {
            let task = || async {
                let mut file =
                    tokio::task::block_in_place(|| Box::pin(StdFile::open(DEV_ZERO).unwrap()));

                for _i in 0..BLOCK_COUNT {
                    let mut buffer = [0u8; BUFFER_SIZE];
                    let mut file_ref = file.as_mut();

                    tokio::task::block_in_place(move || {
                        file_ref.read_exact(&mut buffer).unwrap();
                    });
                }
            };

            rt.block_on(task());
        });
    });
}

fn sync_read(c: &mut Criterion) {
    c.bench_function("sync_read", |b| {
        b.iter(|| {
            let mut file = StdFile::open(DEV_ZERO).unwrap();
            let mut buffer = [0u8; BUFFER_SIZE];

            for _i in 0..BLOCK_COUNT {
                file.read_exact(&mut buffer).unwrap();
            }
        })
    });
}

// criterion_group!(
//     file,
//     // async_read_std_file,
//     // async_read_buf,
//     // many_spawn_blocking_thread,
//     many_spawn_blocking_thread2,
//     // async_read_codec,
//     // sync_read,
//     // spawn_blocking_many
// );
// criterion_main!(file);

use std::hint::black_box;
use tango_bench::{benchmark_fn, tango_benchmarks, tango_main, IntoBenchmarks};

pub fn many_spawn_blocking_thread2() {
    let rt = rt();

    // c.bench_function("many_spawn_blocking_thread2", |b| {
    //     b.iter(|| {
    rt.block_on(async {
        let mut v = vec![];
        for _ in 0..5000 {
            let res = spawn_blocking(|| for _ in 0..10 {});
            v.push(res);
        }

        for j in v {
            j.await.unwrap();
        }
    })
    //     });
    // });
}

fn factorial_benchmarks() -> impl IntoBenchmarks {
    [benchmark_fn("many_spawn_blocking_thread2", |b| {
        b.iter(|| many_spawn_blocking_thread2())
    })]
}

tango_benchmarks!(factorial_benchmarks());
tango_main!();
