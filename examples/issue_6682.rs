use std::time::Duration;
use tokio::{sync::oneshot, task::JoinHandle};

#[tokio::main]
async fn main() {
    let h = tokio::runtime::Handle::current();

    let (tx, mut rx) = tokio::sync::mpsc::channel::<usize>(1024);

    let jh = h.spawn(async move {
        loop {
            let mut wait_time = { Some(Duration::from_millis(100)) };
            if let Some(delay) = wait_time {
                match tokio::time::timeout(delay, rx.recv()).await {
                    Ok(val) => match val {
                        None => {
                            return;
                        }
                        Some(n) => {
                            wait_time = {
                                println!("n: {}", n);
                                Some(Duration::from_millis(100))
                            }
                        }
                    },
                    Err(_e) => wait_time = Some(Duration::from_millis(100)),
                }
            } else {
                return;
            }
        }
    });

    let mut i = 0;
    loop {
        tx.send(i).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        i += 1
    }

    jh.await.unwrap()
}
