#![cfg(unix)]

use tokio::task::JoinSet;
use tokio::time::Instant;
use tokio_stream::StreamExt;

use tokio::fs::{self, File};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::codec::{BytesCodec, FramedRead /*FramedWrite*/};

use criterion::{criterion_group, criterion_main, Criterion, SamplingMode};

use std::fs::File as StdFile;
use std::io::Read as StdRead;

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .build()
        .unwrap()
}

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
const TMP_FILE: &str = "./benches/tmp";
const TMP_DIR: &str = "./benches/tmp";
const DEV_NULL: &str = "/dev/null";

const BUFFER_SIZE: usize = 4096;

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

fn my_file_write(c: &mut Criterion) {
    let rt = rt();

    async fn file() {
        let mut set = JoinSet::new();
        let mut files = vec![];
        for _ in 0..TASK_COUNT_1K {
            let file = File::create(DEV_NULL).await.unwrap();
            files.push(file);
        }
        for mut file in files.into_iter() {
            set.spawn(async move {
                let buf = vec![77; DATA_SIZE_1M];
                file.write_all(&buf[..]).await.unwrap();
                file.flush().await.unwrap();
            });
        }
        while let Some(res) = set.join_next().await {
            res.unwrap();
        }
    }

    async fn fs() {
        let mut set = JoinSet::new();
        let mut files = vec![];
        for _ in 0..TASK_COUNT_1K {
            let file = File::create(DEV_NULL).await.unwrap();
            files.push(file);
        }
        for _ in 0..TASK_COUNT_1K {
            set.spawn(async move {
                let buf = vec![77; DATA_SIZE_1M];
                fs::write(DEV_NULL, buf).await.unwrap();
            });
            let _ = files.pop().unwrap();
        }
        while let Some(res) = set.join_next().await {
            res.unwrap();
        }
    }

    c.bench_function("my_file_write", |b| {
        b.iter(|| {
            rt.block_on(async {
                file().await;
                // fs().await;
            });
        });
    });
}

fn my_file_read(c: &mut Criterion) {
    let rt = rt();
    async fn create_file() {
        for i in 0..TASK_COUNT_1K {
            let file_path = format!("{}_{i}.txt", TMP_DIR);
            File::create(file_path.clone()).await.unwrap();
            let buf = vec![77; DATA_SIZE_1M];
            fs::write(file_path, buf).await.unwrap();
        }
    }

    fn delete_file() {
        std::fs::create_dir_all(TMP_DIR).unwrap();
    }

    async fn file() {
        let mut set = JoinSet::new();
        let mut files = vec![];

        for i in 0..TASK_COUNT_1K {
            let file_path = format!("{}_{i}.txt", TMP_DIR);
            let file = File::open(file_path).await.unwrap();
            files.push(file);
        }
        for mut file in files.into_iter() {
            set.spawn(async move {
                let mut buf = vec![77; DATA_SIZE_1M];
                file.read_exact(&mut buf[..]).await.unwrap()
            });
        }
        while let Some(res) = set.join_next().await {
            res.unwrap();
        }
    }
    async fn fs() {
        let mut set = JoinSet::new();
        let mut files = vec![];

        for i in 0..TASK_COUNT_1K {
            let file_path = format!("{}_{i}.txt", TMP_DIR);
            let file = File::open(file_path).await.unwrap();
            files.push(file);
        }
        for i in 0..TASK_COUNT_1K {
            let file_path = format!("{}_{i}.txt", TMP_DIR);
            set.spawn(async move {
                fs::read(file_path).await.unwrap();
            });
            files.pop().unwrap();
        }

        while let Some(res) = set.join_next().await {
            res.unwrap();
        }
    }

    delete_file();
    rt.block_on(create_file());

    c.bench_function("my_file_read", |b| {
        b.iter(|| {
            rt.block_on(async {
                file().await;
                // fs().await;
            });
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

criterion_group!(
    file,
    async_read_std_file,
    async_read_buf,
    async_read_codec,
    sync_read,
    my_file_write,
    my_file_read
);
criterion_main!(file);

// Benchmarking my_file_write: Collecting 100 samples in estimated 86.807 s (100 iterations)
// my_file_write           time:   [843.04 ms 853.69 ms 864.22 ms]
// Found 1 outliers among 100 measurements (1.00%)

// my_file_write           time:   [296.13 ms 302.24 ms 308.44 ms]
//                         change: [-65.354% -64.596% -63.631%] (p = 0.00 < 0.05)
//                         Performance has improved.
