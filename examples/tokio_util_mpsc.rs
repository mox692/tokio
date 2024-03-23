use futures::SinkExt;
use tokio_util::sync::PollSender;

#[tokio::main]
async fn main() {
    let (tx, _rx) = tokio::sync::mpsc::channel::<()>(1);
    let mut ps = PollSender::new(tx);

    let s = ps.send(()).await.unwrap();
}
