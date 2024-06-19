//! measure an overhead when using tracing crate

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, Registry};

fn single_thread_scheduler_timeout(c: &mut Criterion) {
    c.bench_function("nameee", |b| {
        b.iter(|| {
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

            tracing::subscriber::set_global_default(subscriber)
                .expect("Unable to set global subscriber");
        })
    });
}

criterion_group!(timeout_benchmark, single_thread_scheduler_timeout);

criterion_main!(timeout_benchmark);
