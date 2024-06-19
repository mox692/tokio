//! An experiment tracing subscriber for continuous tracing for tokio
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/tokio-rs/tracing/master/assets/logo-type.png",
    html_favicon_url = "https://raw.githubusercontent.com/tokio-rs/tracing/master/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/tokio-rs/tracing/issues/"
)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    unreachable_pub,
    dead_code
)]

use std::{
    cell::RefCell,
    io::Write,
    sync::{
        atomic::AtomicBool,
        mpsc::{channel, Receiver, Sender},
        RwLock,
    },
};
use tracing::{event, field::Visit, Subscriber};
use tracing_subscriber::registry::LookupSpan;

pub(crate) mod ring_buffer;

#[derive(Debug)]
struct ThreadLocalBuffer {
    sender: std::sync::mpsc::Sender<Box<[u8]>>,
    bytes: [u8; DEFAULT_BUF_SIZE],
    cur: usize,
}

const DEFAULT_BUF_SIZE: usize = 100000;
const OUTPUT_FILE_NAME: &'static str = "./trace.data";

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static SENDER: RwLock<Option<Sender<Box<[u8]>>>> = RwLock::new(None);

thread_local! {
    static TLS_BUF: RefCell<ThreadLocalBuffer> =  RefCell::new(ThreadLocalBuffer::new())
}

impl ThreadLocalBuffer {
    fn new() -> Self {
        let mut sender = SENDER.write().unwrap();
        if !INITIALIZED.fetch_or(true, std::sync::atomic::Ordering::SeqCst) {
            let (tx, rx) = channel::<Box<[u8]>>();

            std::thread::spawn(move || run_recv_thread(rx));
            *sender = Some(tx);
        }
        let sender = sender
            .as_ref()
            .expect("sender must be initialized.")
            .clone();

        Self {
            sender: sender,
            bytes: [0; DEFAULT_BUF_SIZE],
            cur: 0,
        }
    }
}

fn run_recv_thread(rx: Receiver<Box<[u8]>>) {
    let mut file = std::fs::File::create(OUTPUT_FILE_NAME).unwrap();

    let mut vec: Vec<Box<[u8]>> = vec![];
    const FLUSH_BUF_SIZE: usize = 2_000_000;
    let mut i = 0;
    let mut current_sum = 0;
    while let Ok(b) = rx.recv() {
        let total_sum = &b.len() * i;
        current_sum += &b.len();
        println!("{i}: get {:?}, sum: {:?}", &b.len(), total_sum);

        if FLUSH_BUF_SIZE <= current_sum {
            // flush
            let total_size: usize = vec.iter().map(|box_data| box_data.len()).sum();
            let mut combined = Vec::with_capacity(total_size);
            for box_data in vec {
                combined.extend_from_slice(&box_data);
            }
            let combined_box: Box<[u8]> = combined.into_boxed_slice();

            file.write_all(&combined_box).unwrap();
            file.flush().unwrap();

            println!("flush {} bytes done!!!!!!", current_sum);

            vec = vec![];

            current_sum = 0;
        } else {
            vec.push(b);
        }

        i += 1;
    }
}

impl std::io::Write for ThreadLocalBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut written = 0;

        // println!(
        //     "cur: {:?}, buf: {:?}, bytes.len: {:?}",
        //     self.cur,
        //     buf,
        //     self.bytes.len(),
        // );

        while written < buf.len() {
            let remaining = DEFAULT_BUF_SIZE - self.cur;
            let to_write = std::cmp::min(remaining, buf.len() - written);

            self.bytes[self.cur..self.cur + to_write]
                .copy_from_slice(&buf[written..written + to_write]);
            self.cur += to_write;
            written += to_write;

            if DEFAULT_BUF_SIZE <= self.cur {
                self.flush().unwrap();
                self.cur = 0;
            }
        }

        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let b = Box::new(self.bytes);
        self.sender.send(b).unwrap();

        Ok(())
    }
}

impl Visit for ThreadLocalBuffer {
    fn record_debug(&mut self, _field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        // TODO: this seems a bit ugly. are there any other ways?
        let formatted_string = format!("{:?}", value);

        // hardcode. if we get `flihgt_recorder` target or data field, then we store the event.
        if _field.to_string() == "target".to_string()
            && formatted_string == "\"flihgt_recorder\"".to_string()
            || _field.to_string() == "data".to_string()
        {
            self.write(formatted_string.into_bytes().as_slice())
                .unwrap();
        }
    }
}

/// docs
#[derive(Debug)]
pub struct Layer {}

impl Layer {
    /// docs
    pub fn builder() -> LayerBuilder {
        LayerBuilder::new()
    }

    /// docs
    pub fn new() -> Self {
        LayerBuilder::default().build()
    }
}

impl<S> tracing_subscriber::Layer<S> for Layer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &event::Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        // write to thread local buffer
        TLS_BUF.with_borrow_mut(|tls_buf| {
            // TODO: cleanup
            let s: Vec<_> = event
                .fields()
                .filter(|field| field.name() == "target" || field.name() == "data")
                .collect();

            if s.len() == 2 {
                event.record(tls_buf);
            }
        });
    }
}

/// docs
#[derive(Debug)]
pub struct LayerBuilder {}

impl LayerBuilder {
    /// docs
    pub fn new() -> Self {
        Self::default()
    }

    fn build(self) -> Layer {
        Layer {}
    }
}

impl Default for LayerBuilder {
    fn default() -> Self {
        Self {}
    }
}

use prost_types::Timestamp;
use proto_event::Event;

mod proto_event {
    include!(concat!(
        env!("OUT_DIR"),
        "/tracing_flihgt_recorder.items.rs"
    ));
}

pub fn create_large_shirt() -> Event {
    let mut data = Event::default();
    data.event_time = Some(Timestamp::date(2024, 12, 13).unwrap());
    data.payload = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string();
    data
}
