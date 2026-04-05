# neco-fuzzy

コマンド名、パス、短い識別子向けの fuzzy スコアと順位付けを行う crate です。

この crate はスコア計算、順位付け、準備済み検索、準備済み候補アーカイブを担当します。インデックス、キャッシュ、監視、パス正規化、UI ハイライトは利用側の責務です。

## 機能

- 大文字小文字を区別しない部分列マッチを既定値とする
- 連続一致、区切り一致、ベース名付近の一致を優先する DP ベースのスコアリング
- 位置、区切り、ギャップ、スパン、先頭一致、confidence を調整できる `ScoreConfig`
- ASCII の IDF 重み付けに使う `CorpusStats`
- 単発呼び出し向けの安定 API と、呼び出し側所有の作業領域を使う準備済み API
- ASCII 中心のワークロード向けの高速経路
- 永続化と再読込に使える準備済み候補アーカイブ

## 使い方

コマンド検索:

```rust
use neco_fuzzy::{score, top_k, Match};

let candidates = ["open-file", "open-folder", "copy-path"];
let mut out: Vec<Match<'_>> = Vec::new();
top_k("of", &candidates, 2, &mut out);

assert_eq!(out[0].candidate, "open-file");

let score = score("cp", "copy-path").expect("must match");
assert!(score.value > 0);
assert!(score.confidence > 0.0);
```

設定付き検索:

```rust
use neco_fuzzy::{score_with_config, top_k_with_config, Match, ScoreConfig};

let config = ScoreConfig {
    w_gap: 2.0,
    ..ScoreConfig::default()
};
let candidates = ["foo_bar", "foobar"];
let mut out: Vec<Match<'_>> = Vec::new();
top_k_with_config("fb", &candidates, 2, &config, &mut out);

assert_eq!(out[0].candidate, "foo_bar");
assert!(score_with_config("fb", "foo_bar", &config).is_some());
```

コーパス統計付き検索:

```rust
use neco_fuzzy::{score_with_corpus, CorpusStats, ScoreConfig};

let stats = CorpusStats::from_candidates(&["foo_bar", "foobar", "quxbuzz"]);
let config = ScoreConfig {
    w_idf: 2.0,
    ..ScoreConfig::default()
};

let score = score_with_corpus("fb", "foo_bar", &config, &stats).expect("must match");
assert!(score.energy.is_finite());
```

準備済み検索:

```rust
use neco_fuzzy::{top_k_prepared, Match, PreparedCandidate, PreparedQuery, Scratch};

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

準備済み候補アーカイブ:

```rust
use neco_fuzzy::{OwnedPreparedCandidate, PreparedQuery, Scratch, score_prepared_owned};

let owned = OwnedPreparedCandidate::new("src/lib/commands.ts");
let mut bytes = vec![0; owned.encoded_len()];
owned.encode_into(&mut bytes).expect("encode");
let restored = OwnedPreparedCandidate::decode(&bytes).expect("decode");

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

`Score` は同じ結果を 3 つの見方で返します。

- `value`: 順位付けと互換性維持に使う整数スコア
- `energy`: DP の生エネルギー値。低いほど良い
- `confidence`: `(0, 1]` の正規化 confidence

入力を固定しても、区切りに沿った強い一致と、遠く離れた弱い部分列一致は `value` と `confidence` で分かれます。

| Query | Candidate | Value | Confidence |
|------|------|------:|------:|
| `abc` | `abc` | `49` | `0.615` |
| `abc` | `abC` | `40` | `0.595` |
| `abc` | `a_b_c` | `40` | `0.594` |
| `abc` | `abc_suffix` | `-14` | `0.466` |
| `abc` | `AlphaBetaCode` | `-79` | `0.320` |
| `abc` | `prefix_abc` | `-151` | `0.192` |
| `abc` | `unrelated-text` | `—` | `—` |

## API

| 項目 | 説明 |
|------|------|
| `Score` | `value`, `energy`, `confidence`, バイト範囲、マッチ文字数を持つ要約 |
| `Match` | `top_k` の順位付き出力 |
| `PreparedQuery` | 繰り返し検索向けに前処理したクエリ |
| `PreparedCandidate` | 繰り返し検索向けに前処理した候補 |
| `OwnedPreparedCandidate` | 永続化と再読込向けの準備済み候補 |
| `PreparedCandidateRef` | キャッシュ実行向けの借用候補ビュー |
| `PreparedCandidateHeader` | エンコード済み候補向けのバージョン付きヘッダ |
| `ScoreConfig` | スコア重みと変換係数 |
| `CorpusStats` | オプションの IDF 重み付けに使うコーパス統計 |
| `Scratch` | 準備済み一致用の再利用可能な作業領域 |
| `candidate_fingerprint` | キャッシュ再利用と無効化判定向けの安定な指紋値 |
| `score`, `score_case_sensitive` | 単発のスコア計算 |
| `score_with_config`, `score_with_corpus` | 明示的な設定付きの単発スコア計算 |
| `score_prepared*` | 借用候補と所有候補に対応した準備済みスコア計算 |
| `match_indices*` | DP traceback から得たマッチのバイトオフセット |
| `top_k`, `top_k_prepared*` | 単発向けと準備済み向けの順位付け |
| `top_k_with_config`, `top_k_with_corpus` | 明示的な設定付きの順位付け |

## 注意点

- `match_indices` は文字数ではなくバイトオフセットを返します
- 準備済み候補アーカイブは文字列本体と fingerprint を保持し、互換性は `PreparedCandidateHeader` で管理します
- `0.2.x` の `PREPARED_CANDIDATE_ALGORITHM_VERSION` は `2` です
- 版ごとの差分は [CHANGELOG.md](CHANGELOG.md) にまとめています

## 計算量の目安

- 貪欲な部分列フィルタ: `O(candidate_len)`
- マッチ可能な候補に対する DP スコア計算: `O(query_len * candidate_len)`
- Top-K 選択: `O(num_candidates * log(limit))`

## ライセンス

MIT
