# neco-fuzzy

Fuzzy scoring and ranking for commands, paths, and short identifiers.

This crate owns scoring, ranking, prepared matching, and prepared candidate archives. Indexing, caches, watchers, path normalization, and UI highlighting stay with the caller.

## Features

- Case-insensitive subsequence matching by default
- DP-based scoring that prefers contiguous runs, boundary hits, and basename-local matches
- `ScoreConfig` for tuning position, boundary, gap, span, head, and confidence weights
- `CorpusStats` for optional ASCII IDF weighting
- Stable one-shot API and prepared API with caller-owned scratch storage
- ASCII fast path for ASCII-heavy queries and candidates
- Prepared candidate archive for persistence and warm reload

## Usage

Command search:

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

Configured search:

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

Corpus-aware search:

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

Prepared search:

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

Prepared candidate archive:

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

## Ranking

`top_k` ranks matches in this order:

1. Higher score
2. Earlier match start
3. Shorter candidate
4. Earlier input order

`Score` contains three views of the same result:

- `value`: integer score for ranking and compatibility
- `energy`: raw DP energy, where lower is better
- `confidence`: normalized confidence in `(0, 1]`

With a fixed query, the scorer still separates strong boundary-local matches from weak long-range subsequences:

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

| Item | Description |
|------|-------------|
| `Score` | Score summary with `value`, `energy`, `confidence`, byte range, and matched count |
| `Match` | Ranked output item for `top_k` |
| `PreparedQuery` | Query prepared for repeated matching |
| `PreparedCandidate` | Candidate prepared for repeated matching |
| `OwnedPreparedCandidate` | Owned prepared candidate for persistence and warm reload |
| `PreparedCandidateRef` | Borrowed prepared candidate view for cache-backed execution |
| `PreparedCandidateHeader` | Versioned archive header for encoded candidates |
| `ScoreConfig` | Score weights and conversion parameters |
| `CorpusStats` | Corpus statistics for optional IDF weighting |
| `Scratch` | Reusable working storage for prepared matching |
| `candidate_fingerprint` | Stable fingerprint for cache reuse and invalidation checks |
| `score`, `score_case_sensitive` | One-shot scoring |
| `score_with_config`, `score_with_corpus` | One-shot scoring with explicit tuning |
| `score_prepared*` | Prepared scoring for borrowed and owned candidates |
| `match_indices*` | Matched byte offsets from DP traceback |
| `top_k`, `top_k_prepared*` | Ranking for one-shot and prepared workloads |
| `top_k_with_config`, `top_k_with_corpus` | Ranking with explicit tuning |

## Notes

- `match_indices` returns byte offsets, not character counts.
- Prepared candidate archives keep text and fingerprint. Archive compatibility is tracked with `PreparedCandidateHeader`.
- `PREPARED_CANDIDATE_ALGORITHM_VERSION` is `2` in `0.2.x`.
- See [CHANGELOG.md](CHANGELOG.md) for versioned changes.

## Complexity

- Greedy subsequence filter: `O(candidate_len)`
- DP scoring for a matched candidate: `O(query_len * candidate_len)`
- Top-k selection: `O(num_candidates * log(limit))`

## License

MIT
