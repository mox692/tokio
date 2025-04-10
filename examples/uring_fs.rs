use tokio::fs::{read, read3, OpenOptions, UringOption};

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        let file = OpenOptions::new()
            .use_io_uring(UringOption::new())
            .open("/home/mox692/work/tokio/examples/uring_fs.rs")
            .await
            .unwrap();

        // let meta = file.metadata().await.unwrap();
        // println!("meta: {:?}", meta);

        tokio::spawn(async {
            let res = read3("/home/mox692/work/tokio/examples/uring_fs.rs")
                .await
                .unwrap();
            println!("content: {:?}", String::from_utf8(res));
        })
        .await
        .unwrap();
    });
}
