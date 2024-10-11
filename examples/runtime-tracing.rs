use std::sync::atomic::{AtomicUsize, Ordering};
use tracing_perfetto::PerfettoLayer;
use tracing_subscriber::prelude::*;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn main() {
    let layer = PerfettoLayer::new(std::sync::Mutex::new(
        std::fs::File::create("./test.pftrace").unwrap(),
    ))
    .with_filter_by_marker(|field_name| field_name == "tokio_runtime_event")
    .with_debug_annotations(true);

    tracing_subscriber::registry().with(layer).init();

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
        tokio::spawn(async { foo().await }).await.unwrap();
    });
}

#[inline(never)]
async fn foo() {
    bar().await
}
#[inline(never)]
async fn bar() {
    baz().await
}
#[inline(never)]
async fn baz() {
    let mut handles = vec![];
    for i in 0..10000 {
        handles.push(tokio::task::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_micros(i * 10)).await;
        }));
    }

    for handle in handles {
        let _ = handle.await;
    }
}
