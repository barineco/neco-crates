# neco-sha2

[English](README.md)

ゼロ依存の SHA-256、HMAC-SHA256、HKDF-SHA256 実装です。

- SHA-256: RFC 6234 準拠
- HMAC-SHA256: RFC 2104 準拠
- HKDF-SHA256: RFC 5869 準拠

## 使い方

```rust
use neco_sha2::{Sha256, Hmac, Hkdf};

// SHA-256 ワンショット
let hash = Sha256::digest(b"hello");

// SHA-256 ストリーミング
let hash = Sha256::new().update(b"hel").update(b"lo").finalize();

// HMAC-SHA256
let mac = Hmac::new(b"key").update(b"message").finalize();

// HKDF-SHA256
let prk = Hkdf::extract(b"salt", b"input keying material");
let okm = prk.expand(b"info", 32).unwrap();
```

## ライセンス

MIT
