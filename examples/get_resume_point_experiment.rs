use std::task::Poll;

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        println!("1");
        let fut = MyFuture::new();

        // 多分, userはここのline-noを知りたい
        fut.await;

        println!("2");
    });
}

struct MyFuture {
    pub state: i32,
}

impl MyFuture {
    fn new() -> Self {
        Self { state: 0 }
    }
}

impl std::future::Future for MyFuture {
    type Output = ();

    #[track_caller]
    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let location = caller_location();
        println!("@@@@@@@@@@@@@ location: {:?}", &location);

        if self.state <= 10 {
            cx.waker().wake_by_ref();
            unsafe { self.get_unchecked_mut().state += 1 };
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

#[track_caller]
pub(crate) fn caller_location() -> Option<&'static std::panic::Location<'static>> {
    return Some(std::panic::Location::caller());
}
