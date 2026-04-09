# neco-json

[English](README.md)

外部依存ゼロ、`no_std` + `alloc` 環境で動作する最小 JSON codec です。パース、エンコード、型安全なフィールドアクセスに加えて、`JsonValue` ベースの軽量な `ToJson` / `FromJson` 変換 trait を提供します。

## 使い方

### パース

```rust
use neco_json::{parse, JsonValue};

let json = br#"{"name":"neco","score":42.5,"active":true}"#;
let value = parse(json).unwrap();
```

### フィールドアクセス

```rust
use neco_json::{parse, JsonValue};

let json = br#"{"name":"neco","score":42.5,"active":true,"tag":null}"#;
let value = parse(json).unwrap();

// 必須フィールド — 欠落・型不一致はエラー
let name   = value.required_str("name").unwrap();    // "neco"
let score  = value.required_f64("score").unwrap();   // 42.5
let active = value.required_bool("active").unwrap(); // true

// 省略可能フィールド — 欠落・null のとき Ok(None)
let tag = value.optional_str("tag").unwrap(); // None
```

### エンコード

```rust
use neco_json::{encode, JsonValue};
use alloc::vec;

let value = JsonValue::Object(vec![
    ("x".into(), JsonValue::Number(1.0)),
    ("ok".into(), JsonValue::Bool(true)),
]);
let bytes = encode(&value).unwrap(); // b"{\"x\":1.0,\"ok\":true}"
```

### `ToJson` / `FromJson`

```rust
use neco_json::{FromJson, ToJson};

let json = vec![1_u64, 2, 3].to_json();
let restored = Vec::<u64>::from_json(&json).unwrap();
assert_eq!(restored, vec![1, 2, 3]);
```

## API

### トップレベル関数

| 項目 | 説明 |
|------|------|
| `parse(input: &[u8]) -> Result<JsonValue, ParseError>` | バイト列を JSON としてパースする |
| `encode(value: &JsonValue) -> Result<Vec<u8>, EncodeError>` | `JsonValue` を最小 JSON バイト列にエンコードする |
| `ToJson` / `FromJson` | Rust 値と `JsonValue` の相互変換を行う軽量 trait |

### `JsonValue`

JSON 値を表す enum。

```
Null | Bool(bool) | Number(f64) | String(String) | Array(Vec<JsonValue>) | Object(Vec<(String, JsonValue)>)
```

#### 型判定

| 項目 | 説明 |
|------|------|
| `is_null()` | `Null` のとき `true` |
| `is_bool()` | `Bool` のとき `true` |
| `is_number()` | `Number` のとき `true` |
| `is_string()` | `String` のとき `true` |
| `is_array()` | `Array` のとき `true` |
| `is_object()` | `Object` のとき `true` |

#### 値取得（`Option`）

| 項目 | 説明 |
|------|------|
| `as_bool() -> Option<bool>` | bool を取り出す |
| `as_f64() -> Option<f64>` | 数値を取り出す |
| `as_str() -> Option<&str>` | 文字列スライスを取り出す |
| `as_array() -> Option<&[JsonValue]>` | 配列スライスを取り出す |
| `as_object() -> Option<&[(String, JsonValue)]>` | オブジェクトフィールドのスライスを取り出す |
| `get(key) -> Option<&JsonValue>` | キーでオブジェクトフィールドを検索する |

#### 必須フィールドアクセサ

オブジェクトでない・フィールド欠落・型不一致のとき `Err(AccessError)` を返す。

| 項目 | 説明 |
|------|------|
| `required_str(key) -> Result<&str, AccessError>` | 必須文字列フィールド |
| `required_bool(key) -> Result<bool, AccessError>` | 必須 bool フィールド |
| `required_f64(key) -> Result<f64, AccessError>` | 必須数値フィールド |
| `required_array(key) -> Result<&[JsonValue], AccessError>` | 必須配列フィールド |
| `required_object(key) -> Result<&[(String, JsonValue)], AccessError>` | 必須オブジェクトフィールド |

#### 省略可能フィールドアクセサ

フィールド欠落・`null` のとき `Ok(None)`、型不一致のとき `Err(AccessError)` を返す。

| 項目 | 説明 |
|------|------|
| `optional_str(key) -> Result<Option<&str>, AccessError>` | 省略可能文字列フィールド |
| `optional_bool(key) -> Result<Option<bool>, AccessError>` | 省略可能 bool フィールド |
| `optional_f64(key) -> Result<Option<f64>, AccessError>` | 省略可能数値フィールド |
| `optional_array(key) -> Result<Option<&[JsonValue]>, AccessError>` | 省略可能配列フィールド |

### エラー型

| 項目 | 説明 |
|------|------|
| `ParseError` | バイト `position` と `kind` を持つパースエラー |
| `ParseErrorKind` | パース失敗の具体的な原因 |
| `EncodeError` | エンコードエラー — 現在は `NonFiniteNumber` のみ |
| `AccessError` | フィールドアクセスエラー: `NotAnObject`, `MissingField`, `TypeMismatch` |

## ライセンス

MIT
