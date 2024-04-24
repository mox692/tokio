// #[tokio::main]
// async fn main() {}
fn main() {}

// #[test_log::test(tokio::test)]
// #[tokio::test]
// #[test_log::test]
// async fn foo() {
//     println!("Hello, world!")
// }

use test_case::test_case;

#[test_case(4,  2  ; "when operands are swapped")]
#[tokio::test]
async fn bar(x: i32, y: i32) {
    println!("Hello, world!")
}
