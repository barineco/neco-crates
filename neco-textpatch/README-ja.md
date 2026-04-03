# neco-textpatch

[英語版](README.md)

UTF-8 文字列のバイト範囲を決定的に書き換え、必要なら名前付きブロックの中身だけを差し替えるための補助クレートです。

## パッチの挙動

`neco-textpatch` は、書き換え前にすべてのバイト範囲を元の文字列に対して検証します。重なった置換、UTF-8 境界をまたぐ位置、曖昧なキー付き項目は黙って処理せず `Err` で返します。

簡単な構造化テキスト向けには、名前付きの波括弧ブロックを見つけて中身だけを置き換える関数もあります。設定ファイル風の一部分だけを更新し、周囲の文字列はそのまま残したい場面を想定しています。

## 使い方

### 1 つの範囲を置換する

```rust
use neco_textpatch::{apply_patch, TextPatch};

let patch = TextPatch::replace(6, 11, "there").expect("valid patch");
let updated = apply_patch("hello world", &patch).expect("patch should apply");

assert_eq!(updated, "hello there");
```

### 名前付き block を置換する

```rust
use neco_textpatch::{apply_patch, replace_block};

let source = "settings {\n  mode = \"fast\"\n}\n";
let patch = replace_block(source, "settings", "\n  mode = \"safe\"\n")
    .expect("settings block should exist");
let updated = apply_patch(source, &patch).expect("patch should apply");

assert_eq!(updated, "settings {\n  mode = \"safe\"\n}\n");
```

## API

| 項目 | 説明 |
|------|-------------|
| `TextPatch` | 検証付きのバイト範囲置換 1 件 |
| `TextPatch::new(start, end, replacement)` | 範囲を検証して patch を作り、不正なら `Err` を返す |
| `TextPatch::insert(offset, replacement)` | 1 つのバイト位置に文字列を挿入する |
| `TextPatch::delete(start, end)` / `replace(start, end, replacement)` | 削除と置換の補助コンストラクタ |
| `TextPatchError` | 範囲不正、範囲外、UTF-8 境界違反、競合、ブロック探索失敗を表す |
| `PatchConflict` | 検証時に重なった 2 つの置換を表す |
| `validate_patches(source, patches)` | 元の文字列に対して範囲と競合を検証する |
| `apply_patch(source, patch)` / `apply_patches(source, patches)` | 検証済みの置換を決定的に適用する |
| `find_block_range(source, block_name)` | 名前付きブロックのバイト範囲を探す |
| `replace_block(source, block_name, replacement)` | ブロック本文を置換する `TextPatch` を作る |
| `KnownEntry` | ブロック統合用のキー付き置換 1 件 |
| `merge_known_entries(source, block_name, entries)` | 既知のキーを置換し、足りないキーは末尾へ追加する |

## ライセンス

MIT
