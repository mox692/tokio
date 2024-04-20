use tracing_subscriber::layer::SubscriberExt;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .init();

    tracing::debug!("this is a tracing line");
}
