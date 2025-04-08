# Goal

* support io_uring backed file api


# Tenets
* don't affect existing code base
  * network部分は引き続きepoll baseのシステムを維持する
* user can transparently benefit from io_uring file io
* Incrementally
  * perhaps we have to make changes in several places.
  * changes should be done incrementatlly


## API surface
* 基本的には, 既存のfile apiを踏襲, 何もしない
* ring sizeの決定とかはできた方がいい気がする, それを決めるためのoptionはなんとかしないと

```rust
// TODO: ring sizeのoptionをどうやって提供するか
let file = File::new(xxx).with_uring_ops(yyy)
```

### Register Uring fd
* file IOを最初に行った場合に初期化
* mioのtokenを使える. 一旦NOOP token

```rust
pub(crate) fn add_uring_source(
    &self,
    source: &mut impl mio::event::Source,
    interest: Interest,
) -> io::Result<()> {
    self.registry
        .register(source, TOKEN_WAKEUP, interest.to_mio())
}
```

### Uring Tasks
* Opのpollが何をするのかを書く

```rust
impl Future for Op<T> {
    fn poll(...) { ... }
}
```
* Uring <-> Task のmappingをどのように行う?
  * TODO: これを書くまでに, IOとFutureが何:何になるかを調べる
  * 今の実装のままでよかったら, slabを使うでいいはず.
    * TODO: 本当にslabが最適なのかをみる

### Submission
* シンプルなら, 毎回submitでも良い
* ただし, 理想的にはバッチングを活用した方がいい
  * バッチングの戦略をいくつかと pros, cons
    * どこでsubmissionすべき?
    * 自分の実験repo載せてもいいかも

### Completion
* pollingの時に, mioのtokenを使う

```rust
// tokio/src/runtime/io/driver.rs

// Polling events ...
match self.poll.poll(events, max_wait) { ... }

for event in events.iter() {
    // process epoll events
}

/* NEW */
for cqe in cq.iter() {
    // process uring events
    let index = userdata();
    let s = slab.get(index);
}
```
### Multi threaded support
* シンプルには, ringをglobalでも良い
* ただし, 現実的にはshardしたほうがいい
  * benchmarkの結果も載せる


### feature gate
* tokio-unstable?

## Next Steps
I think this proposal can be achieved incrementally, like as follows:

1. Initial PR with a minimum support for io_uring file api with `tokio_unstable`
   * Current thread runtime support
   * Basic Open, Read, Write operation
   * (possibly) No batching logic for submission
2. Muti threaded runtime support
   * Maybe just having one global ring, for simplicity.
3. Further improvements, such as
   * Sharding rings for multi-threaded runtime (having one ring per thread)
   * Support more uring Ops
   * Smarter batching logic for submission
   * Utilize registered buffers, registered file
4. Stabilize 🚀 (remove `tokio_unstable`)


## Prototype
* 実際に行ったbranchとbenchを載せておいても良いかも (ある程度信頼性が増す?)
  * multi-threaded
  * sharding
  * batching logig
* 実装の正しさには注意を払ったが, 間違えている可能性はある。testは全部passしてる
