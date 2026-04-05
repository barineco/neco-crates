# neco-base58

[English](README.md)

外部依存ゼロの Base58BTC エンコーダ/デコーダです。

## 使い方

```rust
use neco_base58::{encode, decode};

let encoded = encode(b"abc");
assert_eq!(encoded, "ZiCa");

let decoded = decode("ZiCa").unwrap();
assert_eq!(decoded, b"abc");
```

## API

| 項目 | 説明 |
|------|------|
| `encode(input: &[u8]) -> String` | バイト列を Base58BTC 文字列にエンコードする |
| `decode(input: &str) -> Result<Vec<u8>, Base58Error>` | Base58BTC 文字列をバイト列にデコードする |
| `Base58Error` | エラー型: `InvalidCharacter(char)` |

## ライセンス

MIT
