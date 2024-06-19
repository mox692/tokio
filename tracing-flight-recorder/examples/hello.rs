use tracing::trace;
use tracing_flight_recorder::create_large_shirt;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() {
    // initialize buffers, etc
    let layer = tracing_flight_recorder::Layer::new();

    tracing_subscriber::registry().with(layer).init();

    let jh = std::thread::spawn(|| {
        // logging
        // loop {
        //     // targetとdataを指定する
        //     trace!(target = "flihgt_recorder", data = "1111");
        // }
    });

    trace!(target = "flihgt_recorder", data = "1111");

    jh.join().unwrap();

    let s = create_large_shirt();
    println!("s: {:?}", &s);
}
