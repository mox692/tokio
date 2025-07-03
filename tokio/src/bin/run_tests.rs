use std::process::exit;
use std::process::Command;

fn main() {
    // let project_root = env!("CARGO_MANIFEST_DIR");

    // let status = Command::new("cargo")
    //     .arg("test")
    //     .arg("-p")
    //     .arg("tokio")
    //     .arg("--features")
    //     .arg("full")
    //     .current_dir(project_root)
    //     .status()
    //     .unwrap_or_else(|e| {
    //         eprintln!("failed to execute cargo test: {}", e);
    //         exit(1);
    //     });

    // match status.code() {
    //     Some(code) => exit(code),
    //     None => {
    //         eprintln!("process terminated by signal");
    //         exit(1);
    //     }
    // }

    println!("hello, world!");
}
