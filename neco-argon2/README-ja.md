# neco-argon2

[English](README.md)

Blake2b を内部ハッシュに用いたゼロ依存の Argon2id パスワードハッシュライブラリです。

- Argon2id: RFC 9106 準拠
- Blake2b: RFC 7693 準拠
- PHC string フォーマット対応 (`$argon2id$v=19$m=...,t=...,p=...$...`)

## 使い方

```rust
use neco_argon2::{Argon2Params, argon2id_hash_encoded, argon2id_verify};

let password = b"hunter2";
let params = Argon2Params::default(); // m=19456, t=2, p=1

// ハッシュ化 (ランダム salt を自動生成)
let encoded = argon2id_hash_encoded(password, params);
// => "$argon2id$v=19$m=19456,t=2,p=1$<salt_b64>$<hash_b64>"

// 検証
assert!(argon2id_verify(&encoded, password));
assert!(!argon2id_verify(&encoded, b"wrong"));
```

### 低レベル API

```rust
use neco_argon2::argon2id_hash;

let hash = argon2id_hash(
    b"password",
    b"randomsalt12345!",  // 最低 8 バイト
    19456,               // m_cost (KiB)
    2,                   // t_cost (反復回数)
    1,                   // p_cost (並列度)
    32,                  // 出力長 (バイト)
);
```

## パラメータの目安

| パラメータ | 説明 | 推奨値 |
|-----------|------|--------|
| `m_cost` | メモリコスト (KiB) | 19456 (19 MiB) |
| `t_cost` | 反復回数 | 2 |
| `p_cost` | 並列度 | 1 |
| `output_len` | 出力長 (バイト) | 32 |

詳細は [MATH.md](MATH.md) を参照。

## 依存

- [`getrandom`](https://docs.rs/getrandom): salt 生成 (OS 乱数源)
- [`neco-base64`](https://docs.rs/neco-base64): PHC string の base64 エンコード

## ライセンス

MIT
