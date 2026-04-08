# neco-cid

[English](README.md)

SHA-256 依存のみで動作する最小 CIDv1 ライブラリです。IPLD / AT Protocol で必要な CID 計算・multibase エンコード／デコードに使えます。

## 特徴

- 依存は `sha2` のみ（コア機能）
- CIDv1 の計算・バイト列シリアライズ・デシリアライズ
- multibase エンコード／デコード（base32lower、`b` プレフィックス）
- サポートコーデック: `dag-cbor` (0x71)、`raw` (0x55)
- ハッシュ関数: SHA-256 固定

## 使い方

### CID を計算する

```rust
use neco_cid::{Cid, Codec, Base};

let data = b"hello world";
let cid = Cid::compute(Codec::DagCbor, data);

let s = cid.to_multibase(Base::Base32Lower);
// "bafyrei..." のような base32lower 文字列
```

### CID 文字列からデコードする

```rust
use neco_cid::Cid;

let cid = Cid::from_multibase("bafyreib...").unwrap();
```

### バイト列との相互変換

```rust
use neco_cid::Cid;

let bytes = cid.to_bytes();
let (cid2, consumed) = Cid::from_bytes(&bytes).unwrap();
assert_eq!(cid, cid2);
assert_eq!(consumed, bytes.len());
```

## API

### `Cid`

CIDv1 を表す構造体。`Debug`, `Clone`, `PartialEq`, `Eq`, `Hash` を実装。

| 項目 | 説明 |
|------|------|
| `Cid::compute(codec: Codec, data: &[u8]) -> Cid` | データの SHA-256 ハッシュから CID を生成する |
| `Cid::from_bytes(input: &[u8]) -> Result<(Cid, usize), CidError>` | バイト列から CID をデシリアライズし、消費バイト数を返す |
| `Cid::to_bytes(&self) -> Vec<u8>` | CID をバイト列にシリアライズする |
| `Cid::to_multibase(&self, base: Base) -> String` | 指定ベースで multibase 文字列にエンコードする |
| `Cid::from_multibase(input: &str) -> Result<Cid, CidError>` | multibase 文字列から CID をデコードする |
| `Cid::codec(&self) -> Codec` | コーデックを返す |
| `Cid::digest(&self) -> &[u8; 32]` | SHA-256 ダイジェストを返す |

### `Codec`

| バリアント | 値 | 説明 |
|-----------|-----|------|
| `Codec::DagCbor` | `0x71` | DAG-CBOR |
| `Codec::Raw` | `0x55` | Raw バイト列 |

### `Base`

| バリアント | プレフィックス | 説明 |
|-----------|--------------|------|
| `Base::Base32Lower` | `b` | RFC 4648 base32 小文字（パディングなし） |

### `CidError`

| バリアント | 説明 |
|-----------|------|
| `InvalidVersion(u64)` | CIDv1 以外のバージョン |
| `UnsupportedCodec(u64)` | 非対応のコーデック |
| `UnsupportedHashCode(u64)` | 非対応のハッシュコード |
| `InvalidDigestLength` | ダイジェスト長が不正 |
| `InvalidMultibase` | multibase 文字列の形式が不正 |
| `UnexpectedEnd` | 入力が途中で終了した |

## ライセンス

MIT
