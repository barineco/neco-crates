use crate::{KeyBundle, SecpError};

#[cfg(all(feature = "batch", feature = "nip19"))]
pub fn mine_vanity_npub(prefix: &str, max_attempts: u64) -> Result<KeyBundle, SecpError> {
    for _ in 0..max_attempts {
        let bundle = KeyBundle::generate()?;
        if count_npub_prefix_matches(&bundle.xonly_public_key().to_bytes(), prefix)? == prefix.len()
        {
            return Ok(bundle);
        }
    }
    Err(SecpError::ExhaustedAttempts)
}

#[cfg(all(feature = "batch", feature = "nip19"))]
#[derive(Debug, Clone)]
pub struct VanityCandidate {
    bundle: KeyBundle,
    matched_len: usize,
}

#[cfg(all(feature = "batch", feature = "nip19"))]
impl VanityCandidate {
    pub fn bundle(&self) -> &KeyBundle {
        &self.bundle
    }

    pub fn matched_len(&self) -> usize {
        self.matched_len
    }
}

#[cfg(all(feature = "batch", feature = "nip19"))]
pub fn mine_vanity_npub_candidates(
    prefix: &str,
    max_attempts: u64,
    top_k: usize,
) -> Result<Vec<VanityCandidate>, SecpError> {
    if top_k == 0 {
        return Ok(vec![]);
    }
    let mut candidates: Vec<VanityCandidate> = Vec::new();
    let mut min_matched = 0usize;

    for _ in 0..max_attempts {
        let bundle = KeyBundle::generate()?;
        let matched = count_npub_prefix_matches(&bundle.xonly_public_key().to_bytes(), prefix)?;

        if matched == 0 {
            continue;
        }

        if matched == prefix.len() || matched > min_matched || candidates.len() < top_k {
            candidates.push(VanityCandidate {
                bundle,
                matched_len: matched,
            });
        }

        if candidates.len() > top_k {
            candidates.sort_by(|a, b| b.matched_len.cmp(&a.matched_len));
            candidates.truncate(top_k);
            min_matched = candidates.last().map_or(0, |c| c.matched_len);
        }
    }

    candidates.sort_by(|a, b| b.matched_len.cmp(&a.matched_len));
    candidates.truncate(top_k);
    Ok(candidates)
}

#[cfg(all(feature = "batch", feature = "nip19"))]
const BECH32_CHARSET: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";

#[cfg(all(feature = "batch", feature = "nip19"))]
pub(crate) fn bech32_value(byte: u8) -> Result<u8, SecpError> {
    match byte {
        b'q' => Ok(0),
        b'p' => Ok(1),
        b'z' => Ok(2),
        b'r' => Ok(3),
        b'y' => Ok(4),
        b'9' => Ok(5),
        b'x' => Ok(6),
        b'8' => Ok(7),
        b'g' => Ok(8),
        b'f' => Ok(9),
        b'2' => Ok(10),
        b't' => Ok(11),
        b'v' => Ok(12),
        b'd' => Ok(13),
        b'w' => Ok(14),
        b'0' => Ok(15),
        b's' => Ok(16),
        b'3' => Ok(17),
        b'j' => Ok(18),
        b'n' => Ok(19),
        b'5' => Ok(20),
        b'4' => Ok(21),
        b'k' => Ok(22),
        b'h' => Ok(23),
        b'c' => Ok(24),
        b'e' => Ok(25),
        b'6' => Ok(26),
        b'm' => Ok(27),
        b'u' => Ok(28),
        b'a' => Ok(29),
        b'7' => Ok(30),
        b'l' => Ok(31),
        _ => Err(SecpError::InvalidNip19("invalid npub vanity prefix")),
    }
}

#[cfg(all(feature = "batch", feature = "nip19"))]
pub(crate) fn count_npub_prefix_matches(
    xonly_bytes: &[u8; 32],
    prefix: &str,
) -> Result<usize, SecpError> {
    let prefix_bytes = prefix.as_bytes();
    if prefix_bytes.is_empty() {
        return Ok(0);
    }

    for &byte in prefix_bytes {
        bech32_value(byte)?;
    }

    let mut matched = 0usize;
    let mut acc = 0u16;
    let mut bits = 0u8;

    for &byte in xonly_bytes {
        acc = (acc << 8) | u16::from(byte);
        bits += 8;

        while bits >= 5 && matched < prefix_bytes.len() {
            bits -= 5;
            let value = ((acc >> bits) & 0x1f) as usize;
            if BECH32_CHARSET[value] != prefix_bytes[matched] {
                return Ok(matched);
            }
            matched += 1;
        }

        if matched == prefix_bytes.len() {
            return Ok(matched);
        }
    }

    if bits > 0 && matched < prefix_bytes.len() {
        let value = ((acc << (5 - bits)) & 0x1f) as usize;
        if BECH32_CHARSET[value] == prefix_bytes[matched] {
            matched += 1;
        }
    }

    Ok(matched)
}

#[cfg(feature = "batch")]
pub fn mine_pow(difficulty: u8, max_attempts: u64) -> Result<KeyBundle, SecpError> {
    for _ in 0..max_attempts {
        let bundle = KeyBundle::generate()?;
        if count_leading_zero_nibbles(&bundle.xonly_public_key().to_bytes()) >= difficulty {
            return Ok(bundle);
        }
    }
    Err(SecpError::ExhaustedAttempts)
}

#[cfg(feature = "batch")]
pub fn mine_pow_best(min_difficulty: u8, max_attempts: u64) -> Result<(KeyBundle, u8), SecpError> {
    let mut best: Option<(KeyBundle, u8)> = None;
    for _ in 0..max_attempts {
        let bundle = KeyBundle::generate()?;
        let diff = count_leading_zero_nibbles(&bundle.xonly_public_key().to_bytes());
        if diff >= min_difficulty {
            match best {
                Some((_, best_diff)) if diff <= best_diff => {}
                _ => best = Some((bundle, diff)),
            }
        }
    }
    best.ok_or(SecpError::ExhaustedAttempts)
}

#[cfg(feature = "batch")]
pub(crate) fn count_leading_zero_nibbles(bytes: &[u8]) -> u8 {
    let mut count = 0u8;
    for &byte in bytes {
        let high = byte >> 4;
        if high == 0 {
            count += 1;
        } else {
            break;
        }

        let low = byte & 0x0f;
        if low == 0 {
            count += 1;
        } else {
            break;
        }
    }
    count
}
