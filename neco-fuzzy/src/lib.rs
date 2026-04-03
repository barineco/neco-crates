//! Minimal fuzzy score core for commands, paths, and short identifiers.
//!
//! This crate focuses on pure scoring and ranking only. Filesystem indexing,
//! caches, watchers, and UI rendering stay outside the crate.

use core::cmp::Ordering;
use core::fmt;

const PREPARED_CANDIDATE_FORMAT_VERSION: u16 = 1;
const PREPARED_CANDIDATE_ALGORITHM_VERSION: u16 = 1;
const PREPARED_CANDIDATE_HEADER_LEN: usize = 16;
const FLAG_BOUNDARY: u8 = 0b0000_0001;

/// Fuzzy score summary for a single query/candidate pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Score {
    /// Higher is better.
    pub value: i64,
    /// Byte offset of the first matched character.
    pub start: usize,
    /// Exclusive byte offset after the last matched character.
    pub end: usize,
    /// Number of matched query characters.
    pub matched: usize,
}

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match<'a> {
    pub candidate: &'a str,
    pub score: Score,
    pub index: usize,
}

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

/// Candidate prepared for repeated matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedCandidate<'a> {
    candidate: &'a str,
    chars: Vec<CandidateChar>,
    basename_start_char: usize,
    ascii_bytes: Option<Vec<u8>>,
    ascii_folded: Option<Vec<u8>>,
    fingerprint: u64,
}

impl<'a> PreparedCandidate<'a> {
    /// Prepare a candidate for repeated matching.
    pub fn new(candidate: &'a str) -> Self {
        let (chars, basename_start_char, ascii_bytes, ascii_folded, fingerprint) =
            build_prepared_parts(candidate);
        Self {
            candidate,
            chars,
            basename_start_char,
            ascii_bytes,
            ascii_folded,
            fingerprint,
        }
    }

    /// Return the original candidate string.
    pub fn candidate(&self) -> &'a str {
        self.candidate
    }

    /// Return a cache-reuse fingerprint for the candidate text.
    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    /// Return a borrowed view over this prepared candidate.
    pub fn as_ref(&self) -> PreparedCandidateRef<'_> {
        PreparedCandidateRef {
            candidate: self.candidate,
            chars: &self.chars,
            basename_start_char: self.basename_start_char,
            ascii_bytes: self.ascii_bytes.as_deref(),
            ascii_folded: self.ascii_folded.as_deref(),
            fingerprint: self.fingerprint,
        }
    }
}

/// Owned prepared candidate for persistence and cache reuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedPreparedCandidate {
    text: String,
    chars: Vec<CandidateChar>,
    basename_start_char: usize,
    ascii_bytes: Option<Vec<u8>>,
    ascii_folded: Option<Vec<u8>>,
    fingerprint: u64,
}

impl OwnedPreparedCandidate {
    /// Prepare and own a candidate for persistence and repeated matching.
    pub fn new(candidate: impl Into<String>) -> Self {
        let text = candidate.into();
        let (chars, basename_start_char, ascii_bytes, ascii_folded, fingerprint) =
            build_prepared_parts(text.as_str());
        Self {
            text,
            chars,
            basename_start_char,
            ascii_bytes,
            ascii_folded,
            fingerprint,
        }
    }

    /// Return the original candidate text.
    pub fn candidate(&self) -> &str {
        &self.text
    }

    /// Return a cache-reuse fingerprint for the candidate text.
    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    /// Return a versioned archive header for persistence.
    pub fn header(&self) -> Result<PreparedCandidateHeader, EncodeError> {
        Ok(PreparedCandidateHeader {
            format_version: PREPARED_CANDIDATE_FORMAT_VERSION,
            algorithm_version: PREPARED_CANDIDATE_ALGORITHM_VERSION,
            fingerprint: self.fingerprint,
            text_len: checked_archive_text_len(self.text.len())?,
        })
    }

    /// Return the encoded archive length in bytes.
    pub fn encoded_len(&self) -> usize {
        PREPARED_CANDIDATE_HEADER_LEN + self.text.len()
    }

    /// Encode the archive into a caller-owned buffer.
    pub fn encode_into(&self, out: &mut [u8]) -> Result<usize, EncodeError> {
        let required = self.encoded_len();
        if out.len() < required {
            return Err(EncodeError::BufferTooSmall { required });
        }

        let header = self.header()?;
        out[0..2].copy_from_slice(&header.format_version.to_le_bytes());
        out[2..4].copy_from_slice(&header.algorithm_version.to_le_bytes());
        out[4..12].copy_from_slice(&header.fingerprint.to_le_bytes());
        out[12..16].copy_from_slice(&header.text_len.to_le_bytes());
        out[16..required].copy_from_slice(self.text.as_bytes());
        Ok(required)
    }

    /// Decode an owned prepared candidate from an archive buffer.
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let header = decode_header(bytes)?;
        if header.format_version != PREPARED_CANDIDATE_FORMAT_VERSION {
            return Err(DecodeError::UnsupportedFormatVersion {
                expected: PREPARED_CANDIDATE_FORMAT_VERSION,
                actual: header.format_version,
            });
        }
        if header.algorithm_version != PREPARED_CANDIDATE_ALGORITHM_VERSION {
            return Err(DecodeError::UnsupportedAlgorithmVersion {
                expected: PREPARED_CANDIDATE_ALGORITHM_VERSION,
                actual: header.algorithm_version,
            });
        }

        let text_len =
            usize::try_from(header.text_len).map_err(|_| DecodeError::PlatformLengthOverflow {
                text_len: header.text_len,
            })?;
        let required = PREPARED_CANDIDATE_HEADER_LEN + text_len;
        if bytes.len() < required {
            return Err(DecodeError::Truncated {
                required,
                actual: bytes.len(),
            });
        }

        let text_bytes = &bytes[PREPARED_CANDIDATE_HEADER_LEN..required];
        let text = core::str::from_utf8(text_bytes)
            .map_err(|_| DecodeError::InvalidUtf8)?
            .to_string();
        let owned = Self::new(text);
        if owned.fingerprint != header.fingerprint {
            return Err(DecodeError::FingerprintMismatch {
                expected: header.fingerprint,
                actual: owned.fingerprint,
            });
        }
        Ok(owned)
    }

    /// Return a borrowed prepared candidate view.
    pub fn as_ref(&self) -> PreparedCandidateRef<'_> {
        PreparedCandidateRef {
            candidate: self.text.as_str(),
            chars: &self.chars,
            basename_start_char: self.basename_start_char,
            ascii_bytes: self.ascii_bytes.as_deref(),
            ascii_folded: self.ascii_folded.as_deref(),
            fingerprint: self.fingerprint,
        }
    }
}

/// Borrowed prepared candidate view for cache-backed execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedCandidateRef<'a> {
    candidate: &'a str,
    chars: &'a [CandidateChar],
    basename_start_char: usize,
    ascii_bytes: Option<&'a [u8]>,
    ascii_folded: Option<&'a [u8]>,
    fingerprint: u64,
}

impl<'a> PreparedCandidateRef<'a> {
    /// Return the original candidate string.
    pub fn candidate(&self) -> &'a str {
        self.candidate
    }

    /// Return a cache-reuse fingerprint for the candidate text.
    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
}

/// Versioned archive header for encoded prepared candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedCandidateHeader {
    pub format_version: u16,
    pub algorithm_version: u16,
    pub fingerprint: u64,
    pub text_len: u32,
}

/// Encoding error for prepared candidate archives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    BufferTooSmall { required: usize },
    TextTooLong { len: usize },
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooSmall { required } => {
                write!(
                    f,
                    "buffer too small for prepared candidate archive: need {required} bytes"
                )
            }
            Self::TextTooLong { len } => {
                write!(
                    f,
                    "prepared candidate text length {len} exceeds u32 archive header"
                )
            }
        }
    }
}

impl std::error::Error for EncodeError {}

/// Decode error for prepared candidate archives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    Truncated { required: usize, actual: usize },
    UnsupportedFormatVersion { expected: u16, actual: u16 },
    UnsupportedAlgorithmVersion { expected: u16, actual: u16 },
    FingerprintMismatch { expected: u64, actual: u64 },
    PlatformLengthOverflow { text_len: u32 },
    InvalidUtf8,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { required, actual } => {
                write!(
                    f,
                    "prepared candidate archive truncated: need {required} bytes, got {actual}"
                )
            }
            Self::UnsupportedFormatVersion { expected, actual } => {
                write!(
                    f,
                    "unsupported prepared candidate format version: expected {expected}, got {actual}"
                )
            }
            Self::UnsupportedAlgorithmVersion { expected, actual } => {
                write!(
                    f,
                    "unsupported prepared candidate algorithm version: expected {expected}, got {actual}"
                )
            }
            Self::FingerprintMismatch { expected, actual } => {
                write!(
                    f,
                    "prepared candidate fingerprint mismatch: expected {expected}, got {actual}"
                )
            }
            Self::PlatformLengthOverflow { text_len } => {
                write!(
                    f,
                    "prepared candidate archive length {text_len} does not fit on this platform"
                )
            }
            Self::InvalidUtf8 => write!(f, "prepared candidate archive contains invalid utf-8"),
        }
    }
}

impl std::error::Error for DecodeError {}

fn checked_archive_text_len(len: usize) -> Result<u32, EncodeError> {
    u32::try_from(len).map_err(|_| EncodeError::TextTooLong { len })
}

/// Return a stable fingerprint for candidate cache reuse.
pub fn candidate_fingerprint(candidate: &str) -> u64 {
    let mut state = 0xcbf2_9ce4_8422_2325u64;
    for &byte in candidate.as_bytes() {
        state ^= u64::from(byte);
        state = state.wrapping_mul(0x0000_0100_0000_01b3);
    }
    state
}

/// Reusable working storage for repeated matching.
#[derive(Debug, Default, Clone)]
pub struct Scratch {
    matched: Vec<usize>,
}

/// Score a candidate with the default case-insensitive matcher.
pub fn score(query: &str, candidate: &str) -> Option<Score> {
    let query = PreparedQuery::new(query);
    let candidate = PreparedCandidate::new(candidate);
    let mut scratch = Scratch::default();
    score_candidate_view(&query, candidate.as_ref(), &mut scratch)
}

/// Score a candidate with case-sensitive matching.
pub fn score_case_sensitive(query: &str, candidate: &str) -> Option<Score> {
    let query = PreparedQuery::new_case_sensitive(query);
    let candidate = PreparedCandidate::new(candidate);
    let mut scratch = Scratch::default();
    score_candidate_view(&query, candidate.as_ref(), &mut scratch)
}

/// Score a prepared candidate with a prepared query.
pub fn score_prepared(
    query: &PreparedQuery,
    candidate: &PreparedCandidate<'_>,
    scratch: &mut Scratch,
) -> Option<Score> {
    score_candidate_view(query, candidate.as_ref(), scratch)
}

/// Score a borrowed prepared candidate view with a prepared query.
pub fn score_prepared_ref(
    query: &PreparedQuery,
    candidate: PreparedCandidateRef<'_>,
    scratch: &mut Scratch,
) -> Option<Score> {
    score_candidate_view(query, candidate, scratch)
}

/// Score an owned prepared candidate with a prepared query.
pub fn score_prepared_owned(
    query: &PreparedQuery,
    candidate: &OwnedPreparedCandidate,
    scratch: &mut Scratch,
) -> Option<Score> {
    score_candidate_view(query, candidate.as_ref(), scratch)
}

fn score_candidate_view(
    query: &PreparedQuery,
    candidate: PreparedCandidateRef<'_>,
    scratch: &mut Scratch,
) -> Option<Score> {
    if !fill_match_positions(query, candidate, scratch) {
        return None;
    }

    if scratch.matched.is_empty() {
        return Some(Score {
            value: 0,
            start: 0,
            end: 0,
            matched: 0,
        });
    }

    let first = candidate.chars[scratch.matched[0]];
    let last = candidate.chars[*scratch.matched.last().expect("non-empty match positions")];
    let first_index = scratch.matched[0];
    let last_index = *scratch.matched.last().expect("non-empty match positions");
    let span_chars = last_index - first_index + 1;

    let mut value = 0i64;
    value += i64::try_from(scratch.matched.len()).expect("matched length fits in i64") * 100;
    value -= i64::try_from(span_chars).expect("span length fits in i64") * 2;
    value -= i64::try_from(first_index).expect("start index fits in i64") * 3;

    if first_index == 0 {
        value += 120;
    }
    if first_index >= candidate.basename_start_char {
        value += 90;
    }

    for (i, &current_index) in scratch.matched.iter().enumerate() {
        if candidate.chars[current_index].flags & FLAG_BOUNDARY != 0 {
            value += 45;
        }
        if i > 0 && current_index == scratch.matched[i - 1] + 1 {
            value += 70;
        }
    }

    Some(Score {
        value,
        start: first.byte,
        end: last.byte + last.ch.len_utf8(),
        matched: scratch.matched.len(),
    })
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
    if !fill_match_positions(query, candidate, scratch) {
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
    let query = PreparedQuery::new(query);
    let mut scratch = Scratch::default();
    out.clear();

    if limit == 0 {
        return 0;
    }

    for (index, &candidate) in candidates.iter().enumerate() {
        let prepared = PreparedCandidate::new(candidate);
        if let Some(score) = score_candidate_view(&query, prepared.as_ref(), &mut scratch) {
            push_top_k(
                out,
                limit,
                Match {
                    candidate,
                    score,
                    index,
                },
            );
        }
    }

    out.sort_by(compare_match);
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
    let refs: Vec<PreparedCandidateRef<'a>> =
        candidates.iter().map(PreparedCandidate::as_ref).collect();
    top_k_prepared_refs(query, &refs, limit, out, scratch)
}

/// Rank the top `limit` prepared candidate views and write them into `out`.
pub fn top_k_prepared_refs<'a>(
    query: &PreparedQuery,
    candidates: &[PreparedCandidateRef<'a>],
    limit: usize,
    out: &mut Vec<Match<'a>>,
    scratch: &mut Scratch,
) -> usize {
    out.clear();
    if limit == 0 {
        return 0;
    }

    for (index, candidate) in candidates.iter().enumerate() {
        if let Some(score) = score_candidate_view(query, *candidate, scratch) {
            push_top_k(
                out,
                limit,
                Match {
                    candidate: candidate.candidate(),
                    score,
                    index,
                },
            );
        }
    }

    out.sort_by(compare_match);
    out.len()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CandidateChar {
    byte: usize,
    ch: char,
    flags: u8,
}

type PreparedParts = (
    Vec<CandidateChar>,
    usize,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
    u64,
);

fn build_prepared_parts(candidate: &str) -> PreparedParts {
    let mut chars = Vec::with_capacity(candidate.chars().count());
    let mut previous = None;
    for (char_index, (byte, ch)) in candidate.char_indices().enumerate() {
        let flags = if is_boundary_char(previous, ch, char_index) {
            FLAG_BOUNDARY
        } else {
            0
        };
        chars.push(CandidateChar { byte, ch, flags });
        previous = Some(ch);
    }

    let basename_start_byte = candidate.rfind('/').map_or(0, |index| index + 1);
    let basename_start_char = chars
        .iter()
        .position(|slot| slot.byte >= basename_start_byte)
        .unwrap_or(chars.len());
    let ascii_bytes = candidate.is_ascii().then(|| candidate.as_bytes().to_vec());
    let ascii_folded = ascii_bytes
        .as_ref()
        .map(|bytes| bytes.iter().map(u8::to_ascii_lowercase).collect());

    (
        chars,
        basename_start_char,
        ascii_bytes,
        ascii_folded,
        candidate_fingerprint(candidate),
    )
}

fn fill_match_positions(
    query: &PreparedQuery,
    candidate: PreparedCandidateRef<'_>,
    scratch: &mut Scratch,
) -> bool {
    scratch.matched.clear();

    if query.is_empty() {
        return true;
    }

    if query.case_sensitive {
        if let (Some(query_ascii), Some(candidate_ascii)) =
            (&query.ascii_bytes, candidate.ascii_bytes)
        {
            return fill_match_positions_ascii(query_ascii, candidate_ascii, scratch);
        }
    } else if let (Some(query_ascii), Some(candidate_ascii)) =
        (&query.ascii_folded, candidate.ascii_folded)
    {
        return fill_match_positions_ascii(query_ascii, candidate_ascii, scratch);
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
            scratch.matched.clear();
            return false;
        };
        scratch.matched.push(index);
    }

    true
}

fn fill_match_positions_ascii(query: &[u8], candidate: &[u8], scratch: &mut Scratch) -> bool {
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
            scratch.matched.clear();
            return false;
        };
        scratch.matched.push(index);
    }
    true
}

fn chars_equal(lhs: char, rhs: char, case_sensitive: bool) -> bool {
    if case_sensitive {
        lhs == rhs
    } else if lhs.is_ascii() && rhs.is_ascii() {
        lhs.eq_ignore_ascii_case(&rhs)
    } else {
        lhs.to_lowercase().eq(rhs.to_lowercase())
    }
}

fn is_boundary_char(previous: Option<char>, current: char, char_index: usize) -> bool {
    if char_index == 0 {
        return true;
    }
    let Some(prev) = previous else {
        return false;
    };
    matches!(prev, '/' | '_' | '-' | '.' | ' ') || (prev.is_lowercase() && current.is_uppercase())
}

fn push_top_k<'a>(out: &mut Vec<Match<'a>>, limit: usize, item: Match<'a>) {
    if out.len() < limit {
        out.push(item);
        return;
    }

    let mut worst_index = 0usize;
    for index in 1..out.len() {
        if compare_match(&out[index], &out[worst_index]) == Ordering::Greater {
            worst_index = index;
        }
    }

    if compare_match(&item, &out[worst_index]) == Ordering::Less {
        out[worst_index] = item;
    }
}

fn compare_match(lhs: &Match<'_>, rhs: &Match<'_>) -> Ordering {
    rhs.score
        .cmp(&lhs.score)
        .then_with(|| lhs.candidate.len().cmp(&rhs.candidate.len()))
        .then_with(|| lhs.index.cmp(&rhs.index))
}

fn decode_header(bytes: &[u8]) -> Result<PreparedCandidateHeader, DecodeError> {
    if bytes.len() < PREPARED_CANDIDATE_HEADER_LEN {
        return Err(DecodeError::Truncated {
            required: PREPARED_CANDIDATE_HEADER_LEN,
            actual: bytes.len(),
        });
    }

    let format_version = u16::from_le_bytes([bytes[0], bytes[1]]);
    let algorithm_version = u16::from_le_bytes([bytes[2], bytes[3]]);
    let fingerprint = u64::from_le_bytes([
        bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9], bytes[10], bytes[11],
    ]);
    let text_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    Ok(PreparedCandidateHeader {
        format_version,
        algorithm_version,
        fingerprint,
        text_len,
    })
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

        let first = score_prepared(&query, &candidate, &mut scratch);
        let second = score_prepared(&query, &candidate, &mut scratch);

        assert_eq!(first, second);
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
    fn decode_rejects_version_mismatch() {
        let owned = OwnedPreparedCandidate::new("workspace.ts");
        let mut bytes = vec![0; owned.encoded_len()];
        owned.encode_into(&mut bytes).expect("encode");
        bytes[0..2].copy_from_slice(&99u16.to_le_bytes());
        let error = OwnedPreparedCandidate::decode(&bytes).expect_err("must reject");
        assert_eq!(
            error,
            DecodeError::UnsupportedFormatVersion {
                expected: PREPARED_CANDIDATE_FORMAT_VERSION,
                actual: 99,
            }
        );
    }

    #[test]
    fn header_rejects_candidate_len_above_u32() {
        let error = checked_archive_text_len(usize::MAX).expect_err("usize::MAX exceeds u32");
        assert_eq!(error, EncodeError::TextTooLong { len: usize::MAX });
    }

    #[test]
    fn fingerprint_matches_text_function() {
        let prepared = PreparedCandidate::new("workspace.ts");
        let owned = OwnedPreparedCandidate::new("workspace.ts");
        let expected = candidate_fingerprint("workspace.ts");
        assert_eq!(prepared.fingerprint(), expected);
        assert_eq!(owned.fingerprint(), expected);
        assert_eq!(prepared.as_ref().fingerprint(), expected);
    }
}
