* noteを追記する
  * 具体的に, どう言うstepで進めるかの更新
    * initial infra
    * 各Opを, 優先度順にsupport
      * supprot されているAPIについては, docsにめいきしておくのが吉
* 実験repoを形にする
  * uringを, io driverに組み込むのではなく, 独自のdriverに分けれないか？ tokio/src/runtime/driver.rs にいろいろdriverがあるように。
    * モチベ
      * 既存コードへの影響を減らす
      * レビューを楽にする
      * 既存のio driverに追加すると, cfg_io_driver!をいじる必要性が出てくる
  * `open`, `read`, `write` をちゃんと実装する (より実際的な労力を知るため)
  * driverのdropの実装
  * Opのリファクタ
    * StateをOpに持たせないように
      * 初回pollまでlazyにできるって思ってたけど, Op単体でuserに返すことはないからこれは問題にならないかも. tokio-uringに近くした方がreviewしやすそう
      * OpのFutreの実装がすっきりするはず
      * ... と思ったが, マルチスレッドだと少し問題がありそう
        * Opが初回Pollされて, PollPendingを返そうとしている時に, driver側で完了が検知されると, taskがwakeされなくなるシナリオがありそう
          * 詰まるところ, sqへのsubmitの前に, wakerをlifecycleに保存しておく必要があるが, eargerにsubmitするtokio-uringの方針だとこれが満たされていない
      * あるタスクがすでに動いている時に, そのtaskのwakeを起動したらどうなる?
        * 正確な実験はできなかったが,  再スケジュールされるはず. (自身でwake_by_refを使ってPendingを返すコードとかと実質的に同じはず)
    * uringContextをDriver::Handle以外の場所におく
      * とはいえ適切な置き場所がわからん
    * Op自体にdriverのreference持たせる？
      * tokio-uring は `Rc<RefCell<Driver>>` だからうま味あったかもだが, tokioでやろうとすると Arc<Mutex<Driver>>になりそう
      * チェーンが短くなる方法があれば, それは採用したいかも
  * (ワンチャン) global ringに戻す
* 新しいbranch切って, 実際にPRを出す粒度でcommitを作っていく
  * initial infrastructure
    * changes to the driver
      * epollのwake処理
      * globalなuring contextをdriverに突っ込む
      * tokio/src/runtime/io/driver/uring.rs
    * add cfg
    * globalなuring contextをdriverに突っ込む
    * tokio/src/runtime/driver/op.rs の内容
    * test
  * actual operation support
    * write
    * open
    * statx
    * ...
* carlとaliceに, incrementalに対応するopを増やすことがokかを聞く


Here is a memo about the order in which we should support the io_uring API.

### API Tier
The implementation of uring operations can be done incrementally. However, some APIs are used internally by others, creating dependencies between them.  

So, the file APIs have been grouped into two tiers, reflecting their implementation priority.

(Note that this is not meant to strictly define the implementation order, but rather to serve as a guideline.)

**Tier 1: Important APIs that are also used by other APIs**
* `OpenOptions::open()`
  * Internally used by `File::open()` and `File::create()`
* `File::open()`, `File::create()`
  * Internally used by `fs::read()` and `fs::write()`
* `statx(2)`
  * Required for determining buffer size in `fs::read()` and for the `fs::metadata()` API
* `fs::write()`
  * Frequently used API
  * Can be supported once `open` is implemented
* `fs::read()`
  * Frequently used API
  * Can be implemented once `open` and `fstat` are supported
* APIs for `fs::File` and others
  * Update the `AsyncWrite` and `AsyncRead` implementations to use io_uring

**Tier 2: APIs that depend on Tier 1 APIs or are considered less frequently used**
* `fs::copy()`
* Directory-related APIs (Unlike files, all directory operations seem to go through `DirBuilder`)
  * `DirBuilder`
  * `create_dir()`
  * `create_dir_all()`
  * `fs::read_dir()`
* `fs::set_permissions()`

... and other APIs

---

もし微調整や別のニュアンスでの翻訳も必要なら教えてくださいね！



* 順ばん(仮). https://docs.rs/tokio/latest/tokio/fs/index.html を上から順番に確認
  * tier 1: relatively easy to support io_uring
    * `File::open()`, `File::create()`, `OpenOptions`
      * 基本的にはopenがないと何もできないので, 一番初めにsupportすべきapi
      * この `open` 関数は, tokioの現行のFile objectを返す
      * 後続のuringの処理は, そのfileObjectからfdを取得して, 
      * publicにする必要はない, 一旦privateのOpenOptionsを導入して, todo commentとかで最終的に既存のOpenOptionsと統合すると記入しておく
        * このopenOptionsは, tokio-uringを参考にできると思う
      * 代案として, 最初からuring対応の`File` objectをsupportしてしまうという方法もある. (`File::open()`, `File::create()`)
        * デメリットとしては, File apiの一部だけ uring を使うという中途半端な状態になりうる点
        * でも意外と問題ないかも
      * first PRで, `File::open`, `File::create` をsupportする, でいいかも (cfgついた時だけuring)
    * `write()`
      * tokio uringを見た感じ, 実装は楽そう
    * `statx()`
      * readで必要
      * そこそこ実装が必要かも. これがないとreadが提供できなくて地獄
    * `read()`
      * open, fstatも呼び出しが必要かも. そこそこ大きなロジックになるかも
      * /home/mox692/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/std/src/sys/pal/unix/fs.rs
    * `read_to_string()`
      * almost the same as `read()`
    * `hard_link`
    * `metadata`
      * fstatが必要
  * tier 2: not hard, but requires implementation a bit
    * `fs::File` の他のapi
    * `fs::copy()`
    * dir系 (fileと違って、どのapiもDirBuilderを経由するっぽい)
      * `DirBuilder`
      * `create_dir()`
      * `create_dir_all()`
    * `OpenOptions`
      * `open()` でFileObjectを作成するのde, FileOpjectに依存してる
  * tier 3: can not be used with io_uring
    * `readlink()`
      * 現状対応してない説: https://github.com/torvalds/linux/blob/fc96b232f8e7c0a6c282f47726b2ff6a5fb341d2/include/uapi/linux/io_uring.h#L223
    * `canonicalize()`
