use std::{sync::Arc, time::Instant};
use tokio::{runtime::Runtime, sync::Semaphore};

fn rt(num_threads: usize) -> Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(num_threads)
        .build()
        .unwrap()
}

fn main() {
    let num_threads = 10;
    let num_tasks = 10;
    let msgs_per_task = 1_000;
    let sem_cap = 100000;

    let rt = rt(num_threads);

    rt.block_on(async {
        let start = Instant::now();
        let mut jhs = Vec::new();
        let semaphore = Arc::new(Semaphore::new(sem_cap));
        for j in 0..num_tasks {
            for k in 0..msgs_per_task {
                let semaphore = semaphore.clone();
                let jh = tokio::spawn(async move {
                    // Acquire permit before sending request.
                    let _permit = semaphore.try_acquire().unwrap();
                    // let _permit = semaphore.acquire().await.unwrap();
                    // Send the request.
                    // Drop the permit after the request has been sent.
                    drop(_permit);
                    // Handle response.
                });
                jhs.push(jh);
            }
        }

        for jh in jhs {
            let _ = jh.await.unwrap();
        }
        let time = start.elapsed();
        println!("time: {:?}", &time);
    });
}
