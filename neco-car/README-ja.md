# neco-car

[English](README.md)

IPFS/IPLD エコシステム向けの CAR v1 (Content Addressable aRchive) パーサー・ライターです。DAG-CBOR ヘッダーのデコードに [neco-cbor](../neco-cbor)、CID パースに [neco-cid](../neco-cid) を使用します。

## 特徴

- CAR v1 ファイルのパース（varint プレフィックス付き DAG-CBOR ヘッダー + varint プレフィックス付き CID+データブロック）
- ルートリストと生ブロックから CAR v1 ファイルを書き出し
- パース・書き出しのすべての失敗ケースを網羅した `CarError` enum
- unsafe コードなし

## 使い方

### CAR v1 ファイルのパース

```rust
use neco_car::{parse_v1, CarV1};

let bytes: &[u8] = /* 生の CAR バイト列 */ &[];
let car = parse_v1(bytes).unwrap();

for root in car.roots() {
    println!("root: {root}");
}

for block in car.blocks() {
    println!("cid={}, len={}", block.cid(), block.data().len());
}
```

### 所有権ごとに分解する

```rust
let (roots, blocks) = car.into_parts();
for (cid, data) in blocks.into_iter().map(|e| e.into_parts()) {
    // cid: Cid, data: Vec<u8>
}
```

### CAR v1 ファイルの書き出し

```rust
use neco_car::write_v1;
use neco_cid::Cid;

let root: Cid = /* ルート CID */ todo!();
let block_data: &[u8] = b"...";

let car_bytes = write_v1(&[root.clone()], &[(root, block_data)]).unwrap();
```

## API

### トップレベル関数

| 項目 | 説明 |
|------|------|
| `parse_v1(input: &[u8]) -> Result<CarV1, CarError>` | バイト列を CAR v1 アーカイブとしてパースする |
| `write_v1(roots: &[Cid], blocks: &[(Cid, &[u8])]) -> Result<Vec<u8>, CarError>` | ルートとブロックを CAR v1 アーカイブとしてエンコードする |

### `CarV1`

パース済みの CAR v1 アーカイブを表す構造体。

| メソッド | 説明 |
|---------|------|
| `roots() -> &[Cid]` | ヘッダーで宣言されたルート CID のスライス |
| `blocks() -> &[CarEntry]` | アーカイブ内の全ブロックのスライス |
| `into_parts() -> (Vec<Cid>, Vec<CarEntry>)` | 所有権ごとルートとブロックに分解する |

### `CarEntry`

CAR アーカイブ内の1ブロック。

| メソッド | 説明 |
|---------|------|
| `cid() -> &Cid` | このブロックを識別する CID |
| `data() -> &[u8]` | 生のブロックデータ |
| `into_parts() -> (Cid, Vec<u8>)` | 所有権ごと CID とデータに分解する |

### `CarError`

| バリアント | 説明 |
|-----------|------|
| `UnexpectedEnd` | 入力が途中で終了した |
| `VarintOverflow` | varint 値が 64 ビット範囲を超えた |
| `InvalidHeader(DecodeErrorKind)` | DAG-CBOR ヘッダーのデコードに失敗した |
| `HeaderNotMap` | ヘッダーが CBOR マップでない |
| `MissingHeaderField(&'static str)` | 必須ヘッダーフィールド（`version` または `roots`）がない |
| `UnsupportedVersion(u64)` | CAR バージョンが 1 でない |
| `RootsNotArray` | `roots` フィールドが CBOR 配列でない |
| `InvalidRootCid(CidError)` | ルート CID のパースに失敗した |
| `InvalidBlockCid(CidError)` | ブロック CID のパースに失敗した |
| `BlockLengthMismatch` | ブロックセクション長が CID サイズと一致しない |
| `EmptySection` | ブロックセクションの長さがゼロ |
| `InvalidCidLink` | ルート CID リンクが有効な tag-42 DAG-CBOR リンクでない |
| `HeaderEncode(EncodeError)` | 書き出し時に DAG-CBOR ヘッダーのエンコードに失敗した |

## ライセンス

MIT
