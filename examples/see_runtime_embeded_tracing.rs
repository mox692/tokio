use tracing::{debug, trace, Subscriber};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    fmt::{self, format::Format, FormatEvent, FormatFields},
    layer::SubscriberExt,
    registry::LookupSpan,
    EnvFilter, Registry,
};

#[tokio::main]
async fn main() {
    let worker_log_rolling_appender = RollingFileAppender::builder()
        // 1時間ごとにログファイルをローテーション
        .rotation(Rotation::HOURLY)
        // ファイル名のプレフィックス
        .filename_prefix("worker_log")
        // 24時間分のログファイルを保持
        .max_log_files(24)
        .build("logs")
        .unwrap();

    // let (worker_log_non_blocking, _worker_log_guard) = NonBlockingBuilder::default()
    //     // バッファがいっぱいになった時にログの欠損を許容しない
    //     .lossy(false)
    //     .thread_name("thread")
    //     .finish(worker_log_rolling_appender);

    let filter = EnvFilter::from_default_env().add_directive(
        "tokio::runtime::scheduler::multi_thread::worker::worker_log"
            .parse()
            .unwrap(),
    );

    let trace_layer_stdout = fmt::layer().with_writer(std::io::stdout);
    let trace_layer_file = fmt::layer()
        // .event_format(JsonLogFormatter)
        .event_format(Format::default().json())
        .with_ansi(false)
        .with_writer(worker_log_rolling_appender);

    let subscriber = Registry::default()
        .with(filter)
        .with(trace_layer_stdout)
        .with(trace_layer_file);

    tracing::subscriber::set_global_default(subscriber).expect("Unable to set global subscriber");

    debug!("this is a tracing line");

    trace!(
        target: "tokio::runtime::scheduler::multi_thread::worker::worker_log",
        id = 3,
        typ = "Start",
    );
    trace!(
        target: "tokio::runtime::scheduler::multi_thread::worker::worker_log",
        id = 1,
        typ = "Park",
    );
    trace!(
        target: "tokio::runtime::scheduler::multi_thread::worker::worker_log",
        id = 2,
        typ = "RunTask",
        task_id = 33,
        backtrace = "╼\u{a0}worker_trace::basic::{{closure}}::{{closure}} at /root/work/tokio/examples/worker_trace.rs:76:19\n\u{a0}\u{a0}└╼\u{a0}worker_trace::foo::{{closure}} at /root/work/tokio/examples/worker_trace.rs:54:11\n\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}└╼\u{a0}worker_trace::bar::{{closure}} at /root/work/tokio/examples/worker_trace.rs:59:11\n\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}└╼\u{a0}worker_trace::baz::{{closure}} at /root/work/tokio/examples/worker_trace.rs:64:60\n\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}└╼\u{a0}<tokio::time::sleep::Sleep as core::future::future::Future>::poll at /root/work/tokio/tokio/src/time/sleep.rs:448:22\n\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}└╼\u{a0}tokio::time::sleep::Sleep::poll_elapsed at /root/work/tokio/tokio/src/time/sleep.rs:404:16"
    );
    trace!(
        target: "tokio::runtime::scheduler::multi_thread::worker::worker_log",
        id = 6,
        typ = "RunTask",
        task_id = 10,
        backtrace = "╼\u{a0}worker_trace::basic::{{closure}}::{{closure}} at /root/work/tokio/examples/worker_trace.rs:76:19\n\u{a0}\u{a0}└╼\u{a0}worker_trace::foo::{{closure}} at /root/work/tokio/examples/worker_trace.rs:54:11\n\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}└╼\u{a0}worker_trace::bar::{{closure}} at /root/work/tokio/examples/worker_trace.rs:59:11\n\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}└╼\u{a0}worker_trace::baz::{{closure}} at /root/work/tokio/examples/worker_trace.rs:64:60\n\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}└╼\u{a0}<tokio::time::sleep::Sleep as core::future::future::Future>::poll at /root/work/tokio/tokio/src/time/sleep.rs:448:22\n\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}└╼\u{a0}tokio::time::sleep::Sleep::poll_elapsed at /root/work/tokio/tokio/src/time/sleep.rs:404:16"
    );
}

///
/// if we want to customize the log format, these code can be used
///
use tracing::field::{Field, Visit};
struct WorkerLogEventBuf<'a> {
    buf: &'a mut String,
}
impl<'a> Visit for WorkerLogEventBuf<'a> {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.buf
            .push_str(format!("field: {:?}, value: {:?}\n", field.name(), value).as_str());
    }
}

struct JsonLogFormatter;

impl<S, N> FormatEvent<S, N> for JsonLogFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &fmt::FmtContext<'_, S, N>,
        mut writer: fmt::format::Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        for f in event.fields() {
            println!("{}", f);
        }

        let mut buf = String::new();
        let mut root_span_entry = WorkerLogEventBuf { buf: &mut buf };
        event.record(&mut root_span_entry);

        write!(&mut writer, "{}", buf)
    }
}
