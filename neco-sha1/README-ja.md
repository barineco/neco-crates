# neco-sha1

[English](README.md)

ゼロ依存の SHA-1 ハッシュ関数実装です。

- SHA-1: RFC 3174 準拠

## 使い方

```rust
use neco_sha1::Sha1;

// SHA-1 ワンショット
let hash = Sha1::digest(b"hello");

// SHA-1 ストリーミング
let mut h = Sha1::new();
h.update(b"hel");
h.update(b"lo");
let hash = h.finalize();
```

## ライセンス

MIT
