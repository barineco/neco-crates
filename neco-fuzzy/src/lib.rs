//! Minimal fuzzy score core for commands, paths, and short identifiers.
//!
//! This crate focuses on pure scoring and ranking only. Filesystem indexing,
//! caches, watchers, and UI rendering stay outside the crate.

mod boundary;
mod candidate;
mod config;
mod corpus;
mod dp;
mod encode;
mod top_k;

use core::cmp::Ordering;

pub use boundary::candidate_fingerprint;
pub use candidate::{OwnedPreparedCandidate, PreparedCandidate, PreparedCandidateRef};
pub use config::{ScoreConfig, VALUE_SCALE};
pub use corpus::CorpusStats;
pub use encode::{
    DecodeError, EncodeError, PreparedCandidateHeader, PREPARED_CANDIDATE_ALGORITHM_VERSION,
    PREPARED_CANDIDATE_FORMAT_VERSION,
};

use dp::{chars_equal_caseless, dp_solve, dp_solve_ascii};
use top_k::collect_top_k;

/// Fuzzy score summary for a single query/candidate pair.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Score {
    /// Higher is better.
    pub value: i64,
    /// Lower is better.
    pub energy: f32,
    /// Confidence in `(0, 1]`. Higher is better.
    pub confidence: f32,
    /// Byte offset of the first matched character.
    pub start: usize,
    /// Exclusive byte offset after the last matched character.
    pub end: usize,
    /// Number of matched query characters.
    pub matched: usize,
}

impl Eq for Score {}

impl Ord for Score {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value
            .cmp(&other.value)
            .then_with(|| other.start.cmp(&self.start))
            .then_with(|| other.end.cmp(&self.end))
            .then_with(|| self.matched.cmp(&other.matched))
    }
}

impl PartialOrd for Score {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Ranked candidate returned from `top_k` variants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Match<'a> {
    pub candidate: &'a str,
    pub score: Score,
    pub index: usize,
}

impl Eq for Match<'_> {}

/// Query prepared for repeated matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedQuery {
    chars: Vec<char>,
    case_sensitive: bool,
    ascii_bytes: Option<Vec<u8>>,
    ascii_folded: Option<Vec<u8>>,
}

impl PreparedQuery {
    /// Prepare a case-insensitive query.
    pub fn new(query: &str) -> Self {
        let ascii_bytes = query.is_ascii().then(|| query.as_bytes().to_vec());
        let ascii_folded = ascii_bytes
            .as_ref()
            .map(|bytes| bytes.iter().map(u8::to_ascii_lowercase).collect());
        Self {
            chars: query.chars().collect(),
            case_sensitive: false,
            ascii_bytes,
            ascii_folded,
        }
    }

    /// Prepare a case-sensitive query.
    pub fn new_case_sensitive(query: &str) -> Self {
        let ascii_bytes = query.is_ascii().then(|| query.as_bytes().to_vec());
        let ascii_folded = ascii_bytes
            .as_ref()
            .map(|bytes| bytes.iter().map(u8::to_ascii_lowercase).collect());
        Self {
            chars: query.chars().collect(),
            case_sensitive: true,
            ascii_bytes,
            ascii_folded,
        }
    }

    /// Return whether the prepared query uses case-sensitive matching.
    pub fn is_case_sensitive(&self) -> bool {
        self.case_sensitive
    }

    /// Return the query length in characters.
    pub fn len(&self) -> usize {
        self.chars.len()
    }

    /// Return whether the query is empty.
    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }
}

/// Reusable working storage for repeated matching.
#[derive(Debug, Default, Clone)]
pub struct Scratch {
    matched: Vec<usize>,
    dp_cost: Vec<f32>,
    dp_prev: Vec<usize>,
    dp_cost_swap: Vec<f32>,
    dp_prev_swap: Vec<usize>,
}

/// Score a candidate with the default case-insensitive matcher.
pub fn score(query: &str, candidate: &str) -> Option<Score> {
    score_with_config(query, candidate, &ScoreConfig::default())
}

/// Score a candidate with case-sensitive matching.
pub fn score_case_sensitive(query: &str, candidate: &str) -> Option<Score> {
    score_case_sensitive_with_config(query, candidate, &ScoreConfig::default())
}

/// Score a candidate with an explicit score config.
pub fn score_with_config(query: &str, candidate: &str, config: &ScoreConfig) -> Option<Score> {
    let query = PreparedQuery::new(query);
    let candidate = PreparedCandidate::new(candidate);
    let mut scratch = Scratch::default();
    score_candidate_view(&query, candidate.as_ref(), config, &mut scratch, None)
}

/// Score a candidate with an explicit score config and corpus statistics.
pub fn score_with_corpus(
    query: &str,
    candidate: &str,
    config: &ScoreConfig,
    stats: &CorpusStats,
) -> Option<Score> {
    let query = PreparedQuery::new(query);
    let candidate = PreparedCandidate::new(candidate);
    let mut scratch = Scratch::default();
    score_candidate_view(
        &query,
        candidate.as_ref(),
        config,
        &mut scratch,
        Some(stats),
    )
}

/// Score a candidate with case-sensitive matching and an explicit score config.
pub fn score_case_sensitive_with_config(
    query: &str,
    candidate: &str,
    config: &ScoreConfig,
) -> Option<Score> {
    let query = PreparedQuery::new_case_sensitive(query);
    let candidate = PreparedCandidate::new(candidate);
    let mut scratch = Scratch::default();
    score_candidate_view(&query, candidate.as_ref(), config, &mut scratch, None)
}

/// Score a prepared candidate with a prepared query.
pub fn score_prepared(
    query: &PreparedQuery,
    candidate: &PreparedCandidate<'_>,
    scratch: &mut Scratch,
) -> Option<Score> {
    score_prepared_with_config(query, candidate, &ScoreConfig::default(), scratch)
}

/// Score a prepared candidate with a prepared query and explicit config.
pub fn score_prepared_with_config(
    query: &PreparedQuery,
    candidate: &PreparedCandidate<'_>,
    config: &ScoreConfig,
    scratch: &mut Scratch,
) -> Option<Score> {
    score_candidate_view(query, candidate.as_ref(), config, scratch, None)
}

/// Score a prepared candidate with a prepared query, config, and corpus stats.
pub fn score_prepared_with_corpus(
    query: &PreparedQuery,
    candidate: &PreparedCandidate<'_>,
    config: &ScoreConfig,
    scratch: &mut Scratch,
    stats: &CorpusStats,
) -> Option<Score> {
    score_candidate_view(query, candidate.as_ref(), config, scratch, Some(stats))
}

/// Score a borrowed prepared candidate view with a prepared query.
pub fn score_prepared_ref(
    query: &PreparedQuery,
    candidate: PreparedCandidateRef<'_>,
    scratch: &mut Scratch,
) -> Option<Score> {
    score_prepared_ref_with_config(query, candidate, &ScoreConfig::default(), scratch)
}

/// Score a borrowed prepared candidate view with explicit config.
pub fn score_prepared_ref_with_config(
    query: &PreparedQuery,
    candidate: PreparedCandidateRef<'_>,
    config: &ScoreConfig,
    scratch: &mut Scratch,
) -> Option<Score> {
    score_candidate_view(query, candidate, config, scratch, None)
}

/// Score a borrowed prepared candidate view with explicit config and corpus stats.
pub fn score_prepared_ref_with_corpus(
    query: &PreparedQuery,
    candidate: PreparedCandidateRef<'_>,
    config: &ScoreConfig,
    scratch: &mut Scratch,
    stats: &CorpusStats,
) -> Option<Score> {
    score_candidate_view(query, candidate, config, scratch, Some(stats))
}

/// Score an owned prepared candidate with a prepared query.
pub fn score_prepared_owned(
    query: &PreparedQuery,
    candidate: &OwnedPreparedCandidate,
    scratch: &mut Scratch,
) -> Option<Score> {
    score_prepared_owned_with_config(query, candidate, &ScoreConfig::default(), scratch)
}

/// Score an owned prepared candidate with explicit config.
pub fn score_prepared_owned_with_config(
    query: &PreparedQuery,
    candidate: &OwnedPreparedCandidate,
    config: &ScoreConfig,
    scratch: &mut Scratch,
) -> Option<Score> {
    score_candidate_view(query, candidate.as_ref(), config, scratch, None)
}

/// Score an owned prepared candidate with explicit config and corpus stats.
pub fn score_prepared_owned_with_corpus(
    query: &PreparedQuery,
    candidate: &OwnedPreparedCandidate,
    config: &ScoreConfig,
    scratch: &mut Scratch,
    stats: &CorpusStats,
) -> Option<Score> {
    score_candidate_view(query, candidate.as_ref(), config, scratch, Some(stats))
}

fn score_candidate_view(
    query: &PreparedQuery,
    candidate: PreparedCandidateRef<'_>,
    config: &ScoreConfig,
    scratch: &mut Scratch,
    stats: Option<&CorpusStats>,
) -> Option<Score> {
    if !fill_match_positions(query, candidate, &mut scratch.matched) {
        return None;
    }

    if query.is_empty() {
        return Some(Score {
            value: 0,
            energy: 0.0,
            confidence: 1.0,
            start: 0,
            end: 0,
            matched: 0,
        });
    }

    let energy = solve_match_positions(query, candidate, config, scratch, stats)?;
    let exact_match = query.chars.len() == candidate.chars.len()
        && query
            .chars
            .iter()
            .zip(candidate.chars.iter())
            .all(|(query_char, candidate_char)| *query_char == candidate_char.ch);
    build_score(
        candidate,
        &scratch.matched,
        query.len(),
        energy,
        config,
        exact_match,
    )
}

/// Write matched character byte offsets into `out`.
pub fn match_indices(query: &str, candidate: &str, out: &mut Vec<usize>) -> bool {
    let query = PreparedQuery::new(query);
    let candidate = PreparedCandidate::new(candidate);
    let mut scratch = Scratch::default();
    match_indices_candidate_view(&query, candidate.as_ref(), out, &mut scratch)
}

/// Write matched character byte offsets into `out` using prepared inputs.
pub fn match_indices_prepared(
    query: &PreparedQuery,
    candidate: &PreparedCandidate<'_>,
    out: &mut Vec<usize>,
    scratch: &mut Scratch,
) -> bool {
    match_indices_candidate_view(query, candidate.as_ref(), out, scratch)
}

/// Write matched character byte offsets into `out` using a borrowed prepared candidate view.
pub fn match_indices_prepared_ref(
    query: &PreparedQuery,
    candidate: PreparedCandidateRef<'_>,
    out: &mut Vec<usize>,
    scratch: &mut Scratch,
) -> bool {
    match_indices_candidate_view(query, candidate, out, scratch)
}

fn match_indices_candidate_view(
    query: &PreparedQuery,
    candidate: PreparedCandidateRef<'_>,
    out: &mut Vec<usize>,
    scratch: &mut Scratch,
) -> bool {
    out.clear();
    if !fill_match_positions(query, candidate, &mut scratch.matched) {
        return false;
    }
    if query.is_empty() {
        return true;
    }
    if solve_match_positions(query, candidate, &ScoreConfig::default(), scratch, None).is_none() {
        return false;
    }
    out.extend(
        scratch
            .matched
            .iter()
            .map(|&index| candidate.chars[index].byte),
    );
    true
}

/// Rank the top `limit` candidates and write them into `out`.
pub fn top_k<'a>(
    query: &str,
    candidates: &'a [&'a str],
    limit: usize,
    out: &mut Vec<Match<'a>>,
) -> usize {
    top_k_with_config(query, candidates, limit, &ScoreConfig::default(), out)
}

/// Rank the top `limit` candidates with an explicit config.
pub fn top_k_with_config<'a>(
    query: &str,
    candidates: &'a [&'a str],
    limit: usize,
    config: &ScoreConfig,
    out: &mut Vec<Match<'a>>,
) -> usize {
    let query = PreparedQuery::new(query);
    let mut scratch = Scratch::default();
    out.clear();
    collect_top_k(
        candidates
            .iter()
            .enumerate()
            .filter_map(|(index, &candidate)| {
                let prepared = PreparedCandidate::new(candidate);
                score_candidate_view(&query, prepared.as_ref(), config, &mut scratch, None).map(
                    |score| Match {
                        candidate,
                        score,
                        index,
                    },
                )
            }),
        limit,
        out,
    );
    out.len()
}

/// Rank the top `limit` candidates with explicit config and corpus stats.
pub fn top_k_with_corpus<'a>(
    query: &str,
    candidates: &'a [&'a str],
    limit: usize,
    config: &ScoreConfig,
    stats: &CorpusStats,
    out: &mut Vec<Match<'a>>,
) -> usize {
    let query = PreparedQuery::new(query);
    let mut scratch = Scratch::default();
    out.clear();
    collect_top_k(
        candidates
            .iter()
            .enumerate()
            .filter_map(|(index, &candidate)| {
                let prepared = PreparedCandidate::new(candidate);
                score_candidate_view(&query, prepared.as_ref(), config, &mut scratch, Some(stats))
                    .map(|score| Match {
                        candidate,
                        score,
                        index,
                    })
            }),
        limit,
        out,
    );
    out.len()
}

/// Rank the top `limit` prepared candidates and write them into `out`.
pub fn top_k_prepared<'a>(
    query: &PreparedQuery,
    candidates: &'a [PreparedCandidate<'a>],
    limit: usize,
    out: &mut Vec<Match<'a>>,
    scratch: &mut Scratch,
) -> usize {
    top_k_prepared_with_config(
        query,
        candidates,
        limit,
        &ScoreConfig::default(),
        out,
        scratch,
    )
}

/// Rank the top `limit` prepared candidates with explicit config.
pub fn top_k_prepared_with_config<'a>(
    query: &PreparedQuery,
    candidates: &'a [PreparedCandidate<'a>],
    limit: usize,
    config: &ScoreConfig,
    out: &mut Vec<Match<'a>>,
    scratch: &mut Scratch,
) -> usize {
    out.clear();
    collect_top_k(
        candidates
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                score_candidate_view(query, candidate.as_ref(), config, scratch, None).map(
                    |score| Match {
                        candidate: candidate.candidate(),
                        score,
                        index,
                    },
                )
            }),
        limit,
        out,
    );
    out.len()
}

/// Rank the top `limit` prepared candidates with explicit config and corpus stats.
pub fn top_k_prepared_with_corpus<'a>(
    query: &PreparedQuery,
    candidates: &'a [PreparedCandidate<'a>],
    limit: usize,
    config: &ScoreConfig,
    out: &mut Vec<Match<'a>>,
    scratch: &mut Scratch,
    stats: &CorpusStats,
) -> usize {
    out.clear();
    collect_top_k(
        candidates
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                score_candidate_view(query, candidate.as_ref(), config, scratch, Some(stats)).map(
                    |score| Match {
                        candidate: candidate.candidate(),
                        score,
                        index,
                    },
                )
            }),
        limit,
        out,
    );
    out.len()
}

/// Rank the top `limit` prepared candidate views and write them into `out`.
pub fn top_k_prepared_refs<'a>(
    query: &PreparedQuery,
    candidates: &[PreparedCandidateRef<'a>],
    limit: usize,
    out: &mut Vec<Match<'a>>,
    scratch: &mut Scratch,
) -> usize {
    top_k_prepared_refs_with_config(
        query,
        candidates,
        limit,
        &ScoreConfig::default(),
        out,
        scratch,
    )
}

/// Rank the top `limit` prepared candidate views with explicit config.
pub fn top_k_prepared_refs_with_config<'a>(
    query: &PreparedQuery,
    candidates: &[PreparedCandidateRef<'a>],
    limit: usize,
    config: &ScoreConfig,
    out: &mut Vec<Match<'a>>,
    scratch: &mut Scratch,
) -> usize {
    out.clear();
    collect_top_k(
        candidates
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                score_candidate_view(query, *candidate, config, scratch, None).map(|score| Match {
                    candidate: candidate.candidate(),
                    score,
                    index,
                })
            }),
        limit,
        out,
    );
    out.len()
}

/// Rank the top `limit` prepared candidate views with explicit config and corpus stats.
pub fn top_k_prepared_refs_with_corpus<'a>(
    query: &PreparedQuery,
    candidates: &[PreparedCandidateRef<'a>],
    limit: usize,
    config: &ScoreConfig,
    out: &mut Vec<Match<'a>>,
    scratch: &mut Scratch,
    stats: &CorpusStats,
) -> usize {
    out.clear();
    collect_top_k(
        candidates
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                score_candidate_view(query, *candidate, config, scratch, Some(stats)).map(|score| {
                    Match {
                        candidate: candidate.candidate(),
                        score,
                        index,
                    }
                })
            }),
        limit,
        out,
    );
    out.len()
}

fn build_score(
    candidate: PreparedCandidateRef<'_>,
    matched: &[usize],
    query_len: usize,
    energy: f32,
    config: &ScoreConfig,
    exact_match: bool,
) -> Option<Score> {
    let first_index = *matched.first()?;
    let last_index = *matched.last()?;
    let first = candidate.chars[first_index];
    let last = candidate.chars[last_index];
    let trailing_chars = candidate.chars.len().saturating_sub(last_index + 1);
    let tail_penalty = if candidate.chars.is_empty() {
        0.0
    } else {
        config.w_tail * (trailing_chars as f32 / candidate.chars.len() as f32)
    };
    let exact_bonus = if exact_match && first_index == 0 && trailing_chars == 0 {
        config.w_exact
    } else {
        0.0
    };
    let total_energy = energy + tail_penalty - exact_bonus;
    Some(Score {
        value: value_from_energy(total_energy),
        energy: total_energy,
        confidence: confidence_from_energy(total_energy, query_len, config.confidence_scale),
        start: first.byte,
        end: last.byte + last.ch.len_utf8(),
        matched: matched.len(),
    })
}

fn value_from_energy(energy: f32) -> i64 {
    (-energy * VALUE_SCALE).round() as i64
}

fn confidence_from_energy(energy: f32, query_len: usize, scale: f32) -> f32 {
    if query_len == 0 {
        return 1.0;
    }
    let denom = (query_len as f32) * scale.max(f32::EPSILON);
    let x = -energy / denom;
    1.0 / (1.0 + (-x).exp())
}

fn solve_match_positions(
    query: &PreparedQuery,
    candidate: PreparedCandidateRef<'_>,
    config: &ScoreConfig,
    scratch: &mut Scratch,
    stats: Option<&CorpusStats>,
) -> Option<f32> {
    let idf = stats.map(|stats| {
        move |ch: char| -> f32 {
            if ch.is_ascii() {
                let byte = u8::try_from(u32::from(ch)).expect("ASCII char fits in u8");
                stats.idf(byte)
            } else {
                0.0
            }
        }
    });
    let idf_ref = idf.as_ref().map(|func| func as &dyn Fn(char) -> f32);

    if query.case_sensitive {
        if let (Some(query_ascii), Some(candidate_ascii)) =
            (&query.ascii_bytes, candidate.ascii_bytes)
        {
            return dp_solve_ascii(
                query_ascii,
                candidate_ascii,
                candidate.chars,
                candidate.basename_start_char,
                config,
                &mut scratch.matched,
                &mut scratch.dp_cost,
                &mut scratch.dp_prev,
                &mut scratch.dp_cost_swap,
                &mut scratch.dp_prev_swap,
                idf_ref,
            );
        }
        return dp::dp_solve_case_sensitive(
            &query.chars,
            candidate.chars,
            candidate.basename_start_char,
            config,
            &mut scratch.matched,
            &mut scratch.dp_cost,
            &mut scratch.dp_prev,
            &mut scratch.dp_cost_swap,
            &mut scratch.dp_prev_swap,
            idf_ref,
        );
    }

    if let (Some(query_ascii), Some(candidate_ascii)) =
        (&query.ascii_folded, candidate.ascii_folded)
    {
        return dp_solve_ascii(
            query_ascii,
            candidate_ascii,
            candidate.chars,
            candidate.basename_start_char,
            config,
            &mut scratch.matched,
            &mut scratch.dp_cost,
            &mut scratch.dp_prev,
            &mut scratch.dp_cost_swap,
            &mut scratch.dp_prev_swap,
            idf_ref,
        );
    }

    dp_solve(
        &query.chars,
        candidate.chars,
        candidate.basename_start_char,
        config,
        &mut scratch.matched,
        &mut scratch.dp_cost,
        &mut scratch.dp_prev,
        &mut scratch.dp_cost_swap,
        &mut scratch.dp_prev_swap,
        idf_ref,
    )
}

fn fill_match_positions(
    query: &PreparedQuery,
    candidate: PreparedCandidateRef<'_>,
    out: &mut Vec<usize>,
) -> bool {
    out.clear();

    if query.is_empty() {
        return true;
    }

    if query.case_sensitive {
        if let (Some(query_ascii), Some(candidate_ascii)) =
            (&query.ascii_bytes, candidate.ascii_bytes)
        {
            return fill_match_positions_ascii(query_ascii, candidate_ascii, out);
        }
    } else if let (Some(query_ascii), Some(candidate_ascii)) =
        (&query.ascii_folded, candidate.ascii_folded)
    {
        return fill_match_positions_ascii(query_ascii, candidate_ascii, out);
    }

    let mut next_candidate = 0usize;
    for &query_char in &query.chars {
        let mut found = None;
        for candidate_index in next_candidate..candidate.chars.len() {
            if chars_equal(
                query_char,
                candidate.chars[candidate_index].ch,
                query.case_sensitive,
            ) {
                found = Some(candidate_index);
                next_candidate = candidate_index + 1;
                break;
            }
        }

        let Some(index) = found else {
            out.clear();
            return false;
        };
        out.push(index);
    }

    true
}

fn fill_match_positions_ascii(query: &[u8], candidate: &[u8], out: &mut Vec<usize>) -> bool {
    let mut next_candidate = 0usize;
    for &query_byte in query {
        let mut found = None;
        for (candidate_index, &candidate_byte) in candidate.iter().enumerate().skip(next_candidate)
        {
            if query_byte == candidate_byte {
                found = Some(candidate_index);
                next_candidate = candidate_index + 1;
                break;
            }
        }

        let Some(index) = found else {
            out.clear();
            return false;
        };
        out.push(index);
    }
    true
}

fn chars_equal(lhs: char, rhs: char, case_sensitive: bool) -> bool {
    if case_sensitive {
        lhs == rhs
    } else if lhs.is_ascii() && rhs.is_ascii() {
        lhs.eq_ignore_ascii_case(&rhs)
    } else {
        chars_equal_caseless(lhs, rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ranked<'a>(query: &str, candidates: &'a [&'a str]) -> Vec<&'a str> {
        let mut out = Vec::new();
        top_k(query, candidates, candidates.len(), &mut out);
        out.into_iter().map(|entry| entry.candidate).collect()
    }

    #[test]
    fn score_accepts_exact_match() {
        let score = score("cmd", "cmd").expect("should match");
        assert_eq!(score.start, 0);
        assert_eq!(score.end, 3);
        assert_eq!(score.matched, 3);
        assert!(score.energy.is_finite());
        assert!(score.confidence.is_finite());
    }

    #[test]
    fn score_accepts_subsequence_match() {
        let score = score("cmd", "command").expect("should match");
        assert_eq!(score.start, 0);
        assert_eq!(score.matched, 3);
    }

    #[test]
    fn score_rejects_non_match() {
        assert!(score("xyz", "command").is_none());
    }

    #[test]
    fn empty_query_matches_everything() {
        let candidates = ["workspace.ts", "src/lib.rs"];
        let mut out = Vec::new();
        let written = top_k("", &candidates, 8, &mut out);
        assert_eq!(written, 2);
        assert_eq!(out[0].candidate, "src/lib.rs");
        assert_eq!(out[1].candidate, "workspace.ts");
    }

    #[test]
    fn prefix_ranks_above_middle_match() {
        let candidates = ["workbench-loader.ts", "workspace.ts"];
        assert_eq!(
            ranked("wo", &candidates),
            vec!["workspace.ts", "workbench-loader.ts"]
        );
    }

    #[test]
    fn contiguous_ranks_above_scattered_match() {
        let candidates = ["foo-bar", "far-out-option"];
        assert_eq!(
            ranked("foo", &candidates),
            vec!["foo-bar", "far-out-option"]
        );
    }

    #[test]
    fn boundary_ranks_above_non_boundary() {
        let candidates = ["foo_bar", "foobar"];
        assert_eq!(ranked("fb", &candidates), vec!["foo_bar", "foobar"]);
    }

    #[test]
    fn shorter_candidate_breaks_ties() {
        let candidates = ["foobar", "foobarbaz"];
        assert_eq!(ranked("foo", &candidates), vec!["foobar", "foobarbaz"]);
    }

    #[test]
    fn exact_match_ranks_above_prefix_extension() {
        let candidates = ["abc_suffix", "abc"];
        assert_eq!(ranked("abc", &candidates), vec!["abc", "abc_suffix"]);
    }

    #[test]
    fn exact_case_variant_ranks_above_prefix_extension() {
        let candidates = ["abc_suffix", "ABC"];
        assert_eq!(ranked("ABC", &candidates), vec!["ABC", "abc_suffix"]);
    }

    #[test]
    fn exact_match_scores_above_prefix_extension() {
        let exact = score("abc", "abc").expect("exact match");
        let extended = score("abc", "abc_suffix").expect("prefix extension");
        assert!(exact.value > extended.value);
        assert!(exact.confidence > extended.confidence);
    }

    #[test]
    fn exact_match_ranks_above_boundary_abbreviations() {
        let candidates = ["a_b_c", "abC", "abc"];
        let ranked = ranked("abc", &candidates);
        assert_eq!(ranked[0], "abc");
        assert!(ranked[1] == "abC" || ranked[1] == "a_b_c");
        assert!(ranked[2] == "abC" || ranked[2] == "a_b_c");
    }

    #[test]
    fn stable_input_order_breaks_full_ties() {
        let candidates = ["alpha", "alpha"];
        let mut out = Vec::new();
        top_k("alp", &candidates, 8, &mut out);
        assert_eq!(out[0].index, 0);
        assert_eq!(out[1].index, 1);
    }

    #[test]
    fn path_cases_favor_basename_and_boundary() {
        let candidates = [
            "src/lib/commands.ts",
            "src/components/StatusCommandBar.vue",
            "workspace.ts",
            "workbench-loader.ts",
        ];
        assert_eq!(
            ranked("cmd", &candidates),
            vec!["src/lib/commands.ts", "src/components/StatusCommandBar.vue"]
        );
        assert_eq!(
            ranked("ws", &candidates),
            vec!["workspace.ts", "workbench-loader.ts"]
        );
    }

    #[test]
    fn case_sensitive_variant_differs_from_default() {
        assert!(score("fb", "FooBar").is_some());
        assert!(score_case_sensitive("fb", "FooBar").is_none());
        assert!(score_case_sensitive("FB", "FooBar").is_some());
    }

    #[test]
    fn match_indices_returns_byte_offsets() {
        let mut out = Vec::new();
        assert!(match_indices("fb", "foo_bar", &mut out));
        assert_eq!(out, vec![0, 4]);
    }

    #[test]
    fn unicode_candidate_is_not_broken() {
        let mut out = Vec::new();
        assert!(match_indices("あい", "あかい", &mut out));
        assert_eq!(out, vec![0, 6]);
    }

    #[test]
    fn prepared_score_matches_stable_api() {
        let query = PreparedQuery::new("cmd");
        let candidate = PreparedCandidate::new("src/lib/commands.ts");
        let mut scratch = Scratch::default();
        assert_eq!(
            score("cmd", "src/lib/commands.ts"),
            score_prepared(&query, &candidate, &mut scratch)
        );
    }

    #[test]
    fn prepared_top_k_matches_stable_api() {
        let candidates = [
            PreparedCandidate::new("src/lib/commands.ts"),
            PreparedCandidate::new("src/components/StatusCommandBar.vue"),
            PreparedCandidate::new("workspace.ts"),
        ];
        let query = PreparedQuery::new("cmd");
        let mut stable = Vec::new();
        let mut prepared = Vec::new();
        let mut scratch = Scratch::default();

        top_k(
            "cmd",
            &[
                "src/lib/commands.ts",
                "src/components/StatusCommandBar.vue",
                "workspace.ts",
            ],
            3,
            &mut stable,
        );
        top_k_prepared(&query, &candidates, 3, &mut prepared, &mut scratch);

        assert_eq!(stable, prepared);
    }

    #[test]
    fn scratch_reuse_keeps_results_stable() {
        let query = PreparedQuery::new("fb");
        let candidate = PreparedCandidate::new("foo_bar");
        let mut scratch = Scratch::default();
        let mut first_indices = Vec::new();
        let mut second_indices = Vec::new();

        let first = score_prepared(&query, &candidate, &mut scratch);
        assert!(match_indices_prepared(
            &query,
            &candidate,
            &mut first_indices,
            &mut scratch
        ));
        let second = score_prepared(&query, &candidate, &mut scratch);
        assert!(match_indices_prepared(
            &query,
            &candidate,
            &mut second_indices,
            &mut scratch
        ));

        assert_eq!(first, second);
        assert_eq!(first_indices, second_indices);
    }

    #[test]
    fn prepared_case_sensitive_ascii_matches_stable_api() {
        let query = PreparedQuery::new_case_sensitive("FB");
        let candidate = PreparedCandidate::new("FooBar");
        let mut scratch = Scratch::default();
        assert_eq!(
            score_case_sensitive("FB", "FooBar"),
            score_prepared(&query, &candidate, &mut scratch)
        );
    }

    #[test]
    fn owned_prepared_roundtrip_keeps_score() {
        let owned = OwnedPreparedCandidate::new("src/lib/commands.ts");
        let mut bytes = vec![0; owned.encoded_len()];
        let written = owned.encode_into(&mut bytes).expect("encode");
        let decoded = OwnedPreparedCandidate::decode(&bytes[..written]).expect("decode");
        let query = PreparedQuery::new("cmd");
        let mut scratch = Scratch::default();

        assert_eq!(
            owned.header().expect("owned header"),
            decoded.header().expect("decoded header")
        );
        assert_eq!(
            score_prepared_owned(&query, &owned, &mut scratch),
            score_prepared_owned(&query, &decoded, &mut scratch)
        );
    }

    #[test]
    fn prepared_refs_rank_like_stable_api() {
        let query = PreparedQuery::new("cmd");
        let owned = [
            OwnedPreparedCandidate::new("src/lib/commands.ts"),
            OwnedPreparedCandidate::new("src/components/StatusCommandBar.vue"),
            OwnedPreparedCandidate::new("workspace.ts"),
        ];
        let refs: Vec<PreparedCandidateRef<'_>> =
            owned.iter().map(OwnedPreparedCandidate::as_ref).collect();
        let mut out = Vec::new();
        let mut scratch = Scratch::default();
        top_k_prepared_refs(&query, &refs, 3, &mut out, &mut scratch);
        assert_eq!(
            out.into_iter()
                .map(|entry| entry.candidate)
                .collect::<Vec<_>>(),
            vec!["src/lib/commands.ts", "src/components/StatusCommandBar.vue"]
        );
    }

    #[test]
    fn default_config_matches_explicit_config() {
        let config = ScoreConfig::default();
        assert_eq!(
            score("cmd", "command"),
            score_with_config("cmd", "command", &config)
        );
    }

    #[test]
    fn score_with_corpus_changes_energy_when_idf_is_enabled() {
        let stats = CorpusStats::from_candidates(&["foo_bar", "foobar", "quxbuzz"]);
        let config = ScoreConfig {
            w_idf: 2.0,
            ..ScoreConfig::default()
        };

        let plain = score_with_config("fb", "foo_bar", &config).expect("plain score");
        let weighted = score_with_corpus("fb", "foo_bar", &config, &stats).expect("weighted score");

        assert_ne!(plain.energy, weighted.energy);
        assert_ne!(plain.value, weighted.value);
    }

    #[test]
    fn confidence_stays_in_open_interval_for_non_empty_query() {
        let score = score("cmd", "src/lib/commands.ts").expect("must match");
        assert!(score.confidence > 0.0);
        assert!(score.confidence < 1.0);
    }
}
