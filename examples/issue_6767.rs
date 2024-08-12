// tokio 1.39.2

use tokio::net::UnixStream;

#[tokio::main]
async fn main() {
    UnixStream::connect("\0aaa").await.unwrap();
}
