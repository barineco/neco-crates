use crate::boundary::{build_prepared_parts, candidate_fingerprint, CandidateChar};
use crate::encode::{
    checked_archive_text_len, decode_candidate_text, encode_candidate, encoded_len, DecodeError,
    EncodeError, PreparedCandidateHeader, PREPARED_CANDIDATE_ALGORITHM_VERSION,
    PREPARED_CANDIDATE_FORMAT_VERSION,
};

/// 繰り返しマッチング用に前処理した借用 candidate。
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedCandidate<'a> {
    candidate: &'a str,
    chars: Vec<CandidateChar>,
    basename_start_char: usize,
    ascii_bytes: Option<Vec<u8>>,
    ascii_folded: Option<Vec<u8>>,
    fingerprint: u64,
}

impl Eq for PreparedCandidate<'_> {}

impl<'a> PreparedCandidate<'a> {
    /// candidate を前処理して保持します。
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

    /// 元の candidate 文字列を返します。
    pub fn candidate(&self) -> &'a str {
        self.candidate
    }

    /// candidate 文字列に対応する fingerprint を返します。
    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    /// 借用 view に変換します。
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

/// 永続化やキャッシュ再利用向けの所有 candidate。
#[derive(Debug, Clone, PartialEq)]
pub struct OwnedPreparedCandidate {
    text: String,
    chars: Vec<CandidateChar>,
    basename_start_char: usize,
    ascii_bytes: Option<Vec<u8>>,
    ascii_folded: Option<Vec<u8>>,
    fingerprint: u64,
}

impl Eq for OwnedPreparedCandidate {}

impl OwnedPreparedCandidate {
    /// candidate を所有して前処理します。
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

    /// 元の candidate 文字列を返します。
    pub fn candidate(&self) -> &str {
        &self.text
    }

    /// candidate 文字列に対応する fingerprint を返します。
    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    /// 永続化ヘッダを返します。
    pub fn header(&self) -> Result<PreparedCandidateHeader, EncodeError> {
        Ok(PreparedCandidateHeader {
            format_version: PREPARED_CANDIDATE_FORMAT_VERSION,
            algorithm_version: PREPARED_CANDIDATE_ALGORITHM_VERSION,
            fingerprint: self.fingerprint,
            text_len: checked_archive_text_len(self.text.len())?,
        })
    }

    /// エンコード後の総バイト数を返します。
    pub fn encoded_len(&self) -> usize {
        encoded_len(self.text.len())
    }

    /// caller 所有のバッファへエンコードします。
    pub fn encode_into(&self, out: &mut [u8]) -> Result<usize, EncodeError> {
        encode_candidate(&self.text, self.fingerprint, out)
    }

    /// エンコード済み bytes から復元します。
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let (text, header) = decode_candidate_text(bytes)?;
        let owned = Self::new(text.to_string());
        let actual = candidate_fingerprint(owned.candidate());
        if actual != header.fingerprint {
            return Err(DecodeError::FingerprintMismatch {
                expected: header.fingerprint,
                actual,
            });
        }
        Ok(owned)
    }

    /// 借用 view に変換します。
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

/// cache-backed 実行向けの借用 view。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedCandidateRef<'a> {
    pub(crate) candidate: &'a str,
    pub(crate) chars: &'a [CandidateChar],
    pub(crate) basename_start_char: usize,
    pub(crate) ascii_bytes: Option<&'a [u8]>,
    pub(crate) ascii_folded: Option<&'a [u8]>,
    pub(crate) fingerprint: u64,
}

impl Eq for PreparedCandidateRef<'_> {}

impl<'a> PreparedCandidateRef<'a> {
    /// 元の candidate 文字列を返します。
    pub fn candidate(&self) -> &'a str {
        self.candidate
    }

    /// candidate 文字列に対応する fingerprint を返します。
    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
}

#[cfg(test)]
mod tests {
    use super::{OwnedPreparedCandidate, PreparedCandidate};
    use crate::encode::{
        DecodeError, PREPARED_CANDIDATE_ALGORITHM_VERSION, PREPARED_CANDIDATE_FORMAT_VERSION,
    };

    #[test]
    fn prepared_candidate_ref_keeps_text() {
        assert_eq!(
            PreparedCandidate::new("foo_bar").as_ref().candidate(),
            "foo_bar"
        );
    }

    #[test]
    fn owned_prepared_candidate_roundtrip_keeps_text_and_fingerprint() {
        let owned = OwnedPreparedCandidate::new("src/lib/commands.ts");
        let mut bytes = vec![0; owned.encoded_len()];
        let written = owned.encode_into(&mut bytes).expect("encode");
        let decoded = OwnedPreparedCandidate::decode(&bytes[..written]).expect("decode");
        assert_eq!(decoded.candidate(), owned.candidate());
        assert_eq!(decoded.fingerprint(), owned.fingerprint());
    }

    #[test]
    fn decode_rejects_algorithm_version_one_archives() {
        let owned = OwnedPreparedCandidate::new("workspace.ts");
        let mut bytes = vec![0; owned.encoded_len()];
        owned.encode_into(&mut bytes).expect("encode");
        bytes[0..2].copy_from_slice(&PREPARED_CANDIDATE_FORMAT_VERSION.to_le_bytes());
        bytes[2..4].copy_from_slice(&1u16.to_le_bytes());

        let error = OwnedPreparedCandidate::decode(&bytes).expect_err("must reject");
        assert_eq!(
            error,
            DecodeError::UnsupportedAlgorithmVersion {
                expected: PREPARED_CANDIDATE_ALGORITHM_VERSION,
                actual: 1,
            }
        );
    }
}
