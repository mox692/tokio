use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_stream::{Stream, StreamExt, StreamMap};

#[tokio::main]
async fn main() {
    let (tx1, mut rx1) = mpsc::channel::<usize>(10);
    let (tx2, mut rx2) = mpsc::channel::<usize>(10);

    // Convert the channels to a `Stream`.
    let rx1 = Box::pin(async_stream::stream! {
          while let Some(item) = rx1.recv().await {
              yield item;
          }
    }) as Pin<Box<dyn Stream<Item = usize> + Send>>;

    let rx2 = Box::pin(async_stream::stream! {
          while let Some(item) = rx2.recv().await {
              yield item;
          }
    }) as Pin<Box<dyn Stream<Item = usize> + Send>>;

    tokio::spawn(async move {
        tx1.send(1).await.unwrap();

        // This value will never be received. The send may or may not return
        // `Err` depending on if the remote end closed first or not.
        let _ = tx1.send(2).await;
    });

    tokio::spawn(async move {
        tx2.send(3).await.unwrap();
        let _ = tx2.send(4).await;
    });

    let mut map = StreamMap::new();

    // Insert both streams
    map.insert("one", rx1);
    map.insert("two", rx2);

    // Read twice
    for _ in 0..2 {
        let (key, val) = map.next().await.unwrap();

        if key == "one" {
            assert_eq!(val, 1);
        } else {
            assert_eq!(val, 3);
        }

        // Remove the stream to prevent reading the next value
        map.remove(key);
    }
}
