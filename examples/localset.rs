use std::rc::Rc;

use tokio::{task, time};
#[tokio::main]
async fn main() {
    let nonsend_data = Rc::new("world");
    let local = task::LocalSet::new();

    let nonsend_data2 = nonsend_data.clone();
    local.spawn_local(async move {
        // ...
        println!("hello {}", nonsend_data2)
    });

    local.spawn_local(async move {
        time::sleep(time::Duration::from_millis(100)).await;
        println!("goodbye {}", nonsend_data)
    });

    // ...

    local.await;
}

// #[tokio::main]
// async fn main() {
//     let nonsend_data = Rc::new("my nonsend data...");

//     // Construct a local task set that can run `!Send` futures.
//     let local = task::LocalSet::new();

//     // Run the local task set.
//     local
//         .run_until(async move {
//             let nonsend_data = nonsend_data.clone();
//             // `spawn_local` ensures that the future is spawned on the local
//             // task set.
//             task::spawn_local(async move {
//                 println!("{}", nonsend_data);
//                 // ...
//             })
//             .await
//             .unwrap();

//             task::spawn(async move {
//                 println!("aaaaaaaaaa");
//                 // ...
//             })
//             .await
//             .unwrap();
//         })
//         .await;
// }
