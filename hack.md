# Summary
One paragraph explanation of the feature.

* support io_uring backed file api

このproposalは議論の出発点となることを目指しており, 議論の度に改訂されることがあります。

# Problem
backgroundと課題間

# Goal
* file apiにio_uringを使うようにする.
  * 初期はopt-inの形で使用できるようにして, 将来的にはユーザーがコードを変えなくても透過的にio_uringのメリットを享受できる
* 既存のruntimeやnetworkコンポーネントにはperformanceの影響を与えない
* 既存のfile apiの互換性を維持したまま小さく変更をincrementalに加えていく

# Non-Goal
* networkでのio_uringの活用

# Archtecture Overview
ハイレベルなアーキテクチャ
* epollとio_uringを共存
  * 具体的には, epollでio_uringのイベントを待つ (tokio-uringが行っているように)
  * この方法のメリットとしては, 変更量が少なくて良い点
* uringのoperationを表現するFutureを実装する (tokio-uringでは`Op`という構造体がこれをやっている)
  * 初回のpollでsqeにoperationのsubmitを実施
  * operationの完了時, epoll_ctlがreturnして, cqeを走査し, taskをwakeする
* linux以外のplatformに関しては, 何も変えない

### Other Options
* io completionをio-uringで待つか, epollで待つかの選択がある
  * io-uringでcompletionを待つ場合, runtimeの大きな変更が必要になり, これは避けたい。

# Details

### API Surface
* 基本的には既存のapi surfaceは変えない。
* ring sizeの決定とかはできた方がいい気がするので, それを決めるためのoptionはunstableで追加する, 例えば:

```rust
let file = OpenOptions::new()
    .read(true)
    .io_uring_config(UringOption::new().ring_size(64)) // **NEW**
    .open(&path)
    .await;

file.read() // this read will use io_uring
```

* このようにすることで, 例えば下記のように段階的にio_uringを使うことができる
  * `OpenOptions` でio_uringのoptionを使ってopenした時だけ, io_uringの実装にfallbackさせる
  * oneshot operationを `tokio::fs::read()`, `tokio::fs::write()` などをデフォルトでio_uringを使用するように
  * `tokio::fs::File::create()` などを, デフォルトでio_uringを使用するように

### Register Uring File Descriptor

* io_uringのfdをepollに登録することで, epoll_ctlでuringの完了イベントを受け取れる
* file IOを最初に行った場合 or runtimeの初期化時に, uringのfdをmio経由でepollに登録
* はじめは, mioの dummy token(TOKEN_WAKEUPなど)からスタートできると思う

```rust
pub(crate) fn add_uring_source(
    &self,
    source: &mut impl mio::event::Source,
) -> io::Result<()> {
    self.registry
        .register(source, TOKEN_WAKEUP, Interest::READABLE)
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

### Buffer
* TODO
* oneshotではなく, file apiのbufferをどうするか
  * tokio-uringの既存のworkが参考になりそう

### feature gate
* tokio-unstable?

# DrawBacks

## Next Steps
I think this proposal can be achieved incrementally, like as follows:

1. Initial PR with a minimum support for io_uring file api with `tokio_unstable`
   * Enable current thread runtime only 
   * Add uring support as an opt-in option to the `OpenOption`.
   * Basic Open, Read, Write operation
   * (possibly) No batching logic for submission
2. Muti threaded runtime support
   * Maybe just having one global ring, for simplicity.
3. Further improvements, such as
   * Sharding rings for multi-threaded runtime (having one ring per thread)
   * Support more uring Ops
   * Smarter batching logic for submission
   * Utilize registered buffers, registered file
4. Use io_uring as a default in `File::new`, `fs::read`, `fs::write` etc.
5. Stabilize 🚀 (remove `tokio_unstable`)


### Other Options
* Taskを, 既存のIO Stackを使わない方法もある
  * tokio-uring的な
  * slabをglobalに持って, そこでやりとりする
  * こっちのが実装は楽かも
  * PollEventedを使った際の難しさの言語化
    * 既存コードへの変更が大きい
      * tokenで処理を区別したい -> scheduledIoのpointerとして使うのを変える必要あり
        * なぜ区別したい？ -> wakeする
    * fdの登録 (コード的にはPollEventedの作成)
      * epoll: そのfd自体の登録
      * uring: sqにsubmit
    * fdのpoll
      * epoll: 既存のpoll_ready()
      * uring: 既存のpoll_ready() (同じ)
    * completeしたとき
      * epoll: 
      * uring: 1つのepollで複数のtaskをwakeすることができるdriverでcqを操作して, uringの結果を適宜taskに渡してwakeする
    * **まとめ**
      * 既存のepollのapiは処理1つ1つのfdを登録していくのに対して, uringはuringのfdだけを登録する -> pollEventedが1つ1つsorceを求めてくることに合わない
      * epollは1つのfd完了eventに対して, 複数のtaskが紐づく, だったが, io_uringの場合は1つのfd完了eventに対して, 複数のioリソースが紐づく
* epoll_taskがuring_taskを起こすようにする
  * pros, cons
* 既存のScheduledIo, PollEventedのconstructに乗っかる 
  * io-uringでは, operationごとにepoll_ctlは呼ばない
  * 1つのfdの完了で, 複数のioリソースが


## Prototype
* 実際に行ったbranchとbenchを載せておいても良いかも (ある程度信頼性が増す?)
  * multi-threaded
  * sharding
  * batching logig
* 実装の正しさには注意を払ったが, 間違えている可能性はある。testは全部passしてる
