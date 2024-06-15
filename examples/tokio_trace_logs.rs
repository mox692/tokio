use std::time::Duration;

use tokio::time::sleep;
use tracing::{Span, Subscriber};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    fmt::{self, format::Format, FormatEvent},
    layer::SubscriberExt,
    registry::LookupSpan,
    Registry,
};

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let worker_log_rolling_appender = RollingFileAppender::builder()
        .rotation(Rotation::HOURLY)
        .filename_prefix("tracing_log")
        .max_log_files(24)
        .build("logs")
        .unwrap();

    let trace_layer_stdout = fmt::layer().with_writer(std::io::stdout);
    let trace_layer_file = fmt::layer()
        // .event_format(JsonLogFormatter)
        // .event_format(Format::default().json())
        .with_ansi(false)
        .with_writer(worker_log_rolling_appender);

    let subscriber = Registry::default()
        .with(trace_layer_stdout)
        .with(trace_layer_file);

    tracing::subscriber::set_global_default(subscriber).expect("Unable to set global subscriber");

    rt.block_on(async {
        sleep(Duration::from_secs(1)).await;

        // spawn another named task.
        let jh = tokio::task::Builder::new()
            .spawn(async {
                println!("hello!");
                let current_span = tracing::Span::current();
                println!(
                    "@@@@@@@@@@@@@@ task creating!!, current_span: {:?}",
                    &current_span
                );
                sleep(Duration::from_secs(1)).await;
            })
            .unwrap();

        jh.await.unwrap();

        sleep(Duration::from_secs(1)).await;
    });
}
