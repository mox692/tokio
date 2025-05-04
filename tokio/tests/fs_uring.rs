#![cfg(all(
    tokio_unstable_uring,
    feature = "rt",
    feature = "fs",
    target_os = "linux",
))]

use tokio::{
    io::AsyncReadExt,
    runtime::{Builder, Runtime},
};

fn rt(num_workers: usize) -> Runtime {
    Builder::new_multi_thread()
        .worker_threads(num_workers)
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn process_many_files() {
    let rt = rt(2);

    rt.block_on(async {
        const NUM_FILES: usize = 1024;
        const FILE_SIZE: usize = 64;

        use rand::Rng;
        use std::io::Write;
        use tempfile::NamedTempFile;

        // create all the files up front
        let mut files = Vec::with_capacity(NUM_FILES);
        for _ in 0..NUM_FILES {
            let mut tmp = NamedTempFile::new().unwrap();
            let mut data = vec![0u8; FILE_SIZE];
            rand::thread_rng().fill(&mut data[..]);
            tmp.write_all(&data).unwrap();
            let path = tmp.path().to_path_buf();
            files.push((tmp, data, path));
        }

        // spawn one task per file, _moving_ the NamedTempFile into the closure
        let mut handles = Vec::with_capacity(NUM_FILES);
        for (tmp, original, path) in files {
            handles.push(tokio::spawn(async move {
                // by binding `tmp` here (even if unused) we keep it alive
                let _keep_alive = tmp;

                // now the file still exists on disk:
                let mut file = tokio::fs::OpenOptions::new()
                    .read(true)
                    .open(&path)
                    .await
                    .unwrap();
                let mut buf = vec![0u8; FILE_SIZE];

                file.read_exact(&mut buf).await.unwrap();

                assert_eq!(buf, original);
            }));
        }

        // await them all
        for h in handles {
            h.await.unwrap();
        }
    });
}
