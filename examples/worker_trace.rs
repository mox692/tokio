//! A simple client that opens a TCP stream, writes "hello world\n", and closes
//! the connection.
//!
//! To start a server that this client can talk to on port 6142, you can use this command:
//!
//!     ncat -l 6142
//!
//! And then in another terminal run:
//!
//!     cargo run --example hello_world

#![warn(rust_2018_idioms)]

use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::spawn;
use tokio::task::JoinSet;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt;
use tracing_subscriber::fmt::format::{FmtSpan, Format, JsonFields};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, Registry};

pub fn main() {
    let worker_log_rolling_appender = RollingFileAppender::builder()
        // 1時間ごとにログファイルをローテーション
        .rotation(Rotation::HOURLY)
        // ファイル名のプレフィックス
        .filename_prefix("worker_log")
        // 24時間分のログファイルを保持
        .max_log_files(24)
        .build("logs")
        .unwrap();

    // TODO: なぜかaysncにするとlogが出ない
    // let (worker_log_non_blocking, _worker_log_guard) = NonBlockingBuilder::default()
    //     // バッファがいっぱいになった時にログの欠損を許容しない
    //     .lossy(false)
    //     .thread_name("thread")
    //     .finish(worker_log_rolling_appender);

    let filter = EnvFilter::from_default_env()
        // directive for target names
        .add_directive(
            "tokio::runtime::scheduler::multi_thread::worker::worker_log=trace"
                .parse()
                .unwrap(),
        )
        // directive for span names
        .add_directive("[span_name_shutdown]=trace".parse().unwrap());

    let trace_layer_stdout = fmt::layer()
        .with_span_events(FmtSpan::ENTER | FmtSpan::EXIT)
        .with_writer(std::io::stdout);

    let json_format = Format::default().with_ansi(false).json();

    let trace_layer_file = fmt::layer()
        .with_span_events(FmtSpan::ENTER | FmtSpan::EXIT)
        // .event_format(JsonLogFormatter)
        // .event_format(Format::default().json())
        .event_format(json_format)
        .fmt_fields(JsonFields::default())
        .with_writer(worker_log_rolling_appender);

    let subscriber = Registry::default()
        .with(filter)
        .with(trace_layer_stdout)
        .with(trace_layer_file);

    tracing::subscriber::set_global_default(subscriber).expect("Unable to set global subscriber");

    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        // basic().await;
        sleep_little_task().await;
    });
}

/// expected event:
/// * RunTask(a, b)
/// * Park(a) // because there are no tasks to run.
/// * RunTask(c, b) // task might run on a different worker.
async fn sleep_little_task() {
    spawn(async {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    })
    .await
    .unwrap();
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
    tokio::time::sleep(std::time::Duration::from_nanos(1)).await;
}

/// expected event:
/// * RunTask(a, b) // sould happen 10 times. (These often seem to be scheduled concentrating on one or two workers.)
async fn basic() {
    let mut set = JoinSet::new();

    let count = Arc::new(AtomicUsize::new(0));
    for _ in 0..10 {
        let count = count.clone();
        let _ = set.spawn(async move {
            foo().await;
            count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        });
    }

    while let Some(s) = set.join_next().await {
        s.unwrap();
    }

    assert_eq!(count.load(std::sync::atomic::Ordering::Relaxed), 10)
}

///
/// if we want to customize the log format, these code can be used
///
use tracing::field::{Field, Visit};
// struct WorkerLogEventBuf<'a> {
//     buf: &'a mut String,
// }
// impl<'a> Visit for WorkerLogEventBuf<'a> {
//     fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
//         self.buf
//             .push_str(format!("field: {:?}, value: {:?}\n", field.name(), value).as_str());
//     }
// }

// struct JsonLogFormatter;

// impl<S, N> FormatEvent<S, N> for JsonLogFormatter
// where
//     S: Subscriber + for<'a> LookupSpan<'a>,
//     N: for<'a> FormatFields<'a> + 'static,
// {
//     fn format_event(
//         &self,
//         ctx: &fmt::FmtContext<'_, S, N>,
//         mut writer: fmt::format::Writer<'_>,
//         event: &tracing::Event<'_>,
//     ) -> std::fmt::Result {
//         for f in event.fields() {
//             println!("{}", f);
//         }

//         let mut buf = String::new();
//         let mut root_span_entry = WorkerLogEventBuf { buf: &mut buf };
//         event.record(&mut root_span_entry);

//         write!(&mut writer, "{}", buf)
//     }
// }
