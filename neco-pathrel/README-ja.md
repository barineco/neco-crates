# neco-pathrel

[英語版](README.md)

パス文字列どうしの包含判定と、ファイル名変更やディレクトリ名変更に沿ったパス書き換えを行う補助クレートです。

## パスの扱い

`neco-pathrel` はファイルシステムを見に行かず、明示した `PathPolicy` に従ってパス文字列を比較します。呼び出し側は区切り文字と大文字小文字の扱いを 1 回決めれば、その方針を配下判定、親パス取得、パス結合、名前変更追従にそのまま使えます。

`.`, `..`, シンボリックリンク、ドライブレター、UNC パスの正規化は行いません。ホストごとの差を持ち込まず、実行時のパス文字列処理だけに責務を絞っています。

## 使い方

```rust
use neco_pathrel::{path_matches_or_contains, remap_path_for_rename, PathPolicy};

let policy = PathPolicy::posix();

assert!(path_matches_or_contains(
    "/workspace/src/lib.rs",
    "/workspace/src",
    &policy,
));

let renamed = remap_path_for_rename(
    "/workspace/src/lib.rs",
    "/workspace/src",
    "/workspace/core",
    &policy,
);
assert_eq!(renamed.as_deref(), Some("/workspace/core/lib.rs"));
```

## API

| 項目 | 説明 |
|------|-------------|
| `PathPolicy::new(separator, case_sensitivity)` | 明示的なパス比較方針を構築する |
| `PathPolicy::posix()` | 既定の POSIX 風方針を構築する |
| `path_matches_or_contains(path, target, policy)` | `path` が `target` と一致するか、その配下にあるかを返す |
| `parent_path(path, policy)` | 親パスがあればその文字列片を返す |
| `join_path(base, name, policy)` | 2 つのパス断片を 1 個の区切り文字で結合する |
| `remap_path_for_rename(path, source, target, policy)` | ファイルまたは配下全体の名前変更を通したパス書き換えを行う |

## ライセンス

MIT
