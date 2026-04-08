# neco-cbor

[English](README.md)

necosystems series の CBOR / DAG-CBOR codec です。`no_std` + `alloc` 環境で動作し、AT Protocol や IPLD を扱う組み込み・WASM 環境に使えます。

## 特徴

- 外部 crate なし。runtime dependency は neco-* crate のみ（`no_std` + `alloc`）
- CBOR デコード・エンコード（RFC 7049）
- DAG-CBOR デコード・エンコード（IPLD deterministic encoding）
- Tag 42（CID link）対応
- DAG-CBOR モードはマップキーのソート順・重複・非テキストキーを検証

## 使い方

### CBOR デコード

```rust
use neco_cbor::{decode, CborValue};

let bytes = &[0xa1, 0x61, 0x78, 0x01]; // {"x": 1}
let value = decode(bytes).unwrap();

let x = value.required_unsigned("x").unwrap(); // 1u64
```

### DAG-CBOR デコード

```rust
use neco_cbor::{decode_dag, CborValue};

let value = decode_dag(dag_cbor_bytes).unwrap();
```

### CBOR エンコード

```rust
use neco_cbor::{encode, CborValue};
use alloc::vec;

let value = CborValue::Map(vec![
    (CborValue::Text("n".into()), CborValue::Unsigned(42)),
]);
let bytes = encode(&value).unwrap();
```

### DAG-CBOR エンコード

```rust
use neco_cbor::{encode_dag, CborValue};

let bytes = encode_dag(&value).unwrap();
```

### Tag 42（CID link）の読み取り

```rust
use neco_cbor::CborValue;

if let Some((42, inner)) = value.as_tag() {
    let cid_bytes = inner.as_bytes().unwrap(); // 0x00 プレフィックス付きバイト列
}
```

## API

### トップレベル関数

| 項目 | 説明 |
|------|------|
| `decode(input: &[u8]) -> Result<CborValue, DecodeError>` | バイト列を CBOR としてデコードする |
| `decode_dag(input: &[u8]) -> Result<CborValue, DecodeError>` | バイト列を DAG-CBOR としてデコードする（規則違反を検証） |
| `encode(value: &CborValue) -> Result<Vec<u8>, EncodeError>` | `CborValue` を CBOR バイト列にエンコードする |
| `encode_dag(value: &CborValue) -> Result<Vec<u8>, EncodeError>` | `CborValue` を DAG-CBOR バイト列にエンコードする（マップキーをソート） |

### `CborValue`

CBOR 値を表す enum。

```
Unsigned(u64) | Negative(i64) | Bytes(Vec<u8>) | Text(String)
| Array(Vec<CborValue>) | Map(Vec<(CborValue, CborValue)>)
| Tag(u64, Box<CborValue>) | Bool(bool) | Null
```

#### 値取得（`Option`）

| 項目 | 説明 |
|------|------|
| `as_unsigned() -> Option<u64>` | 非負整数を取り出す |
| `as_negative() -> Option<i64>` | 負整数を取り出す |
| `as_bytes() -> Option<&[u8]>` | バイト列を取り出す |
| `as_text() -> Option<&str>` | 文字列スライスを取り出す |
| `as_array() -> Option<&[CborValue]>` | 配列スライスを取り出す |
| `as_map() -> Option<&[(CborValue, CborValue)]>` | マップエントリのスライスを取り出す |
| `as_tag() -> Option<(u64, &CborValue)>` | タグ番号と内部値を取り出す |
| `as_bool() -> Option<bool>` | bool を取り出す |
| `is_null() -> bool` | `Null` のとき `true` |
| `get(key: &str) -> Option<&CborValue>` | テキストキーでマップフィールドを検索する |

#### 必須フィールドアクセサ

マップでない・フィールド欠落・型不一致のとき `Err(AccessError)` を返す。

| 項目 | 説明 |
|------|------|
| `required_text(key) -> Result<&str, AccessError>` | 必須テキストフィールド |
| `required_bytes(key) -> Result<&[u8], AccessError>` | 必須バイトフィールド |
| `required_unsigned(key) -> Result<u64, AccessError>` | 必須非負整数フィールド |
| `required_negative(key) -> Result<i64, AccessError>` | 必須負整数フィールド |
| `required_bool(key) -> Result<bool, AccessError>` | 必須 bool フィールド |
| `required_array(key) -> Result<&[CborValue], AccessError>` | 必須配列フィールド |
| `required_map(key) -> Result<&[(CborValue, CborValue)], AccessError>` | 必須マップフィールド |
| `required_tag(key) -> Result<(u64, &CborValue), AccessError>` | 必須タグフィールド |

### エラー型

| 項目 | 説明 |
|------|------|
| `DecodeError` | バイト `position` と `kind` を持つデコードエラー |
| `DecodeErrorKind` | デコード失敗の具体的な原因（下記参照） |
| `EncodeError` | エンコードエラー（下記参照） |
| `AccessError` | フィールドアクセスエラー: `NotAMap`, `MissingField`, `TypeMismatch` |

#### `DecodeErrorKind` バリアント

| バリアント | 説明 |
|-----------|------|
| `UnexpectedEnd` | 入力が途中で終了した |
| `InvalidMajorType(u8)` | 未知のメジャータイプ |
| `NestingTooDeep` | ネストが深すぎる |
| `IndefiniteLength` | 不定長アイテムは非対応 |
| `FloatNotAllowed` | 浮動小数点は非対応 |
| `UnsortedMapKeys` | DAG-CBOR のマップキーがソートされていない |
| `NonCanonicalInteger` | 整数が最小エンコードになっていない |
| `DuplicateMapKey` | マップキーが重複している |
| `TrailingContent` | CBOR 値の後に余剰バイトがある |
| `InvalidUtf8` | テキストが有効な UTF-8 でない |
| `NonTextMapKey` | DAG-CBOR モードでマップキーがテキストでない |
| `UnsupportedTag(u64)` | 非対応のタグ番号 |
| `IntegerOverflow` | 整数が対象表現に収まらない |

#### `EncodeError` バリアント

| バリアント | 説明 |
|-----------|------|
| `NonTextKeyInDagMode` | DAG-CBOR モードでマップキーがテキストでない |
| `DuplicateKeyInDagMode` | DAG-CBOR モードでマップキーが重複している |
| `UnsupportedTag(u64)` | 非対応のタグ番号 |
| `InvalidTag42Payload` | Tag 42 のペイロードが `0x00` プレフィックス付きバイト列でない |
| `InvalidNegativeValue(i64)` | `Negative` の値が 0 以上になっている |

## ライセンス

MIT
