//! Blake2b hash function (RFC 7693).
//!
//! Used internally by Argon2id as the mixing hash function.
//! Supports variable output length from 1 to 64 bytes.

/// Blake2b initialization vector (first 64 bits of fractional parts of sqrt of primes 2..19).
/// RFC 7693, Section 2.6.
const IV: [u64; 8] = [
    0x6a09e667f3bcc908,
    0xbb67ae8584caa73b,
    0x3c6ef372fe94f82b,
    0xa54ff53a5f1d36f1,
    0x510e527fade682d1,
    0x9b05688c2b3e6c1f,
    0x1f83d9abfb41bd6b,
    0x5be0cd19137e2179,
];

/// Blake2b σ permutation table (RFC 7693, Section 2.7).
const SIGMA: [[usize; 16]; 12] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
    [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
    [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
    [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
    [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
    [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
    [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
    [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
    [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
];

/// Blake2b G mixing function (RFC 7693, Section 3.1).
#[inline(always)]
fn g(v: &mut [u64; 16], a: usize, b: usize, c: usize, d: usize, x: u64, y: u64) {
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
    v[d] = (v[d] ^ v[a]).rotate_right(32);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(24);
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
    v[d] = (v[d] ^ v[a]).rotate_right(16);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(63);
}

/// Blake2b compression function F (RFC 7693, Section 3.2).
pub fn compress(h: &mut [u64; 8], m: &[u64; 16], t: [u64; 2], last_block: bool) {
    let mut v = [0u64; 16];
    v[0..8].copy_from_slice(h);
    v[8] = IV[0];
    v[9] = IV[1];
    v[10] = IV[2];
    v[11] = IV[3];
    v[12] = IV[4] ^ t[0];
    v[13] = IV[5] ^ t[1];
    v[14] = if last_block {
        IV[6] ^ 0xffff_ffff_ffff_ffff
    } else {
        IV[6]
    };
    v[15] = IV[7];

    for s in &SIGMA {
        g(&mut v, 0, 4, 8, 12, m[s[0]], m[s[1]]);
        g(&mut v, 1, 5, 9, 13, m[s[2]], m[s[3]]);
        g(&mut v, 2, 6, 10, 14, m[s[4]], m[s[5]]);
        g(&mut v, 3, 7, 11, 15, m[s[6]], m[s[7]]);
        g(&mut v, 0, 5, 10, 15, m[s[8]], m[s[9]]);
        g(&mut v, 1, 6, 11, 12, m[s[10]], m[s[11]]);
        g(&mut v, 2, 7, 8, 13, m[s[12]], m[s[13]]);
        g(&mut v, 3, 4, 9, 14, m[s[14]], m[s[15]]);
    }

    for i in 0..8 {
        h[i] ^= v[i] ^ v[i + 8];
    }
}

/// Core Blake2b hash: variable output length (1..=64 bytes), optional key (0..=64 bytes).
///
/// RFC 7693, Section 2.
pub fn blake2b(input: &[u8], key: &[u8], output_len: usize) -> Vec<u8> {
    assert!((1..=64).contains(&output_len), "output_len must be 1..=64");
    assert!(key.len() <= 64, "key length must be <= 64");

    let kk = key.len();
    let nn = output_len;

    // Parameter block p[0]: fan-out=1, max depth=1, leaf length=0, etc.
    // h[0] ^= 0x01010000 ^ (kk << 8) ^ nn
    let mut h = IV;
    h[0] ^= 0x01010000u64 ^ ((kk as u64) << 8) ^ (nn as u64);

    let mut counter: u64 = 0;

    // If key is provided, pad to 128 bytes and prepend as first block.
    if kk > 0 {
        let mut block = [0u8; 128];
        block[..kk].copy_from_slice(key);

        if input.is_empty() {
            // Key block is the only (last) block.
            counter = 128;
            let m = bytes_to_words(&block);
            compress(&mut h, &m, [counter, 0], true);
            return finalize(&h, nn);
        } else {
            counter = 128;
            let m = bytes_to_words(&block);
            compress(&mut h, &m, [counter, 0], false);
        }
    }

    // Process message blocks.
    // Each block is 128 bytes. The last block must be flagged.
    let mut offset = 0;
    let len = input.len();

    if len == 0 {
        // Empty message, no key: compress one zero block as the last.
        let m = [0u64; 16];
        compress(&mut h, &m, [0, 0], true);
        return finalize(&h, nn);
    }

    while offset < len {
        let remaining = len - offset;
        let is_last = remaining <= 128;
        let take = remaining.min(128);

        let mut block = [0u8; 128];
        block[..take].copy_from_slice(&input[offset..offset + take]);

        counter = counter.wrapping_add(if is_last { take as u64 } else { 128 });

        let m = bytes_to_words(&block);
        compress(&mut h, &m, [counter, 0], is_last);

        offset += take;
    }

    finalize(&h, nn)
}

fn bytes_to_words(block: &[u8; 128]) -> [u64; 16] {
    let mut m = [0u64; 16];
    for (i, word) in m.iter_mut().enumerate() {
        let b = &block[i * 8..(i + 1) * 8];
        *word = u64::from_le_bytes(b.try_into().unwrap());
    }
    m
}

fn finalize(h: &[u64; 8], nn: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(nn);
    for word in h {
        out.extend_from_slice(&word.to_le_bytes());
    }
    out.truncate(nn);
    out
}

/// Blake2b with variable output length > 64 bytes (Argon2 internal use).
///
/// RFC 9106, Section 3.2: H' (variable-length hash).
/// For output_len <= 64, this is identical to blake2b().
/// For output_len > 64, it fans out into multiple Blake2b hashes.
pub fn blake2b_long(input: &[u8], output_len: usize) -> Vec<u8> {
    assert!(output_len >= 1);

    if output_len <= 64 {
        // Prepend output_len as 4-byte LE integer.
        let mut msg = Vec::with_capacity(4 + input.len());
        msg.extend_from_slice(&(output_len as u32).to_le_bytes());
        msg.extend_from_slice(input);
        return blake2b(&msg, &[], output_len);
    }

    // r = ceil(output_len / 32) - 2
    let r = output_len.div_ceil(32) - 2;
    let mut out = Vec::with_capacity(output_len);

    // a[1] = Blake2b(LE32(output_len) || input, 64)
    let mut msg = Vec::with_capacity(4 + input.len());
    msg.extend_from_slice(&(output_len as u32).to_le_bytes());
    msg.extend_from_slice(input);
    let mut a_prev = blake2b(&msg, &[], 64);
    out.extend_from_slice(&a_prev[..32]);

    // a[i] = Blake2b(a[i-1], 64) for i = 2..=r
    for _ in 1..r {
        let a_i = blake2b(&a_prev, &[], 64);
        out.extend_from_slice(&a_i[..32]);
        a_prev = a_i;
    }

    // a[r+1] = Blake2b(a[r], remaining)
    let remaining = output_len - 32 * r;
    let a_last = blake2b(&a_prev, &[], remaining);
    out.extend_from_slice(&a_last);

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 7693 Appendix A: Blake2b-512 test vector.
    /// Input: "abc", no key.
    #[test]
    fn test_blake2b_rfc7693_abc() {
        let input = b"abc";
        let result = blake2b(input, &[], 64);
        let expected = hex_to_bytes(
            "ba80a53f981c4d0d6a2797b69f12f6e9\
             4c212f14685ac4b74b12bb6fdbffa2d1\
             7d87c5392aab792dc252d5de4533cc95\
             18d38aa8dbf1925ab92386edd4009923",
        );
        assert_eq!(result, expected);
    }

    /// RFC 7693 Appendix A: Blake2b-512, empty input, no key.
    #[test]
    fn test_blake2b_empty() {
        let result = blake2b(&[], &[], 64);
        let expected = hex_to_bytes(
            "786a02f742015903c6c6fd852552d272\
             912f4740e15847618a86e217f71f5419\
             d25e1031afee585313896444934eb04b\
             903a685b1448b755d56f701afe9be2ce",
        );
        assert_eq!(result, expected);
    }

    fn hex_to_bytes(s: &str) -> Vec<u8> {
        let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }
}
