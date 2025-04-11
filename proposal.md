- Feature Name: (fill me in with a unique ident, `my_awesome_feature`)
- Start Date: (fill me in with today's date, YYYY-MM-DD)
- RFC PR: [rust-lang/rfcs#0000](https://github.com/rust-lang/rfcs/pull/0000)
- Rust Issue: [rust-lang/rust#0000](https://github.com/rust-lang/rust/issues/0000)

このproposalは議論の出発点となることを目指しており, 議論の度に改訂されることがあります。

# Summary
[summary]: #summary

* tokioのfile apiに, linux向けにio_uringを使用することを提案します。
* 当面は, 既存のfile apiの裏側を透過的に置き換えることにフォーカスします。io_uring専用のregistered fd, registered bufferなどの発展的な機能は後続のRFCによって解決されます。
* network ioに関してはこのRFCの対象外です。


# Motivation
[motivation]: #motivation

* 現状のfile apiはoperationごとにspawn_blockを使ったthread poolを用いている
* io_uringを使い, 下記を達成することでFileAPIのpeformance改善を行う
  * 操作ごとに発生するthread spawnを減らす
  * system callを減らす

* file apiによらない, もっと広い文脈に関しては [こちら](https://github.com/tokio-rs/tokio-uring/blob/master/DESIGN.md#motivation) も参考にして。

# Guide-level explanation

* epollとio_uringを共存
  * 具体的には, epollでio_uringのイベントを待つ (tokio-uringが行っているように)
  * この方法のメリットとしては, 変更量が少なくて良い点
* uringのoperationを表現するFutureを実装する (tokio-uringでは`Op`という構造体がこれをやっている)
  * 初回のpollでsqeにoperationのsubmitを実施
  * operationの完了時, epoll_ctlがreturnして, cqeを走査し, taskをwakeする

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

# Implementation Design

### Register Uring File Descriptor

* io_uringのfdをepollに登録することで, epoll_ctlでuringの完了イベントを受け取れる
* file IOを最初に行った場合 or runtimeの初期化時に, uringのfdをmio経由でepollに登録
* これは将来的には各worker threadごとに行うことができる(i.e. threadごとにringを1つ持つ)と思われますが, 実装が複雑になる場合ははじめは1つのglobal ringを持つことから始めることができます。
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
* file operationが発行されると, 対応するUringFutureが内部で生成されます。これらのFutureのライフサイクルは次のようになります:
  * 初回のpoll: sqに自身のoperationをsubmitします. さらに, driver側が現在発生しているoperationの一覧にアクセスできるように, slabのようなdata構造に自身を追加します.
  * completion時: driverからwakeされ, 完了した操作をuser programに返します.
  * cancel時: 自身をslabから取り除きます.
* これらのFutureのデザインはtokio-uringとほぼ同じものです

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
    let index = cqe.userdata();
    let waker = slab.get(index);
    waker.wake()
}
```

### Driver
* uringfdをepollに登録したことで, uringでsubmitしたoperationが完了したらepollが返るようになる
* driverでは, 普通のepollの処理を終えた後に, cqeの操作も行うことで, uringのtaskもwakeすることが可能.

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
    let index = cqe.userdata();
    let waker = slab.get(index);
    waker.wake()
}
```

# Drawbacks
[drawbacks]: #drawbacks

* tokioの複数のcomponentに変更を入れる必要があるため, 変更はincrementalに行われる必要があります。

# Alternatives
* io completionをio-uringで待つか, epollで待つかの選択がある
  * io-uringでcompletionを待つ場合, runtimeの大きな変更が必要になり, これは避けたい。
* uring taskをpollingするtokio taskを作成
  * これはtokio-uringが取っている戦略
  * しかし, scheduleのfairnessの観点で問題がある

# Prior art

### tokio-uring
* tokioとの差分は
  * current thread runtimeのみを対象としている
  * network ioもサポート
* しかし他の部分, 例えばOperationに関するFuture(`Op`)は実装のいくつかを継承する可能性が高い

### glommio, monoio


# Unresolved questions
* 賢いbatchingロジック
  * tokioのイベントループの中で, いつエントリをバッチでsubmitすべき?
* threadごとにringを持つ際の, 実装の詳細
  * 具体的なprototypeやベンチマークが有用になるでしょう

# Future work

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



Think about what the natural extension and evolution of your proposal would
be and how it would affect the language and project as a whole in a holistic
way. Try to use this section as a tool to more fully consider all possible
interactions with the project and language in your proposal.
Also consider how this all fits into the roadmap for the project
and of the relevant sub-team.

This is also a good place to "dump ideas", if they are out of scope for the
RFC you are writing but otherwise related.

If you have tried and cannot think of any future possibilities,
you may simply state that you cannot think of anything.

Note that having something written down in the future-possibilities section
is not a reason to accept the current or a future RFC; such notes should be
in the section on motivation or rationale in this or subsequent RFCs.
The section merely provides additional information.
