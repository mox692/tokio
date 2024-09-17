### how to run

run example code:
```bash
# release
RUSTFLAGS="--cfg tokio_unstable -C force-frame-pointers=yes" cargo build --release --package examples --example worker-tracing && ./target/release/examples/worker-tracing

# debug
RUSTFLAGS="--cfg tokio_unstable -C force-frame-pointers=yes" cargo build --package examples --example worker-tracing && ./target/debug/examples/worker-tracing
```

symbol resolve:

```bash
cargo run --package tracing-perfetto --bin tracing-perfetto 94356196978688 release # offset, release/debug
```
