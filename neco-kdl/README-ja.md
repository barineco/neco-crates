# neco-kdl

ゼロ外部依存の KDL v2 パーサー/シリアライザー。設定ファイルや DSL の読み書きに使える。

## 機能

- KDL v2 仕様の完全パース
  - multiline strings、raw strings、escline
  - type annotations: `(type)node`
  - slashdash コメント (`/-`)、ブロックコメント (`/* ... */`)、ネストコメント
  - `#true` / `#false` / `#null` / `#inf` / `#-inf` / `#nan` キーワード
  - 16 進数・8 進数・2 進数リテラル、アンダースコア区切り
  - version marker (`/- kdl-version 2`)
- `serialize()` および `Display` impl によるシリアライズ (roundtrip 安全)
- 正規化出力（公式テストスイートの expected_kdl と完全一致）
- フォーマット非依存の `Value` 変換 (`Value` <-> `KdlDocument`)
- ゼロ外部依存
- 公式テストスイート全件通過

## 使い方

```toml
[dependencies]
neco-kdl = "0.2"
```

```rust
use neco_kdl::{parse, serialize, normalize};

fn main() {
    let src = r#"
        node "hello" key=#true {
            child 42
        }
    "#;

    let doc = parse(src).unwrap();

    // ノードを走査する
    for node in doc.nodes() {
        println!("{}: {} entries", node.name(), node.entries().len());
    }

    // KDL テキストに書き戻す
    let output = serialize(&doc);
    print!("{}", output);

    // 正規化形式に変換する
    let normalized = normalize(&doc);
    print!("{}", normalized);
}
```

## API

### `parse`

```rust
pub fn parse(input: &str) -> Result<KdlDocument, KdlError>
```

KDL v2 ドキュメントをパースして `KdlDocument` を返す。

### `serialize`

```rust
pub fn serialize(doc: &KdlDocument) -> String
```

`KdlDocument` を KDL テキストに変換する。`KdlDocument`、`KdlNode`、`KdlEntry`、`KdlValue` はすべて `Display` も実装している。

### `normalize`

```rust
pub fn normalize(doc: &KdlDocument) -> String
```

`KdlDocument` を正規化形式の文字列に変換する。正規化ルールは以下のとおり:

- コメントを除去
- property をキーのアルファベット順に並び替え
- 重複 property は後勝ち（最後に出現した値を採用）
- 文字列はすべて quoted string に統一
- identifier として有効な文字列はアンクォート
- インデントは 4 スペース
- 数値は 10 進数に変換し、アンダースコアを除去
- 末尾改行あり

### `value_to_kdl_document` / `kdl_document_to_value`

```rust
pub fn value_to_kdl_document(value: &Value) -> Result<KdlDocument, KdlError>
pub fn kdl_document_to_value(doc: &KdlDocument) -> Result<Value, KdlError>
```

`KdlDocument` とフォーマット非依存の `Value` enum を相互変換する。`Value` は KDL と他フォーマット (JSON, CBOR 等) の橋渡し用中間表現として使える。

### 主な型

| 項目 | 説明 |
|------|------|
| `KdlDocument` | パース結果のルート。`nodes()` でノード一覧を取得できる |
| `KdlNode` | ノード。`ty()`、`name()`、`entries()`、`children()` で各要素を参照 |
| `KdlEntry` | `Argument`（位置引数）または `Property`（名前付き引数） |
| `KdlValue` | `String(String)`、`Number(KdlNumber)`、`Bool(bool)`、`Null` |
| `KdlNumber` | `raw()`、`as_i64()`、`as_f64()` を提供。数値サイズに上限なし |
| `KdlError` | `line()`、`col()`（1-based）、`kind()` でエラー情報を参照 |
| `KdlErrorKind` | エラー種別（`UnexpectedChar`、`InvalidEscape`、`UnclosedString` 等） |
| `Value` | フォーマット非依存の中間表現: `Null`、`Bool`、`Integer`、`Float`、`String`、`Array`、`Object` |

## License

MIT
