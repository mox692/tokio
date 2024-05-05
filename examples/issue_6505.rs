use std::time::Duration;

use tokio::{sync::oneshot, task::JoinHandle};

#[tokio::main]
async fn main() {
    let (sender, receiver) = oneshot::channel::<JoinHandle<()>>();
    let jh1 = tokio::spawn(async {
        let jh = receiver.await.unwrap();
        let f = tokio::time::timeout(Duration::from_secs(1), jh);
        let res = f.await;
        println!("timer fire!!");
        res.unwrap_or(Ok(()))
    });

    let jh2 = tokio::spawn(async {
        let s = jh1.await.unwrap();
        println!("timeout done!!, jh1 dropped now!");
        // but still waker for jh1 is alive ? ...
    });

    let _ = tokio::spawn(async {
        sender.send(jh2).unwrap();
    })
    .await;

    tokio::time::sleep(Duration::from_secs(1000)).await;
}
