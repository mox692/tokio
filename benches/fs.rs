#![cfg(unix)]

use tokio::task::JoinSet;
use tokio_stream::StreamExt;

use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio_util::codec::{BytesCodec, FramedRead /*FramedWrite*/};

use criterion::{criterion_group, criterion_main, Criterion};

use std::fs::File as StdFile;
use std::io::Read as StdRead;
use std::time::Instant;

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
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

fn process_many_files(c: &mut Criterion) {
    let rt = rt();
    c.bench_function("process_many_files", |b| {
        b.iter_custom(|iters| {
            const NUM_FILES: usize = 32;
            const FILE_SIZE: usize = 64;

            use rand::Rng;
            use std::io::Write;
            use tempfile::NamedTempFile;

            let mut files = Vec::with_capacity(NUM_FILES);
            for _ in 0..NUM_FILES {
                let mut tmp = NamedTempFile::new().unwrap();
                let mut data = vec![0u8; FILE_SIZE];
                rand::thread_rng().fill(&mut data[..]);
                tmp.write_all(&data).unwrap();
                let path = tmp.path().to_path_buf();
                files.push((tmp, data, path));
            }

            rt.block_on(async move {
                let mut set = JoinSet::new();
                let start = Instant::now();
                for (tmp, original, path) in files {
                    set.spawn(async move {
                        let _keep_alive = tmp;

                        for _ in 0..iters {
                            let mut file = tokio::fs::OpenOptions::new()
                                .read(true)
                                .open(&path)
                                .await
                                .unwrap();
                            let mut buf = vec![0u8; FILE_SIZE];

                            file.read_exact(&mut buf).await.unwrap();

                            assert_eq!(buf, original);
                        }
                    });
                }
                while let Some(Ok(_)) = set.join_next().await {}

                start.elapsed()
            })
        })
    });
}

criterion_group!(
    file,
    async_read_std_file,
    async_read_buf,
    async_read_codec,
    sync_read,
    process_many_files
);
criterion_main!(file);
