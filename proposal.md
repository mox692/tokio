このproposalは議論の出発点となることを目指しており, 議論の度に改訂されることがあります。

# Summary

* tokioのfile apiに, linux向けにio_uringを使用することを提案します。
* 当面は, 既存のfile apiの裏側を透過的に置き換えることにフォーカスします。io_uring専用のregistered fd, registered bufferなどの発展的な機能は別のRFCによって解決されます。
* network ioや, runtimeのio stack全般をio_uringにする試みに関してはこのRFCの対象外です。
* 実装は幾つかのフェーズに分けられインクリメンタルに実施されます。

# Motivation
現行の File API は各操作ごとに spawn_blocking を使用しスレッドプール上で処理しています。しかし, linuxにおいてはfile ioに関して io_uring を導入することで, 下記のような点を改善できる可能性があります。

* 操作ごとのスレッド生成の削減
* システムコールの回数削減

これにより、File API 全体の性能向上を図ります。なお、File API に限定されない広い文脈での背景については [tokio-uring の DESIGN ドキュメント](https://github.com/tokio-rs/tokio-uring/blob/master/DESIGN.md#motivation) も参照してください。


# Guide-level explanation

### Overview
現状, tokioのnetwork IOにはepollを使用しています。 io_uringのfdをepollに登録することで, io_uringのイベントの完了をepoll_ctlから読み取ることでき, この仕組みを用いることでepollとio_uringはある程度共存させることが可能です。

将来的にはio_uringを全面的にsupportすることもできるかもしれませんが, それには大変な労力が必要になります。file apiをio_uringに対応させるだけであれば, epollとio_uringを共存させつつ io_uringのメリットを享受することが十分可能です。 これらの変更は, 破壊的な変更を必要とせずに実現できると思います。

この変更には, おそらくハイレベルには下記の変更が必要になります:

* io_uringのoperationを表現するFutureの追加 (tokio-uringの`Op<T>` のようなもの)
* Driverの変更
  * uringのfdの登録
  * operationのsubmission
  * io_uringで完了したtaskのwake
* fs moduleのapiを io_uring を使用するように変更

これらの変更は, 破壊的な変更を必要とせずに実現できると思います。ただし作業にはtokioのさまざまな箇所への変更が必要になるため, 変更は小さくかつincrementalに行う必要があります。

### API Surface

当面は既存のfile apiのバックエンドをio_uringにすることを目標にするのがいいと思います。そのため, apiは基本的には現状のものから変わりません。最終的には, ユーザーはこれまでのfile apiのコードをそのまま使いつつ, 透過的にio_uringの恩恵を受けることができます。

### Opt-in options

unstableの間に, io_uringを使うことをopt-inさせることができるのは有用だと思われます。これはapiを段階的にio_uringに移行することに役立つし, stableになった後でも何らかの理由(古いlinux kernelを使っている)でこれまでのthread poolの実装を使いたいユーザーにもメリットとなる可能性があります。

**add config options to `OpenOptios`**  
1つのアイデアとしては, 
 `fs::OpenOptions`にモードを選択するoption(`set_mode`)を追加することを検討できるかもしれません。

```rust
let file = OpenOptions::new()
    .read(true)
    // A file object that is created by `set_mode(Mode::Uring(...))` will use
    // io_uring to perform IO.
    .set_mode(Mode::Uring(uring_config)) // **NEW**
    .open(&path)
    .await;

file.read(&mut buf).await; // this read will use io_uring
```

デメリットは, 必要なpublic APIをいくつか追加する必要があるという点と, ワンショット系のオペレーション(`tokio::fs::write()`, `tokio::fs::read()`)には適用できないという点です。(ワンショット系のオペレーションには, どこかのタイミングでデフォルトを差し替えることを検討できるかもしれません.)

**support dedicated cfg: `--tokio_uring_fs`**  
現在のtaskdumpと同様に新しいcfgを導入し, io_uringを使うかthread_poolを使うかをcompile時に決定します。上記と比較してメリットはワンショットAPIにも適用できる点ですが, コード内でio_uringとthread_poolを切り替えることができなくなるのがデメリットです。

# Details
下記に, 上記を達成するための具体的な実装レベルのproposalを示します。

### Register Uring File Descriptor
io_uring のファイルディスクリプタ（fd）を epoll に登録することで、epoll_ctl を通じて完了イベントを受け取れるようにします。
File I/O の最初のタイミングやランタイム初期化時に、mio を経由してこの fd を登録します。初期実装では TOKEN_WAKEUP などのダミートークンを用いて開始可能です。

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
file apiを呼び出すと, 対応するUringFutureが内部で生成されます。これらのFutureのライフサイクルは次のようになります:

* **Submitted**: Futureが生成された際,この状態から開始します。submission queueに自身のoperationをpushします. さらに, driver側から現在発生しているoperationの一覧にアクセスできるように, operationのリストを保持しているデータ構造に自身を追加します. その後, pendingに遷移します。
* **Pending**: operationがまだ完了していない状態です. taskのwakerを保持しています.
* **Completed** io_uringのoperationが完了して, driverからwakeされた場合にこの状態になります。 完了した操作をuser programに返します.

これらのFutureのデザインは[tokio-uring](https://github.com/tokio-rs/tokio-uring/blob/7761222aa7f4bd48c559ca82e9535d47aac96d53/src/runtime/driver/op/mod.rs#L160-L177)とほぼ同じものを継承すると思います。

### Driver
uringfdをepollに登録したことで, uringでsubmitしたoperationが完了したらepollが返るようになります。
driverでは, 普通のepollの処理を終えた後に, cqeの操作も行うことで, uringのtaskもwakeすることが可能になります。  
driverは現在in-flightになっているoperationのlistを保持しており, cqeから得られるuserdataから, どのoperationが完了したのかを判別することができます。

driver内の疑似コードは下記になります:

```rust
// tokio/src/runtime/io/driver.rs

// Polling events ...
self.poll.poll(events, max_wait);

// process epoll events
for event in events.iter() {
}

/* NEW */
// process uring events
for cqe in cq.iter() {
    // process uring events
    let index = cqe.userdata();
    // look up which operation has finished
    let operation = operation_list.get(index);
    operation.wake();
}
```

### Multi thread

マルチスレッドランタイムの場合, ringをどのように保持するかについていくつか選択があります。   

シンプルな方法はglobalにringを1つだけ保持する方法です。これは実装が簡単ですが, スレッドが増えるとringに対する競合が増える可能性があります。

この対抗策として, worker threadの数分だけringをshardingすることで(workerごとにringを割り当てる), 競合を減らせる可能性があります。これにはいくつか実現方法が考えられます(io_uringのuserdataにshard indexを含むデータを保存するようにする, mioのtokenに細工をしてbit fieldにindex情報を持たせるようにする etc).

* io_uringのuserdataにshard indexを含むデータを保存するようにして, 
* mioの`Token`に細工をしてbit fieldにuringのindex情報を持たせるようにする

ただし, その対価として実装の複雑さが増す可能性があります。
おそらく最初はユニークなグローバルringからstartして, follow-upでringのshardingを追加できます。

私の実験レポではこの方法を採用していて, 今の所workしています。

# Drawbacks

* driver内でepollのeventをwakeした後に, uringのwakeを行う戦略をとると, taskのschedulingに暗黙的な優先度が追加される可能性があります。(epollのtaskが優先的に実行される)
  * epoll eventの走査とio_uringの走査を区別せず, eventがio_uringかepollのものかを判別できるようになればこれが解決できる可能性があります。

# Alternatives
**io_uringでepollのeventを待つ**  
epollとio_uringの統合は, 理論的には, io_uringがepollのイベントを待つようにすることも可能です。(`IORING_OP_POLL_ADD` 等を用いて)。しかし, これは既存のepollベースのruntimeを大きく書き換える必要があり, あまり現実的ではありません。

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

**unstableで提供する間のflagの提供方法**  
移行期間中に, どのようにuserにio_uringをopt-inさせるか(もしくはそもそもopt-inを提供せずに暗黙的に差し替えるか)に関しては明確な答えがまだありません。

**submissionの賢いbatchingロジック**  
io_uringのperformanceを最大化するには, submit時のbatchingをうまく活用することが重要です。tokioのevent loopの中で, どのようなbatching戦略を取るのかはまだ未解決です。

このRFCはハイレベルは設計をすり合わせることを目的としており, batchingの詳細な実装戦略は別のissueもしくはPR上で行われます。

**multi thread runtimeでどのようにringを扱うか**  
multi-threadでringをshardingする詳細な実装戦略は別のissueもしくはPR上で行われます。

# Future work

I think this proposal can be achieved incrementally, like as follows:

1. Add minimal io_uring file api support for current thread runtime
   * Add uring support as an opt-in option (using dedicated cfg or `OpenOptions`)
   * Only some important apis are supported (e.g. `File` object only at first)
2. Muti threaded runtime support
   *  Maybe we could start having only one global ring, for simplicity.
3. Further improvements, such as
   * Sharding rings for multi-threaded runtime (having one ring per thread)
   * Expand the usage of io_uring to other fs apis
   * Smarter batching logic for submission
   * Utilize registered buffers, registered file
4. Use io_uring as a default in `File::new`, `fs::read`, `fs::write` etc.
5. Stabilize (remove `tokio_unstable`)
