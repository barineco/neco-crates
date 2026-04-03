# neco-watchnorm

[英語版](README.md)

`neco-watchnorm` は、ホストごとの差があるファイル監視イベントを、後段の実行時コードが扱いやすい決定的な一括 API へ正規化するクレートです。

## イベント正規化

このクレートは `RawWatchEvent` を受け取り、`drain()` 時により小さな正規化イベント列へ変換します。呼び出し側は世代番号を渡し、正規化器は古いイベントを捨て、名前変更の片側同士を結合し、結合できない情報は推測せず `PartialRename` として残します。

`Modify` の統合は保守的です。同じパスの連続 `Modify` はまとめ、`Create` 直後の `Modify` は吸収し、同一バッチ内の `Remove` はそのパスに対する先行 `Modify` を落とします。

## 使い方

```rust
use neco_watchnorm::{
    NormalizedWatchKind, RawWatchEvent, RawWatchKind, RenameHint, WatchBatchNormalizer,
};

let mut normalizer = WatchBatchNormalizer::new();
normalizer.push(RawWatchEvent {
    kind: RawWatchKind::Rename,
    paths: vec!["/old.txt".into()],
    rename_from: Some("/old.txt".into()),
    rename_to: None,
    rename_hint: Some(RenameHint::From),
    generation: 2,
});
normalizer.push(RawWatchEvent {
    kind: RawWatchKind::Rename,
    paths: vec!["/new.txt".into()],
    rename_from: None,
    rename_to: Some("/new.txt".into()),
    rename_hint: Some(RenameHint::To),
    generation: 2,
});

let result = normalizer.drain(2);
assert_eq!(result.events.len(), 1);
assert!(matches!(
    result.events[0].kind,
    NormalizedWatchKind::Rename { .. }
));
```

## API

| 項目 | 説明 |
|------|-------------|
| `RawWatchEvent` | 世代番号を含むホスト非依存の一括入力イベント |
| `RawWatchKind` | create / remove / modify / rename の種別 |
| `RenameHint` | ホスト橋渡し層が渡す名前変更側ヒント |
| `NormalizedWatchEvent` | 世代番号付きの利用側向け正規化イベント |
| `NormalizedWatchKind` | `Create`, `Remove`, `Modify`, `Rename`, `PartialRename` |
| `WatchBatchNormalizer` | `push` と `drain` を持つ状態付き一括正規化器 |
| `FlushResult` | 正規化後イベントと破棄した古いイベント件数 |

## ライセンス

MIT
