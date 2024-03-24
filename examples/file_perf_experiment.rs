use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
    time::Instant,
};

const BLOCK_COUNT: usize = 2;

const BUFFER_SIZE: usize = 4096;
const DEV_ZERO: &str = "/dev/zero";
const DEV_NULL: &str = "/dev/null";

#[tokio::main]
async fn main() {
    write().await;
}

async fn read() {
    let mut file = File::open(DEV_ZERO).await.unwrap();
    let mut buffer = [0u8; BUFFER_SIZE];

    let now = Instant::now();
    for _i in 0..BLOCK_COUNT {
        let count = file.read(&mut buffer).await.unwrap();
        if count == 0 {
            break;
        }
    }

    println!("time: {}", now.elapsed().as_millis());
}

async fn write() {
    let mut file = File::open(DEV_NULL).await.unwrap();
    let mut buffer = [0u8; BUFFER_SIZE];

    let now = Instant::now();
    for _i in 0..BLOCK_COUNT {
        let buf = vec![0u8; BUFFER_SIZE];
        file.write_all(&buf[..]).await.unwrap();
        // let count = file.read(&mut buffer).await.unwrap();
        // if count == 0 {
        //     break;
        // }
    }

    println!("time: {}", now.elapsed().as_millis());
}
