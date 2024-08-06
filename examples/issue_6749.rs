use std::time::Duration;
use tokio::{
    sync::oneshot,
    task::{JoinHandle, JoinSet},
};

#[tokio::main]
async fn main() {
    std::panic::set_hook(Box::new(|panic_info| {
        if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            println!("panic occurred!!!!!!!!!!!!!: {s:?}");
        } else {
            println!("panic occurred");
        }
    }));

    // std::panic::panic_any("panic message");

    let jh = tokio::spawn(async {
        std::panic::panic_any("panic message");
    })
    .await
    .map_err(|e| {
        // println!("panic message {:?}", e);
        map_join_error(e);
    });
}

pub fn map_join_error(err: tokio::task::JoinError) {
    let Ok(panic) = err.try_into_panic() else {
        println!("task cancell");
        return;
    };

    let panic_str = panic_payload_to_str(&panic);

    println!("errrrrrrrrrrrr: {panic_str}");
    // eyre::eyre!("task panicked: {panic_str}")
}

/// Extract a string from a panic payload.
pub fn panic_payload_to_str<'a>(panic: &'a (dyn std::any::Any + 'static)) -> &'a str {
    // Panic payloads are almost always `String` (if made with formatting arguments)
    // or `&'static str` (if given a string literal).
    //
    // Non-string p)ayloads are a legacy feature so we don't need to worry about those much.
    panic
        .downcast_ref::<&str>()
        .map(|s| s.clone())
        .unwrap_or("non-string")
    // .map(|s| &**s)
    // .or_else(|| panic.downcast_ref::<&'static str>().copied())
    // .unwrap_or("(non-string payload)")
}
