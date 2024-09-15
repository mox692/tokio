use std::{
    future::Future,
    sync::atomic::{AtomicUsize, Ordering},
    task::Poll,
};

use tokio::task::JoinSet;
use tracing::{event, span, Level};

fn main() {
    use tracing_perfetto::PerfettoLayer;
    use tracing_subscriber::{prelude::*, registry::Registry};

    let layer = PerfettoLayer::new(std::sync::Mutex::new(
        std::fs::File::create("./test.pftrace").unwrap(),
    ))
    .with_filter_by_marker(|field_name| field_name == "data")
    .with_debug_annotations(true);

    // let filter = EnvFilter::from_default_env().add_directive("flihgt_recorder".parse().unwrap());
    tracing_subscriber::registry()
        // .with(filter)
        .with(layer)
        .init();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name_fn(|| {
            static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
            let id = ATOMIC_ID.fetch_add(1, Ordering::SeqCst);
            format!("tokio-runtime-worker-{}", id)
        })
        .build()
        .unwrap();

    rt.block_on(async {
        // println!("hello");
        // cpu_task().await;
        // yield_task().await
        // handmade_future().await
        sleep_program().await;
    });
}

async fn cpu_task() {
    let mut handles = vec![];
    for i in 0..10000 {
        handles.push(tokio::task::spawn(async move {
            // println!("Worker {} is starting work.", i);
            let mut counter = 0u64;

            // CPU負荷をかけるために無限ループで計算
            loop {
                counter = counter.wrapping_add(1);
                if counter > 1_000 {
                    break;
                }
            }

            ()
        }));
    }

    // スレッドが終了しないようにするために待機
    for handle in handles {
        let _ = handle.await;
    }
}

async fn yield_task() {
    for i in 0..10000 {
        tokio::task::yield_now().await;
    }
}

struct MyFuture {
    count: usize,
}
impl Future for MyFuture {
    type Output = ();
    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        for i in 0..100_000_000 {
            let _ = i;
        }
        if self.count > 10 {
            Poll::Ready(())
        } else {
            let this = self.get_mut();
            this.count += 1;
            let w = cx.waker().clone();
            w.wake();
            Poll::Pending
        }
    }
}

async fn handmade_future() {
    let fut = MyFuture { count: 0 };
    fut.await;
}

#[inline(never)]
async fn foo(i: i32) {
    bar(i).await
}
#[inline(never)]
async fn bar(i: i32) {
    baz(i).await
}
#[inline(never)]
async fn baz(i: i32) {
    tokio::time::sleep(std::time::Duration::from_micros(10 * (i as u64))).await;
}

async fn sleep_program() {
    let mut set = JoinSet::new();

    for i in 0..10 {
        set.spawn(async move {
            // tokio::time::sleep(std::time::Duration::from_micros(i * 10)).await;
            foo(i).await
        });
    }

    while let Some(res) = set.join_next().await {
        res.unwrap()
    }
}
// fn main() {
//     use tracing_perfetto::PerfettoLayer;
//     use tracing_subscriber::{prelude::*, registry::Registry};

//     let layer = PerfettoLayer::new(std::sync::Mutex::new(
//         std::fs::File::create("./test.pftrace").unwrap(),
//     ))
//     .with_filter_by_marker(|f| f == "perfetto");

//     tracing_subscriber::registry().with(layer).init();

//     let span = span!(Level::TRACE, "my span");
//     let _enter = span.enter();

//     let jh = std::thread::spawn(|| {
//         let span = span!(Level::TRACE, "my span33");

//         std::thread::sleep(std::time::Duration::from_secs(1));
//         // tracing::trace!(target = "flihgt_recorder", data = "close!");
//         tracing::trace!(name: "completed", meta = "aa");

//         let _enter = span.enter();
//         bar();
//         drop(_enter);
//     });

//     foo();
//     std::thread::sleep(std::time::Duration::from_secs(1));
//     drop(_enter);
//     jh.join().unwrap();
// }
