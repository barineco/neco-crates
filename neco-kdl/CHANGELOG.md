# Changelog

## 0.2.0

- `serialize` 関数と `Display` impl を追加 (parse → serialize の roundtrip が可能に)
- `KdlDocument`, `KdlNode`, `KdlNumber` の構造体フィールドを `pub` に変更
- `Cargo.toml` の description を更新

## 0.1.1

- `Value` enum と `value_to_kdl_document` / `kdl_document_to_value` 変換 API を追加
- `KdlNode` に `get`, `get_value` アクセサを追加
- `KdlValue` に `as_str`, `as_i64`, `as_f64`, `as_bool` アクセサを追加

## 0.1.0

- KDL v2 仕様の完全パース
- `normalize` による正規化出力
- 公式テストスイート全件通過
