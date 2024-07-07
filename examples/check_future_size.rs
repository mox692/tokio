use std::hint::black_box;

use futures::Future;

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let big_future = async {
        // 8 * 1MiB = 8 MiB
        let array = [0 as u64; (1024 * 1024) / 8];
        // let array = [0 as u64; 1024];
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        black_box(array)
    };
    // print_size(Box::pin(big_future));

    rec(big_future, 8);

    // print_size(big_future);
}

// 8くらいにすると死ぬ
#[inline(never)]
fn rec<T>(t: T, n: usize) {
    println!("called: {}", n);

    if n > 0 {
        rec(t, n - 1);
    } else {
        return;
    }
}

fn print_size<F>(t: F)
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    println!("the size of t is {}", std::mem::size_of::<F>());
}
