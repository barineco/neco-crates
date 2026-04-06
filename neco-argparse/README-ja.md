# neco-argparse

[English](README.md)

neco-json をバックエンドに使う necosystems series の CLI 引数パーサーおよびバリデータです。

## 概要

- `ArgDef` / `CommandMeta` でコマンドのスキーマを宣言する
- `parse_and_validate` で `JsonValue` のパラメータをスキーマに照らして検証する
- `parse_cli_args` で `std::env::args()` 由来の生引数を構造化された `JsonValue` に変換する

## 使い方

```rust
use neco_argparse::{ArgDef, ArgType, CommandMeta, parse_cli_args, parse_and_validate};
```

## ライセンス

MIT
