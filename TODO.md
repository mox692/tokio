
* 実験repoを形にする
  * `read`, `write` をちゃんと実装する (より実際的な労力を知るため)
  * // potentially we want to iterate cq here. のことろを治す
  * (ワンチャン) global ringに戻す
  * テストを追加
* PRどのようにsplitするのかを考える
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
    * tier 3: can not be used with io_uring
      * `readlink()`
        * 現状対応してない説: https://github.com/torvalds/linux/blob/fc96b232f8e7c0a6c282f47726b2ff6a5fb341d2/include/uapi/linux/io_uring.h#L223
      * `canonicalize()`
  * just idea
    * initial infra structureと, 実際のopを別のPRに分けれるのじゃないかな

* carlとaliceに, incrementalに対応するopを増やすことがokかを聞く
