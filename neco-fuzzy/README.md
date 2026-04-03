# neco-fuzzy

Minimal fuzzy score core for commands, paths, and short identifiers.

This crate focuses on pure scoring and ranking. Filesystem indexing, caches, watchers, path normalization, and UI highlighting remain the responsibility of the caller.

## Features

- Case-insensitive subsequence matching by default
- Boundary-aware bonuses for `/`, `_`, `-`, `.`, spaces, and camelCase transitions
- Stable ranking for command search and path search
- Stable API for one-shot matching and ranking
- Prepared API for repeated matching with caller-owned scratch storage
- ASCII fast path for repeated ASCII-heavy queries and candidates
- Candidate archive foundation for cache persistence and warm reload

## Usage

Command search:

```rust
use neco_fuzzy::{score, top_k, Match};

let candidates = ["open-file", "open-folder", "copy-path"];
let mut out: Vec<Match<'_>> = Vec::new();
top_k("of", &candidates, 2, &mut out);

assert_eq!(out[0].candidate, "open-file");
assert!(score("cp", "copy-path").is_some());
```

Path search:

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

Prepared search:

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

Archive foundation:

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

## Ranking

`top_k` ranks matches in this order:

1. Higher score
2. Earlier match start
3. Shorter candidate
4. Earlier input order

The score favors prefix hits, contiguous runs, boundary hits, and matches near the basename of a path.

## API layers

- Stable API: `score`, `score_case_sensitive`, `match_indices`, `top_k`
- Prepared API: `PreparedQuery`, `PreparedCandidate`, `Scratch`, `score_prepared`, `match_indices_prepared`, `top_k_prepared`
- Archive foundation: `OwnedPreparedCandidate`, `PreparedCandidateRef`, `PreparedCandidateHeader`, `candidate_fingerprint`

Use the stable API for simple one-shot calls. Use the prepared API when the same query or candidate set is matched repeatedly.
Use the archive foundation when a caller wants to persist prepared candidates outside the process and reload them into a hot cache.

## Limitations

- File index and score cache are handled by adjacent components outside this crate.
- Returned match indices are byte offsets, not character counts.
- The first release optimizes for short queries and moderate candidate lengths rather than maximum throughput.

## Complexity

- `score`: `O(candidate_len)` for a fixed short query
- `top_k`: `O(n * candidate_len)` scoring work, plus bounded top-`limit` maintenance and a final `O(limit log limit)` sort

## API

| Item | Description |
|------|-------------|
| `Score` | Score summary with value, byte range, and matched count |
| `Match` | Ranked output item for `top_k` |
| `PreparedQuery` | Query prepared for repeated matching |
| `PreparedCandidate` | Candidate prepared for repeated matching |
| `OwnedPreparedCandidate` | Owned prepared candidate for persistence and warm reload |
| `PreparedCandidateRef` | Borrowed prepared candidate view for cache-backed execution |
| `PreparedCandidateHeader` | Versioned archive header for encoded candidates |
| `Scratch` | Reusable working storage for prepared matching |
| `candidate_fingerprint` | Stable fingerprint for cache reuse and invalidation checks |
| `score` | Case-insensitive fuzzy score |
| `score_case_sensitive` | Case-sensitive fuzzy score |
| `score_prepared` | Score with prepared query and candidate |
| `score_prepared_ref` | Score with a borrowed prepared candidate view |
| `score_prepared_owned` | Score with an owned prepared candidate |
| `match_indices` | Write matched byte offsets into a caller-owned buffer |
| `match_indices_prepared` | Write matched byte offsets using prepared inputs |
| `match_indices_prepared_ref` | Write matched byte offsets using a borrowed candidate view |
| `top_k` | Rank the best matches into a caller-owned buffer |
| `top_k_prepared` | Rank prepared candidates with caller-owned scratch |
| `top_k_prepared_refs` | Rank borrowed prepared candidate views with caller-owned scratch |

## License

MIT
