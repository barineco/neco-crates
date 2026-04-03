# neco-fuzzy

コマンド名、パス、短い識別子向けの最小 fuzzy スコアライブラリです。

このライブラリは fuzzy スコアと順位付けの責務だけを担い、ファイルのインデックス、キャッシュ、監視、パス正規化、UI のハイライトは利用側の責任です。

## 機能

- 大文字小文字を区別しない部分一致マッチを既定値とする
- `/`, `_`, `-`, `.`, 空白、camelCase の区切り遷移を優遇
- コマンド検索とパス検索の順序を安定化
- 一回呼び出し向けに安定 API を提供
- 繰り返し検索向けに準備済み API と呼び出し側所有の一時作業領域を用意
- ASCII 中心のワークロード向けに高速経路を持つ
- キャッシュ永続化と再読込向けの候補アーカイブ基盤を持つ

## 使い方

コマンド検索:

```rust
use neco_fuzzy::{score, top_k, Match};

let candidates = ["open-file", "open-folder", "copy-path"];
let mut out: Vec<Match<'_>> = Vec::new();
top_k("of", &candidates, 2, &mut out);

assert_eq!(out[0].candidate, "open-file");
assert!(score("cp", "copy-path").is_some());
```

パス検索:

```rust
use neco_fuzzy::{match_indices, top_k, Match};

let candidates = ["src/lib/commands.ts", "src/components/StatusCommandBar.vue"];
let mut out: Vec<Match<'_>> = Vec::new();
top_k("cmd", &candidates, 2, &mut out);

assert_eq!(out[0].candidate, "src/lib/commands.ts");

let mut indices = Vec::new();
assert!(match_indices("cmd", "src/lib/commands.ts", &mut indices));
assert_eq!(indices, vec![0, 8, 10]);
```

準備済み検索:

```rust
use neco_fuzzy::{PreparedCandidate, PreparedQuery, Scratch, Match, top_k_prepared};

let query = PreparedQuery::new("cmd");
let candidates = [
    PreparedCandidate::new("src/lib/commands.ts"),
    PreparedCandidate::new("src/components/StatusCommandBar.vue"),
];
let mut scratch = Scratch::default();
let mut out: Vec<Match<'_>> = Vec::new();
top_k_prepared(&query, &candidates, 2, &mut out, &mut scratch);

assert_eq!(out[0].candidate, "src/lib/commands.ts");
```

候補アーカイブ:

```rust
use neco_fuzzy::{OwnedPreparedCandidate, PreparedQuery, Scratch, score_prepared_owned};

let owned = OwnedPreparedCandidate::new("src/lib/commands.ts");
let mut bytes = vec![0; owned.encoded_len()];
owned.encode_into(&mut bytes).unwrap();
let restored = OwnedPreparedCandidate::decode(&bytes).unwrap();

let query = PreparedQuery::new("cmd");
let mut scratch = Scratch::default();
assert!(score_prepared_owned(&query, &restored, &mut scratch).is_some());
```

## 並び順

`top_k` は次の順で並べます。

1. スコアが高い
2. マッチ開始位置が早い
3. 候補が短い
4. 元の入力順が早い

先頭一致、連続一致、区切り一致、パスのベース名付近の一致を優遇します。

## API 層

- 安定 API: `score`, `score_case_sensitive`, `match_indices`, `top_k`
- 準備済み API: `PreparedQuery`, `PreparedCandidate`, `Scratch`, `score_prepared`, `match_indices_prepared`, `top_k_prepared`
- アーカイブ API: `OwnedPreparedCandidate`, `PreparedCandidateRef`, `PreparedCandidateHeader`, `candidate_fingerprint`

単発の呼び出しは安定 API、同じクエリや候補群を繰り返し評価する場合は準備済み API を使います。
プロセス外保存と再読込は候補アーカイブ基盤を使います。

## 制約

- ファイルのインデックスや、スコアのキャッシュは持ちません
- `match_indices` の返り値は文字数ではなくバイトオフセット
- 初版は最大スループットより、短いクエリと中程度の候補長でわかりやすい仕様を優先

## 計算量の目安

- `score`: 短いクエリを前提におおむね `O(candidate_len)`
- `top_k`: `O(n * candidate_len)` のスコア計算に、上位 `limit` 件の維持と最後の `O(limit log limit)` ソートが加わる

## API

| 項目 | 説明 |
|------|------|
| `Score` | スコア値、バイト範囲、マッチ文字数を持つ要約 |
| `Match` | `top_k` の順位付き出力 |
| `PreparedQuery` | 繰り返し検索向けに前処理したクエリ |
| `PreparedCandidate` | 繰り返し検索向けに前処理した候補 |
| `OwnedPreparedCandidate` | 永続化と再読込向けの準備済み候補 |
| `PreparedCandidateRef` | キャッシュ実行向けの借用候補ビュー |
| `PreparedCandidateHeader` | エンコード済み候補向けのバージョン付きヘッダ |
| `Scratch` | 準備済み一致用の再利用可能な作業領域 |
| `candidate_fingerprint` | キャッシュ再利用と無効化判定向けの安定な指紋値 |
| `score` | 大文字小文字を区別しない fuzzy スコア |
| `score_case_sensitive` | 大文字小文字を区別する fuzzy スコア |
| `score_prepared` | 準備済みクエリと候補でスコアを計算 |
| `score_prepared_ref` | 借用した候補ビューでスコアを計算 |
| `score_prepared_owned` | 所有した候補でスコアを計算 |
| `match_indices` | 呼び出し側所有バッファへマッチのバイトオフセットを書き込む |
| `match_indices_prepared` | 準備済み入力でマッチのバイトオフセットを書き込む |
| `match_indices_prepared_ref` | 借用した候補ビューでマッチのバイトオフセットを書き込む |
| `top_k` | 呼び出し側所有バッファへ上位候補を書き込む |
| `top_k_prepared` | 準備済み候補群を呼び出し側所有の一時作業領域で順位付けする |
| `top_k_prepared_refs` | 借用した準備済み候補ビュー群を呼び出し側所有の一時作業領域で順位付けする |

## ライセンス

MIT
