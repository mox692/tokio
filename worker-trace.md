### how to run

run example code:
```bash
# release
cargo build --release --package examples --example runtime-tracing && ./target/release/examples/runtime-tracing

# debug
cargo build --package examples --example runtime-tracing && ./target/debug/examples/runtime-tracing
```

symbol resolve:

```bash
cargo run --package tracing-perfetto --bin tracing-perfetto release 
```
