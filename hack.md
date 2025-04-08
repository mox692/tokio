# Goal

* support io_uring backed file api
* don't affect existing code base (network, etc)
* user can transparently benefit from io_uring file io


# Summary


### API surface
* 基本的には, 既存のfile apiを踏襲, 何もしない
* ring sizeの決定とかはできた方がいい気がする, それを決めるためのoptionはなんとかしないと

```rust
// TODO: ring sizeのoptionをどうやって提供するか
let file = File::new(xxx).with_uring_ops(yyy)
```

### Uring Tasks
* OpのPollが何をするのかを書く

```rust

```

### Submission
* シンプルなら, 毎回submitでも良い
* ただし, 理想的にはバッチングを活用した方がいい
  * バッチングの戦略をいくつかと pros, cons

### Completion
* pollingの時に, mioのtokenを使う
* 

### feature gate
* tokio-unstable?

### Multi threaded support
* シンプルには, ringをglobalでも良い
* ただし, 現実的にはshardしたほうがいい
  * benchmarkの結果も載せる

# Next Step
I think this proposal can be achieved incrementally, like as follows:

1. Initial PR with a minimum support for io_uring file api with `tokio_unstable`
   * Support for current thread runtime
   * Basic Open, Read, Write operation
   * (possibly) No batching logic for submission
2. Muti threaded runtime support
   * Maybe just having one global ring, for simplicity.
3. Further improvements, such as
   * Sharding rings for multi-threaded runtime (having one ring per thread)
   * Support more uring Ops
   * Smarter batching logic
   * Utilize registered buffers, registered file
4. Stabilize 🚀


# Prototype
* 実際に行ったbranchとbenchを載せておいても良いかも (ある程度信頼性が増す?)
  * multi-threaded
  * sharding
