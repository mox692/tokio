use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

use tokio::sync::mpsc;

// 単調増加な配列を三乗する
fn cube(a: &mut [i32]) {
    for x in a {
        *x = x.pow(3);
    }
}

#[tokio::main]
async fn main() {
    let mut a = [1290, 1291, 1292];
    let (tx, rx) = mpsc::channel::<()>(1024);
    // ここでコンパイルエラーになる
    let perm = tx.reserve().await.unwrap();
    let perm_ref = &perm;
    let tx_ref = &tx;
    let result = catch_unwind(|| async {
        // cube(&mut a);
        // let _ = perm_ref.send(());
        let _ = tx_ref.strong_count();
    });

    // 上の処理がpanicしても実行される
    println!("a = {:?}", a);

    // panic処理をレジュームする
    result.unwrap_or_else(|e| resume_unwind(e));
}
