use tracing::{event, span, Level};
use tracing_subscriber::EnvFilter;

fn bar() {
    tokio::time::sleep(std::time::Duration::from_secs(2));
}
fn foo() {
    let span = span!(Level::TRACE, "my span2");
    let _enter = span.enter();
    // tracing::trace!(data = "start");
    std::thread::sleep(std::time::Duration::from_millis(1600));
    tracing::trace!(name: "completed", meta = "aa");
    bar();
    drop(_enter);
}

fn main() {
    use tracing_perfetto::PerfettoLayer;
    use tracing_subscriber::{prelude::*, registry::Registry};

    let layer = PerfettoLayer::new(std::sync::Mutex::new(
        std::fs::File::create("./test.pftrace").unwrap(),
    ))
    .with_filter_by_marker(|f| f == "perfetto");

    let filter = EnvFilter::from_default_env().add_directive("flihgt_recorder".parse().unwrap());

    tracing_subscriber::registry()
        .with(layer)
        .with(filter)
        .init();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {});
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
