#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CandidateChar {
    pub(crate) byte: usize,
    pub(crate) ch: char,
    pub(crate) boundary: f32,
}

pub(crate) fn compute_boundary(previous: Option<char>, current: char, char_index: usize) -> f32 {
    if char_index == 0 {
        return 1.0;
    }

    let Some(prev) = previous else {
        return 0.0;
    };

    if prev == '/' {
        return 1.0;
    }

    if matches!(prev, '_' | '-' | '.' | ' ') {
        return 0.76;
    }

    if prev.is_lowercase() && current.is_uppercase() {
        return 0.8;
    }

    if (prev.is_ascii_digit() && current.is_ascii_alphabetic())
        || (prev.is_ascii_alphabetic() && current.is_ascii_digit())
    {
        return 0.6;
    }

    0.0
}

pub(crate) type PreparedParts = (
    Vec<CandidateChar>,
    usize,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
    u64,
);

pub(crate) fn build_prepared_parts(candidate: &str) -> PreparedParts {
    let mut chars = Vec::with_capacity(candidate.chars().count());
    let mut previous = None;
    for (char_index, (byte, ch)) in candidate.char_indices().enumerate() {
        chars.push(CandidateChar {
            byte,
            ch,
            boundary: compute_boundary(previous, ch, char_index),
        });
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

pub fn candidate_fingerprint(candidate: &str) -> u64 {
    let mut state = 0xcbf2_9ce4_8422_2325u64;
    for &byte in candidate.as_bytes() {
        state ^= u64::from(byte);
        state = state.wrapping_mul(0x0000_0100_0000_01b3);
    }
    state
}

#[cfg(test)]
mod tests {
    use super::{build_prepared_parts, candidate_fingerprint, compute_boundary};

    #[test]
    fn computes_expected_boundary_weights() {
        assert_eq!(compute_boundary(Some('a'), 'B', 1), 0.8);
        assert_eq!(compute_boundary(Some('1'), 'a', 1), 0.6);
        assert_eq!(compute_boundary(None, 'x', 0), 1.0);
        assert_eq!(compute_boundary(Some('/'), 'x', 5), 1.0);
        assert_eq!(compute_boundary(Some('a'), 'b', 1), 0.0);
    }

    #[test]
    fn prepared_parts_keep_basename_start_and_ascii_views() {
        let (chars, basename_start_char, ascii_bytes, ascii_folded, fingerprint) =
            build_prepared_parts("src/FooBar-v2.rs");

        assert_eq!(basename_start_char, 4);
        assert_eq!(ascii_bytes.as_deref(), Some(b"src/FooBar-v2.rs".as_slice()));
        assert_eq!(
            ascii_folded.as_deref(),
            Some(b"src/foobar-v2.rs".as_slice())
        );
        assert_eq!(fingerprint, 0xd662_cea5_1829_1d93);

        let boundaries: Vec<(usize, char, f32)> = chars
            .into_iter()
            .map(|slot| (slot.byte, slot.ch, slot.boundary))
            .collect();
        assert_eq!(
            boundaries,
            vec![
                (0, 's', 1.0),
                (1, 'r', 0.0),
                (2, 'c', 0.0),
                (3, '/', 0.0),
                (4, 'F', 1.0),
                (5, 'o', 0.0),
                (6, 'o', 0.0),
                (7, 'B', 0.8),
                (8, 'a', 0.0),
                (9, 'r', 0.0),
                (10, '-', 0.0),
                (11, 'v', 0.76),
                (12, '2', 0.6),
                (13, '.', 0.0),
                (14, 'r', 0.76),
                (15, 's', 0.0),
            ]
        );
    }

    #[test]
    fn fingerprint_matches_fnv1a64() {
        assert_eq!(candidate_fingerprint(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(candidate_fingerprint("abc"), 0xe71f_a219_0541_574b);
        assert_eq!(
            candidate_fingerprint("src/FooBar-v2.rs"),
            0xd662_cea5_1829_1d93
        );
    }
}
