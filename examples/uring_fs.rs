use tokio::fs::{read3, OpenOptions};

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        let file = OpenOptions::new()
            .open3("/home/mox692/work/tokio/examples/uring_fs.rs")
            .await
            .unwrap();
        let meta = file.metadata().await.unwrap();

        println!("meta: {:?}", meta);

        let res = read3("/home/mox692/work/tokio/examples/uring_fs.rs")
            .await
            .unwrap();
        println!("content: {:?}", String::from_utf8(res));
    });
}
