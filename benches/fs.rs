#![allow(
    unused_imports,
    dead_code,
    unreachable_code,
    unreachable_pub,
    unused_variables
)]
#![cfg(unix)]

use tokio::task::JoinSet;
use tokio_stream::StreamExt;

use tokio::fs::{read, File, OpenOptions, UringOption};
use tokio::io::AsyncReadExt;
use tokio_util::codec::{BytesCodec, FramedRead /*FramedWrite*/};

use criterion::{criterion_group, criterion_main, Criterion};

use std::fs::File as StdFile;
use std::hint::black_box;
use std::io::Read as StdRead;
use std::time::{Duration, Instant};

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(8)
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

fn open_read_spawn_blocking(c: &mut Criterion) {
    let rt = rt();
    let num_files = 100;

    c.bench_function("open_read_spawn_blocking", |b| {
        b.iter_custom(|iters| {
            let mut dur = None;
            rt.block_on(async {
                let start = Instant::now();
                let mut set = JoinSet::new();
                for _ in 0..iters {
                    for i in 1..=num_files {
                        set.spawn(async move {
                            let path = format!("/home/mox692/work/tokio/test_file/{i}.txt");

                            let file = OpenOptions::new().read(true).open(&path).await.unwrap();
                            black_box(file);

                            // let res = read(&path).await.unwrap();
                            // black_box(res);
                        });
                    }
                }

                while let Some(Ok(_)) = set.join_next().await {}

                dur = Some(start.elapsed())
            });

            dur.unwrap()
        })
    });
}
fn open_read_io_uring(c: &mut Criterion) {
    let rt = rt();
    let num_files = 100;
    c.bench_function("open_read_io_uring", |b| {
        b.iter_custom(|iters| {
            let mut dur = None;
            rt.block_on(async {
                let start = Instant::now();
                let mut set = JoinSet::new();
                for _ in 0..iters {
                    for i in 1..=num_files {
                        set.spawn(async move {
                            let path = format!("/home/mox692/work/tokio/test_file/{i}.txt");

                            let file = OpenOptions::new().read(true).open(&path).await.unwrap();
                            black_box(file);

                            // let res = read3(&path).await.unwrap();
                            // black_box(res);
                        });
                    }
                }

                while let Some(Ok(_)) = set.join_next().await {}

                dur = Some(start.elapsed())
            });

            dur.unwrap()
        })
    });
}

criterion_group!(
    file,
    // async_read_std_file,
    // async_read_buf,
    // async_read_codec,
    // sync_read
    open_read_io_uring,
    open_read_spawn_blocking,
);
criterion_main!(file);
