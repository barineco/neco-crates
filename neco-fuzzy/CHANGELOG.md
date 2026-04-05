# Changelog

## 0.2.0

- `ScoreConfig` と `CorpusStats` を追加
- `Score` に `energy` と `confidence` を追加
- スコアリングを DP energy minimization ベースへ変更
- Top-K 選択を `BinaryHeap` ベースへ変更
- prepared candidate archive の `algorithm_version` を `2` へ更新
- 既存 API は `ScoreConfig::default()` を使う形で継続
- `Score` の公開フィールド構成変更を含むため、0.1.x からは破壊的変更

## 0.1.0

- `score`, `score_case_sensitive`, `match_indices`, `top_k` の基本 API を追加
- prepared matching API と candidate archive 基盤を追加
- command/path 向けの境界ボーナス付き fuzzy scoring を追加
