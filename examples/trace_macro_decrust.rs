use tracing::trace;

/*

trace -> event
*/
fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    trace!("hello");
    runtime.block_on(async { println!("hello!") });
}
