use tokio::io::AsyncBufReadExt;

use std::io::Cursor;

#[tokio::main]
async fn main() {
    let mut cursor = Cursor::new(b"s");
    // let mut buf = vec![];

    let s = cursor.fill_buf().await.unwrap();

    println!("{:?}", s);
    // // cursor is at 'l'
    // let num_bytes = cursor
    //     .read_until(b'-', &mut buf)
    //     .await
    //     .expect("reading from cursor won't fail");

    // assert_eq!(num_bytes, 6);
    // assert_eq!(buf, b"lorem-");
    // buf.clear();

    // // cursor is at 'i'
    // let num_bytes = cursor
    //     .read_until(b'-', &mut buf)
    //     .await
    //     .expect("reading from cursor won't fail");

    // assert_eq!(num_bytes, 5);
    // assert_eq!(buf, b"ipsum");
    // buf.clear();

    // // cursor is at EOF
    // let num_bytes = cursor
    //     .read_until(b'-', &mut buf)
    //     .await
    //     .expect("reading from cursor won't fail");
    // assert_eq!(num_bytes, 0);
    // assert_eq!(buf, b"");
}
