# neco-base64

[English](README.md)

外部依存ゼロの Base64 エンコーダ/デコーダです。標準および URL-safe アルファベットに対応しています。

## 使い方

```rust
use neco_base64::{encode, decode, encode_url, decode_url};

let encoded = encode(b"hello");       // "aGVsbG8="
let decoded = decode("aGVsbG8=").unwrap(); // b"hello"

let url_encoded = encode_url(b"hello");         // "aGVsbG8"
let url_decoded = decode_url("aGVsbG8").unwrap(); // b"hello"
```

## API

### エンコード

| 関数 | 説明 |
|------|------|
| `encode(input: &[u8]) -> String` | 標準 Base64 (パディング付き) |
| `encode_url(input: &[u8]) -> String` | URL-safe Base64 (パディングなし) |
| `encode_url_padded(input: &[u8]) -> String` | URL-safe Base64 (パディング付き) |

### デコード

| 関数 | 説明 |
|------|------|
| `decode(input: &str) -> Result<Vec<u8>, Base64Error>` | 標準 Base64 (パディング省略可) |
| `decode_url(input: &str) -> Result<Vec<u8>, Base64Error>` | URL-safe Base64 (パディング省略可) |
| `decode_url_strict(input: &str) -> Result<Vec<u8>, Base64Error>` | URL-safe、パディング文字拒否 + 末尾余りビットゼロ検証 |

### エラー型

```rust
pub enum Base64Error {
    InvalidCharacter,
    InvalidLength,
    NonZeroPaddingBits,
}
```

## ライセンス

MIT
