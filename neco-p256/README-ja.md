# neco-p256

[English](README.md)

P-256 (NIST) ECDSA の署名・検証を提供する最小 crate です。依存は [RustCrypto の p256](https://crates.io/crates/p256) のみです。

## 特徴

- P-256 ECDSA prehash 署名・検証
- 署名側で low-S 正規化、検証側で high-S 拒否
- 公開鍵は SEC1 compressed 形式（33 バイト）
- unsafe コードなし

## 使い方

```rust
use neco_p256::{SecretKey, EcdsaSignature};

let secret = SecretKey::generate().unwrap();
let public = secret.public_key().unwrap();
let digest: [u8; 32] = [0x42; 32]; // SHA-256 ダイジェスト

let sig: EcdsaSignature = secret.sign_ecdsa_prehash(digest).unwrap();
public.verify_ecdsa_prehash(digest, &sig).unwrap();
```

鍵を hex 文字列で扱うこともできます。

```rust
use neco_p256::SecretKey;

let secret = SecretKey::generate().unwrap();
let hex = secret.to_hex();
let restored = SecretKey::from_hex(&hex).unwrap();
assert_eq!(secret, restored);
```

公開鍵は SEC1 compressed bytes で往復できます。

```rust
use neco_p256::PublicKey;

let secret = neco_p256::SecretKey::generate().unwrap();
let public = secret.public_key().unwrap();
let bytes: [u8; 33] = public.to_sec1_bytes();
let restored = PublicKey::from_sec1_bytes(&bytes).unwrap();
assert_eq!(public, restored);
```

## API

### `SecretKey`

| メソッド | 説明 |
|---------|------|
| `generate() -> Result<Self, P256Error>` | ランダムな秘密鍵を生成する |
| `from_bytes(bytes: [u8; 32]) -> Result<Self, P256Error>` | 32 バイト配列から構築する |
| `from_hex(hex: &str) -> Result<Self, P256Error>` | 64 文字の hex 文字列から構築する |
| `to_bytes() -> [u8; 32]` | 32 バイト配列として返す |
| `to_hex() -> String` | hex 文字列として返す |
| `public_key() -> Result<PublicKey, P256Error>` | 対応する公開鍵を返す |
| `sign_ecdsa_prehash(digest32: [u8; 32]) -> Result<EcdsaSignature, P256Error>` | P-256 ECDSA prehash 署名（low-S 正規化） |

### `PublicKey`

| メソッド | 説明 |
|---------|------|
| `from_sec1_bytes(bytes: &[u8]) -> Result<Self, P256Error>` | SEC1 compressed bytes から構築する |
| `from_hex(hex: &str) -> Result<Self, P256Error>` | 66 文字の hex 文字列から構築する |
| `to_sec1_bytes() -> [u8; 33]` | SEC1 compressed bytes として返す |
| `to_hex() -> String` | hex 文字列として返す |
| `verify_ecdsa_prehash(digest32: [u8; 32], sig: &EcdsaSignature) -> Result<(), P256Error>` | P-256 ECDSA prehash 検証（high-S 拒否） |

### `EcdsaSignature`

64 バイトの ECDSA 署名（raw r\|\|s compact 形式）。

| メソッド | 説明 |
|---------|------|
| `from_bytes(bytes: [u8; 64]) -> Self` | 64 バイト配列から構築する |
| `from_hex(hex: &str) -> Result<Self, P256Error>` | 128 文字の hex 文字列から構築する |
| `to_bytes() -> [u8; 64]` | 64 バイト配列として返す |
| `to_hex() -> String` | hex 文字列として返す |

### `P256Error`

| バリアント | 説明 |
|-----------|------|
| `InvalidSecretKey` | 秘密鍵のバイト列が無効 |
| `InvalidPublicKey` | 公開鍵のバイト列が無効 |
| `InvalidSignature` | 署名が無効（検証失敗または high-S） |
| `InvalidHex(&'static str)` | hex デコードに失敗した |

## ライセンス

MIT
