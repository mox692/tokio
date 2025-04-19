
* 実験repoを形にする
  * `read`, `write` をちゃんと実装する (より実際的な労力を知るため)
  * cancelの実装ちゃんとしてる？
  * // potentially we want to iterate cq here. のことろを治す
  * テストを追加
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
  * read / write + a lot of document
* PRどのようにsplitするのかを考える
  * 
  * just idea
    * initial infra structureと, 実際のopを別のPRに分けれるのじゃないかな

* carlとaliceに, incrementalに対応するopを増やすことがokかを聞く

### Api Tier
* 順ばん(仮). https://docs.rs/tokio/latest/tokio/fs/index.html を上から順番に確認
  * tier 1: relatively easy to support io_uring
    * `read()`
    * `write()`
    * `read_to_string()`
      * almost the same as `read()`
    * `hard_link`
    * `metadata`
  * tier 2: not hard, but requires implementation a bit
    * `fs::File`
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
