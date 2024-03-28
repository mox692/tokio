use std::sync::Arc;

use futures::stream;
use tokio::{
    fs::{self, File},
    io::{AsyncReadExt, AsyncWriteExt},
    task::JoinSet,
    time::Instant,
};

const BLOCK_COUNT: usize = 1;

const DATA_SIZE_1G: usize = 1024 * 1024 * 1024;
const DATA_SIZE_1M: usize = 1024 * 1024;
const DATA_SIZE_1K: usize = 1024;

const TASK_COUNT_1K: usize = 1000;
const TASK_COUNT_10K: usize = 1000 * 10;
const TASK_COUNT_1M: usize = 1000 * 1000;

const DEV_ZERO: &str = "/dev/zero";
const TMP_FILE: &str = "./examples/tmp/foo";
const TMP_DIR: &str = "./examples/tmp/";
const DEV_NULL: &str = "/dev/null";

#[tokio::main]
async fn main() {
    write_many_task().await;
}

async fn read() {
    let mut file = File::open(DEV_ZERO).await.unwrap();
    let mut buffer = [0u8; DATA_SIZE_1M];

    let now = Instant::now();
    for _i in 0..BLOCK_COUNT {
        let count = file.read(&mut buffer).await.unwrap();
        if count == 0 {
            break;
        }
    }

    println!("time: {}", now.elapsed().as_millis());
}

async fn write() {
    let now = Instant::now();
    for _i in 0..BLOCK_COUNT {
        let mut file = File::create(TMP_FILE).await.unwrap();
        file.set_max_buf_size(1024);
        let buf = vec![77; DATA_SIZE_1G];
        file.write_all(&buf[..]).await.unwrap();
    }

    println!("time: {}", now.elapsed().as_millis());
}

async fn write_many_task() {
    std::fs::remove_dir_all(TMP_DIR).unwrap();
    std::fs::create_dir_all(TMP_DIR).unwrap();

    let mut set = JoinSet::new();

    let mut files = vec![];

    for i in 0..TASK_COUNT_10K {
        let file = File::create(format!("{}_{}", TMP_DIR, i)).await.unwrap();
        files.push(file);
    }

    let now = Instant::now();
    for mut file in files.into_iter() {
        set.spawn(async move {
            let buf = vec![77; DATA_SIZE_1K];
            file.write_all(&buf[..]).await.unwrap();
        });
    }

    while let Some(res) = set.join_next().await {
        res.unwrap();
    }

    println!("time: {}", now.elapsed().as_millis());
}

async fn write_many_task_fs_write() {
    std::fs::remove_dir_all(TMP_DIR).unwrap();
    std::fs::create_dir_all(TMP_DIR).unwrap();

    let mut set = JoinSet::new();

    let now = tokio::time::Instant::now();
    for i in 0..TASK_COUNT_10K {
        set.spawn(async move {
            let buf = vec![77; DATA_SIZE_1K];
            fs::write(format!("{}_{}", TMP_DIR, i), buf).await.unwrap();
        });
    }

    while let Some(res) = set.join_next().await {
        res.unwrap();
    }

    println!("time: {}", now.elapsed().as_millis());
}
