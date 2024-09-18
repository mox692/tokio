### how to run

run example code:
```bash
# release
cargo build --release --package examples --example worker-tracing && ./target/release/examples/worker-tracing

# debug
cargo build --package examples --example worker-tracing && ./target/debug/examples/worker-tracing
```

symbol resolve:

```bash
cargo run --package tracing-perfetto --bin tracing-perfetto release 
```
