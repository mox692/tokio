このproposalは議論の出発点となることを目指しており, 議論の度に改訂されることがあります。

# Summary
[summary]: #summary

* tokioのfile apiに, linux向けにio_uringを使用することを提案します。
* 当面は, 既存のfile apiの裏側を透過的に置き換えることにフォーカスします。io_uring専用のregistered fd, registered bufferなどの発展的な機能は別のRFCによって解決されます。
* network ioや, runtimeのio stack全般をio_uringにする試みに関してはこのRFCの対象外です。

# Motivation
[motivation]: #motivation

* 現状のfile apiはoperationごとにspawn_blockを使ったthread poolを用いている
* io_uringを使い, 下記を達成することでFileAPIのpeformance改善を行う
  * 操作ごとに発生するthread spawnを減らす
  * system callを減らす
* file apiによらない, もっと広い文脈に関しては [こちら](https://github.com/tokio-rs/tokio-uring/blob/master/DESIGN.md#motivation) も参考になります。

# Guide-level explanation

### Overview
現状, tokioのnetwork IOにはepollを使用しています。 io_uringのfdをepollに登録することで, io_uringのイベントの完了をepoll_ctlから読み取ることでき, この仕組みを用いることでepollとio_uringはある程度共存させることが可能です。

将来的にはio_uringを全面的にsupportすることもできるかもしれませんが, それには大変な労力が必要になります。file apiをio_uringに対応させるだけであれば, epollとio_uringを共存させつつ io_uringのメリットを享受することが十分可能です。

この変更には, おそらく下記の変更が必要になります:

* io_uringのoperationを表現するFutureの追加 (tokio-uringの`Op` のようなもの)
* Driverの変更
  * operationのsubmit
  * in-flihgtで行われいているOperationsの管理
* fs moduleのapiを io_uring を使用するように変更

これらの変更は, 破壊的な変更を必要とせずに実現できると思います。


### API Surface
上記にも書いたように, 当面は既存のfile apiのバックエンドをio_uringにすることが目標になるので, apiは基本的には現状のものから変わりません。最終的には, ユーザーはこれまでのコードをそのまま使いつつ, 透過的にio_uringの恩恵を受けることができます。


しかし, `unstable`の間は, ユーザーに明示的にio_uringを使うことをopt-inさせることも有用だと思われます。 そのため私は `fs::OpenOptions`  に下記の `io_uring_config()` ような io_uring のためのoptionを追加することを提案します:

```rust
let file = OpenOptions::new()
    .read(true)
    .io_uring_config(UringOption::new().ring_size(64)) // **NEW**
    .open(&path)
    .await;

file.read(&mut buf).await; // this read will use io_uring
```

このようにすることで, 例えば下記のように段階的にio_uringのsupportを進めることができます

1. `OpenOptions` でio_uringのoptionを使ってopenした時**だけ**, io_uringの実装にfallbackさせる
2. oneshot operationを `tokio::fs::read()`, `tokio::fs::write()` などをデフォルトでio_uringを使用するように
3. `tokio::fs::File::create()` などを, デフォルトでio_uringを使用するように

また, 将来的に io_uring に関する設定(queue sizeなど)を調整したい場合にもこのconfigを拡張することで引き続き使用することができます。(stabilizeしても, このapiが腐ることがない)


# Implementation Design
下記に, 上記を達成するための具体的な実装レベルのproposalを示します。

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

// Register uringfd
let uringfd = IoUring::new(num_entry).unwrap();
let mut source = SourceFd(&uringfd);
add_uring_source(&mut source);

```

### Uring Tasks
file operationが発行されると, 対応するUringFutureが内部で生成されます。これらのFutureのライフサイクルは次のようになります:

* **最初のpoll** submission queueに自身のoperationをpushします. さらに, driver側が現在発生しているoperationの一覧にアクセスできるように, slabのようなdata構造に自身を追加します.
* **完了時** completion時: driverからwakeされ, 完了した操作をuser programに返します.
* **キャンセル時** 自身をslabから取り除きます.

これらのFutureのデザインはtokio-uringとほぼ同じものです

### Driver
uringfdをepollに登録したことで, uringでsubmitしたoperationが完了したらepollが返るようになリマス。

driverでは, 普通のepollの処理を終えた後に, cqeの操作も行うことで, uringのtaskもwakeすることが可能になります。

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
    let operation = slab.get(index);
    operation.wake();
}
```

# Drawbacks
[drawbacks]: #drawbacks

* tokioの複数のcomponentに変更を入れる必要があるため, 変更はincrementalに行われる必要があります。
* driver内でepollのeventをwakeした後に, uringのwakeを行う戦略をとると, taskのschedulingに暗黙的な優先度が追加される可能性があります。(epollのtaskが優先的に実行される)
  * epoll eventの走査とio_uringの走査を区別せず, eventがio_uringかepollのものかを判別できるようになればこれが解決できる可能性があります。

# Alternatives
* io completionをio-uringで待つか, epollで待つかの選択がある
  * io-uringでcompletionを待つ場合, runtimeの大きな変更が必要になり, これは避けたい。
* uring taskをpollingするtokio taskを作成
  * これはtokio-uringが取っている戦略
  * しかし, scheduleのfairnessの観点で問題がある
* io_uring専用のFile Objectを新しく定義する
  * 既存のユーザーが明示的に切り替える必要があるので望ましくありません。
* 既存のIO Stac使う
  * PollEvented, ScheduledIoを使う方法
  * 実際に, Pipeの実装とかでは使われている
  * Pros, cons

**io_uringを使うためのapiを提供しない**  
io_uringに関するフラグを提供せずに, tokio側で透過的にどんどんfs moduleをio_uringを使用するように変更している方針も考えられます。


**io_uringでepollのeventを待つ**  
epollとio_uringの統合は, 理論的には, io_uringがepollのイベントを待つようにすることも可能です。(`IORING_OP_POLL_ADD` 等を用いて)。しかし, これにはruntimeの大幅な変更が必要になります。

**io_uring専用のFile Objectを新しく定義する**  
この方法は, ユーザーが明示的にFile objectを差し替える必要が出てくるため, 理想的ではありません。また, linux向けだけにそのタイプを維持していく必要があり, メンテナンス性の観点からも好ましくありません。

**uring taskをpollingするtokio taskを作成**  
これはtokio-uringが取っている戦略です. しかしこのproposalはtokio-uringと違って, tokio runtimeのdriverに直接アクセスできるので, わざわざそのような専用のtaskを生成することは不要です。また, 専用のtaskを使ってuring taskを起床させることは, fairnessの観点で問題が残ります。

# Prior art

### tokio-uring
先行研究として tokio-uring projectがあります。このproposalとの差分は:

* file io以外にもnetwork ioもサポート
* current thread runtimeをベースとしている
* kernelによってregisterされたbufferなどの, 発展的な機能のsupport

しかし他の部分, 例えばOperationに関するFuture(`Op`)などは, 実装のいくつかを継承する可能性が高いです。


# Unresolved questions

**submissionの賢いbatchingロジック**  
io_uringのperformanceを最大化するには, batchingをうまく活用することが重要です。tokioのイベントループの中で, いつエントリをバッチでsubmitすべきかの詳細な検証は, followupで行われる可能性があります。

**`tokio_unstable`で提供する間のopt-in option**  



**threadごとにringを持つ際の, 実装の詳細**  
具体的なprototypeやベンチマークが有用になるでしょう

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
5. Stabilize (remove `tokio_unstable`)
