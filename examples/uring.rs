//! deadlock reproducer.
//!
//! $ cargo build --package examples --example uring &&  strace -f -e trace=all -s 256 -o strace.log ./target/debug/examples/uring
//!
//!
//! ### Notes
//!
//! * Cで再現しないやつとの差分を減らす
//!     * liburingの io_uring_peek_cqe()が, overflowからcqeに戻すみたいなことをやってるかも
//!
//!

use std::path::PathBuf;
use tempfile::NamedTempFile;
use tokio::{
    fs::OpenOptions,
    runtime::{Builder, Runtime},
    task::JoinSet,
};

fn rt(worker_threads: usize) -> Runtime {
    if worker_threads == 1 {
        let mut builder = Builder::new_current_thread();
        #[cfg(tokio_uring)]
        {
            builder.enable_uring();
        }
        builder.build().unwrap()
    } else {
        let mut builder = Builder::new_multi_thread();
        builder.worker_threads(worker_threads);
        #[cfg(tokio_uring)]
        {
            builder.enable_uring();
        }
        builder.build().unwrap()
    }
}

fn create_tmp_files(num_files: usize) -> (Vec<NamedTempFile>, Vec<PathBuf>) {
    let mut files = Vec::with_capacity(num_files);
    for _ in 0..num_files {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        files.push((tmp, path));
    }

    files.into_iter().unzip()
}

fn process_many_files2() {
    const NUM_FILES: usize = 512;

    let iter = 1;

    let (_tmp_files, paths): (Vec<NamedTempFile>, Vec<PathBuf>) = create_tmp_files(NUM_FILES);
    for _i in 0..iter {
        for &threads in &[1] {
            // for (idx, threads) in [8, 16, 8, 16, 8, 16].into_iter().enumerate() {
            let rt = rt(threads);

            let paths = paths.clone();
            rt.block_on(async {
                tokio::spawn(async move {
                    let mut set = JoinSet::new();

                    for i in 0..100 {
                        let path = paths.get(i % NUM_FILES).unwrap().clone();
                        set.spawn(async move {
                            let _file = OpenOptions::new().read(true).open(path).await.unwrap();
                        });
                    }
                    let mut i = 0;
                    while let Some(Ok(_)) = set.join_next().await {
                        i += 1;
                        // if i % 100 == 0 {
                        println!("Completed {} tasks", i);
                        // }
                    }
                })
                .await
                .unwrap();
            })
        }
    }
}

fn main() {
    process_many_files2();
}
