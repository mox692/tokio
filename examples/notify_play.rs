// use std::sync::Arc;

// use tokio::sync::Notify;
// use tokio::time::{sleep, Duration};

// #[tokio::main]
// async fn main() {
//     let n1 = Arc::new(Notify::new());
//     let n2 = n1.clone();

//     let jh = tokio::spawn(async move {
//         n2.notified().await;
//         println!("notified!");
//     });

//     sleep(Duration::from_secs(2)).await;

//     println!("sleep done");

//     n1.notify_one();
//     jh.await.unwrap();
// }

use std::sync::Arc;
use tokio::sync::Notify;

#[tokio::main]
async fn main() {
    let notify = Arc::new(Notify::new());
    let notify2 = notify.clone();

    let notified1 = notify.notified();
    let notified2 = notify.notified();

    let handle = tokio::spawn(async move {
        println!("sending notifications");
        notify2.notify_waiters();
    });

    notified1.await;
    notified2.await;
    println!("received notifications");
}
