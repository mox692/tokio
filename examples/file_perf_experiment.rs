use tokio::{
    fs::{self, File},
    io::{AsyncReadExt, AsyncWriteExt},
    runtime::Builder,
    task::JoinSet,
    time::Instant,
};

const BLOCK_COUNT: usize = 1024;

const DATA_SIZE_1G: usize = 1024 * 1024 * 1024;
const DATA_SIZE_1M: usize = 1024 * 1024;
const DATA_SIZE_1K: usize = 1024;

const TASK_COUNT_1: usize = 1;
const TASK_COUNT_4: usize = 4;
const TASK_COUNT_1K: usize = 1000;
const TASK_COUNT_10K: usize = 1000 * 10;
const TASK_COUNT_100K: usize = 1000 * 100;
const TASK_COUNT_1M: usize = 1000 * 1000;

const DEV_ZERO: &str = "/dev/zero";
const TMP_FILE: &str = "./examples/tmp/foo";
const TMP_DIR: &str = "./examples/tmp/";
const DEV_NULL: &str = "/dev/null";

// #[tokio::main]
// async fn main() {
//     // write().await;
//     write_many_task().await;
//     // write_many_task_fs_write().await;

//     // read_many_task().await;
//     // read_many_task_fs_read().await;
// }

fn main() {
    // custom runtime
    let runtime = Builder::new_multi_thread()
        // .worker_threads(1)
        .thread_name("my-custom-name")
        .thread_stack_size(3 * 1024 * 1024)
        .build()
        .unwrap();

    runtime.block_on(async {
        write_many_task().await;
    });
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
        let mut file = File::create(DEV_NULL).await.unwrap();
        file.set_max_buf_size(1024);
        let buf = vec![77; DATA_SIZE_1K];
        file.write_all(&buf[..]).await.unwrap();
        file.flush().await.unwrap()
    }

    println!("time: {}", now.elapsed().as_millis());
}

async fn read_many_task() {
    // std::fs::remove_dir_all(TMP_DIR).unwrap();
    // println!("rm dir done.");
    // std::fs::create_dir_all(TMP_DIR).unwrap();
    // println!("create dir done.");

    let mut set = JoinSet::new();

    let mut files = vec![];

    for _ in 0..TASK_COUNT_10K {
        let file = File::open(DEV_ZERO).await.unwrap();
        files.push(file);
    }

    let now = Instant::now();
    for mut file in files.into_iter() {
        set.spawn(async move {
            let mut buf = vec![77; DATA_SIZE_1M];
            file.read_exact(&mut buf[..]).await.unwrap()
        });
    }

    while let Some(res) = set.join_next().await {
        res.unwrap();
    }

    println!("time: {}", now.elapsed().as_millis());
}

async fn read_many_task_fs_read() {
    // std::fs::remove_dir_all(TMP_DIR).unwrap();
    // println!("rm dir done.");
    // std::fs::create_dir_all(TMP_DIR).unwrap();
    // println!("create dir done.");

    let mut set = JoinSet::new();

    let now = tokio::time::Instant::now();
    for i in 0..TASK_COUNT_10K {
        set.spawn(async move {
            let mut buf = vec![77; DATA_SIZE_1M];
            // fs::write(DEV_NULL, buf).await.unwrap();
            fs::read(DEV_NULL).await.unwrap()
        });
    }

    while let Some(res) = set.join_next().await {
        res.unwrap();
    }

    println!("time: {}", now.elapsed().as_millis());
}

async fn write_many_task() {
    // std::fs::remove_dir_all(TMP_DIR).unwrap();
    // println!("rm dir done.");
    // std::fs::create_dir_all(TMP_DIR).unwrap();
    // println!("create dir done.");

    let mut set = JoinSet::new();

    let mut files = vec![];

    for _ in 0..TASK_COUNT_1 {
        let file = File::create(DEV_NULL).await.unwrap();
        files.push(file);
    }

    let now = Instant::now();
    for mut file in files.into_iter() {
        set.spawn(async move {
            let buf = vec![77; DATA_SIZE_1K];
            file.write_all(&buf[..]).await.unwrap();
            file.flush().await.unwrap();
        });
    }

    while let Some(res) = set.join_next().await {
        res.unwrap();
    }

    println!("time: {}", now.elapsed().as_millis());
}

async fn write_many_task_fs_write() {
    // std::fs::remove_dir_all(TMP_DIR).unwrap();
    // println!("rm dir done.");
    // std::fs::create_dir_all(TMP_DIR).unwrap();
    // println!("create dir done.");

    let mut set = JoinSet::new();

    let now = tokio::time::Instant::now();
    for i in 0..TASK_COUNT_10K {
        set.spawn(async move {
            let buf = vec![77; DATA_SIZE_1M];
            fs::write(DEV_NULL, buf).await.unwrap();
        });
    }

    while let Some(res) = set.join_next().await {
        res.unwrap();
    }

    println!("time: {}", now.elapsed().as_millis());
}
