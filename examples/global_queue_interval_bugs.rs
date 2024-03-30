use std::time::Duration;

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .global_queue_interval(1)
        .build()
        .unwrap();

    rt.block_on(async {})
}
