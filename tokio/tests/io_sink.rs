#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full"))]

use tokio::io::AsyncWriteExt;

#[tokio::test]
async fn sink_is_cooperative() {
    tokio::select! {
        biased;
        _ = async {
            loop {
                let buf = vec![1, 2, 3];
                tokio::io::sink().write_all(&buf).await.unwrap();
            }
        } => {},
        _ = tokio::task::yield_now() => {}
    }
}
