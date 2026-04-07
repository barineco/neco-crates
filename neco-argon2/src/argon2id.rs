//! Argon2id password hashing (RFC 9106).
//!
//! Implements Argon2id: Argon2i (data-independent) for the first two slices
//! of the first pass, Argon2d (data-dependent) otherwise.

use crate::blake2b::{blake2b, blake2b_long};
use neco_base64::{decode_url, encode_url};

// ── Public types ─────────────────────────────────────────────────────────────

/// Argon2id tuning parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Argon2Params {
    /// Memory cost in KiB (minimum 8 * p_cost).
    pub m_cost: u32,
    /// Time cost (number of passes).
    pub t_cost: u32,
    /// Parallelism (number of lanes).
    pub p_cost: u32,
    /// Output length in bytes.
    pub output_len: usize,
}

impl Default for Argon2Params {
    fn default() -> Self {
        // OWASP recommended defaults (2023): m=19456 KiB (19 MiB), t=2, p=1
        Self {
            m_cost: 19456,
            t_cost: 2,
            p_cost: 1,
            output_len: 32,
        }
    }
}

// ── Constants ─────────────────────────────────────────────────────────────────

const BLOCK_BYTES: usize = 1024;
const BLOCK_WORDS: usize = BLOCK_BYTES / 8; // 128 u64 words
const VERSION: u32 = 0x13;
const ARGON2ID_TYPE: u32 = 2;
/// Number of u64 addresses per generated address block.
const ADDRESSES_IN_BLOCK: usize = 128;

// ── Block ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct Block([u64; BLOCK_WORDS]);

impl Default for Block {
    fn default() -> Self {
        Block([0u64; BLOCK_WORDS])
    }
}

impl Block {
    #[inline]
    fn xor_assign(&mut self, other: &Block) {
        for (a, b) in self.0.iter_mut().zip(other.0.iter()) {
            *a ^= b;
        }
    }

    fn to_bytes(&self) -> [u8; BLOCK_BYTES] {
        let mut out = [0u8; BLOCK_BYTES];
        for (i, w) in self.0.iter().enumerate() {
            out[i * 8..(i + 1) * 8].copy_from_slice(&w.to_le_bytes());
        }
        out
    }

    fn from_bytes(bytes: &[u8]) -> Self {
        assert_eq!(bytes.len(), BLOCK_BYTES);
        let mut b = Block::default();
        for (i, w) in b.0.iter_mut().enumerate() {
            *w = u64::from_le_bytes(bytes[i * 8..(i + 1) * 8].try_into().unwrap());
        }
        b
    }
}

impl core::ops::BitXorAssign<&Block> for Block {
    fn bitxor_assign(&mut self, rhs: &Block) {
        self.xor_assign(rhs);
    }
}

// ── Argon2 G mixing function ──────────────────────────────────────────────────

/// One step of the Argon2 G function (RFC 9106, Section 3.4).
///
/// Adds a multiplication term for memory-hardness.
macro_rules! permute_step {
    ($a:expr, $b:expr, $c:expr, $d:expr) => {{
        const TRUNC: u64 = u32::MAX as u64;
        $a = $a
            .wrapping_add($b)
            .wrapping_add(2u64.wrapping_mul($a & TRUNC).wrapping_mul($b & TRUNC));
        $d = ($d ^ $a).rotate_right(32);
        $c = $c
            .wrapping_add($d)
            .wrapping_add(2u64.wrapping_mul($c & TRUNC).wrapping_mul($d & TRUNC));
        $b = ($b ^ $c).rotate_right(24);
        $a = $a
            .wrapping_add($b)
            .wrapping_add(2u64.wrapping_mul($a & TRUNC).wrapping_mul($b & TRUNC));
        $d = ($d ^ $a).rotate_right(16);
        $c = $c
            .wrapping_add($d)
            .wrapping_add(2u64.wrapping_mul($c & TRUNC).wrapping_mul($d & TRUNC));
        $b = ($b ^ $c).rotate_right(63);
    }};
}

/// Apply GB permutation to 16 words in-place (on a local array).
#[inline(always)]
fn permute16(v: &mut [u64; 16]) {
    permute_step!(v[0], v[4], v[8], v[12]);
    permute_step!(v[1], v[5], v[9], v[13]);
    permute_step!(v[2], v[6], v[10], v[14]);
    permute_step!(v[3], v[7], v[11], v[15]);
    permute_step!(v[0], v[5], v[10], v[15]);
    permute_step!(v[1], v[6], v[11], v[12]);
    permute_step!(v[2], v[7], v[8], v[13]);
    permute_step!(v[3], v[4], v[9], v[14]);
}

/// Apply GB permutation to 16 words extracted from a block by indices.
fn permute16_indexed(q: &mut [u64; BLOCK_WORDS], indices: [usize; 16]) {
    let mut tmp = [
        q[indices[0]],
        q[indices[1]],
        q[indices[2]],
        q[indices[3]],
        q[indices[4]],
        q[indices[5]],
        q[indices[6]],
        q[indices[7]],
        q[indices[8]],
        q[indices[9]],
        q[indices[10]],
        q[indices[11]],
        q[indices[12]],
        q[indices[13]],
        q[indices[14]],
        q[indices[15]],
    ];
    permute16(&mut tmp);
    for (k, &idx) in indices.iter().enumerate() {
        q[idx] = tmp[k];
    }
}

/// Argon2 block compression function G (RFC 9106, Section 3.4).
///
/// r = rhs XOR lhs; apply row then column permutations; return q XOR r.
fn compress(rhs: &Block, lhs: &Block) -> Block {
    let mut r = *rhs;
    r ^= lhs;
    let mut q = r;

    // Row permutations: 8 rows of 16 consecutive words.
    for row in 0..8usize {
        let base = row * 16;
        let indices = [
            base,
            base + 1,
            base + 2,
            base + 3,
            base + 4,
            base + 5,
            base + 6,
            base + 7,
            base + 8,
            base + 9,
            base + 10,
            base + 11,
            base + 12,
            base + 13,
            base + 14,
            base + 15,
        ];
        permute16_indexed(&mut q.0, indices);
    }

    // Column permutations: 8 columns, each at stride 16 starting from b = i*2.
    for i in 0..8usize {
        let b = i * 2;
        let indices = [
            b,
            b + 1,
            b + 16,
            b + 17,
            b + 32,
            b + 33,
            b + 48,
            b + 49,
            b + 64,
            b + 65,
            b + 80,
            b + 81,
            b + 96,
            b + 97,
            b + 112,
            b + 113,
        ];
        permute16_indexed(&mut q.0, indices);
    }

    q ^= &r;
    q
}

// ── H0 ────────────────────────────────────────────────────────────────────────

/// Generate the initial 64-byte seed H0 (RFC 9106, Section 3.3).
fn compute_h0(
    p_cost: u32,
    output_len: usize,
    m_cost: u32,
    t_cost: u32,
    version: u32,
    argon2_type: u32,
    password: &[u8],
    salt: &[u8],
    secret: &[u8],
    associated_data: &[u8],
) -> [u8; 64] {
    let mut input = Vec::new();
    input.extend_from_slice(&p_cost.to_le_bytes());
    input.extend_from_slice(&(output_len as u32).to_le_bytes());
    input.extend_from_slice(&m_cost.to_le_bytes());
    input.extend_from_slice(&t_cost.to_le_bytes());
    input.extend_from_slice(&version.to_le_bytes());
    input.extend_from_slice(&argon2_type.to_le_bytes());
    input.extend_from_slice(&(password.len() as u32).to_le_bytes());
    input.extend_from_slice(password);
    input.extend_from_slice(&(salt.len() as u32).to_le_bytes());
    input.extend_from_slice(salt);
    input.extend_from_slice(&(secret.len() as u32).to_le_bytes());
    input.extend_from_slice(secret);
    input.extend_from_slice(&(associated_data.len() as u32).to_le_bytes());
    input.extend_from_slice(associated_data);
    blake2b(&input, &[], 64).try_into().unwrap()
}

// ── Core Argon2id ─────────────────────────────────────────────────────────────

/// Internal Argon2id implementation.
fn argon2id_internal(
    password: &[u8],
    salt: &[u8],
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
    output_len: usize,
    secret: &[u8],
    associated_data: &[u8],
) -> Vec<u8> {
    assert!(p_cost >= 1, "p_cost >= 1");
    assert!(t_cost >= 1, "t_cost >= 1");
    assert!(m_cost >= 8 * p_cost, "m_cost >= 8 * p_cost");
    assert!(output_len >= 4, "output_len >= 4");
    assert!(salt.len() >= 8, "salt >= 8 bytes");

    let lanes = p_cost as usize;
    // Segment length = floor(m_cost / (4 * lanes)) * 1 (but lane_length = 4 * seg_len)
    // RFC: m' = floor(m / (4p)) * 4p
    let segment_length = (m_cost as usize / (4 * lanes)).max(1);
    let lane_length = 4 * segment_length;
    let block_count = lanes * lane_length;

    let mut memory = vec![Block::default(); block_count];

    // H0.
    let h0 = compute_h0(
        p_cost,
        output_len,
        m_cost,
        t_cost,
        VERSION,
        ARGON2ID_TYPE,
        password,
        salt,
        secret,
        associated_data,
    );

    // Initialize B[lane][0] and B[lane][1] for each lane.
    for lane in 0..lanes {
        for i in 0usize..2 {
            let mut inp = Vec::with_capacity(72);
            inp.extend_from_slice(&h0);
            inp.extend_from_slice(&(i as u32).to_le_bytes());
            inp.extend_from_slice(&(lane as u32).to_le_bytes());
            let bytes = blake2b_long(&inp, BLOCK_BYTES);
            memory[lane * lane_length + i] = Block::from_bytes(&bytes);
        }
    }

    let zero_block = Block::default();

    // Main passes.
    for pass in 0..t_cost as usize {
        for slice in 0..4usize {
            let data_independent = pass == 0 && slice < 2; // Argon2id rule

            for lane in 0..lanes {
                let mut address_block = Block::default();
                let mut input_block = Block::default();

                if data_independent {
                    // input_block = [pass, lane, slice, block_count, t_cost, type, 0, ...]
                    input_block.0[0] = pass as u64;
                    input_block.0[1] = lane as u64;
                    input_block.0[2] = slice as u64;
                    input_block.0[3] = block_count as u64;
                    input_block.0[4] = t_cost as u64;
                    input_block.0[5] = ARGON2ID_TYPE as u64;
                    // Increment counter and generate first address block.
                    input_block.0[6] += 1;
                    address_block = compress(&zero_block, &input_block);
                    address_block = compress(&zero_block, &address_block);
                }

                // first block: skip 0,1 on pass 0, slice 0.
                let first_block = if pass == 0 && slice == 0 { 2 } else { 0 };

                let base = lane * lane_length + slice * segment_length;

                // Previous block before the first processed block of this segment.
                let mut prev_index = if first_block == 0 {
                    if slice == 0 {
                        lane * lane_length + lane_length - 1
                    } else {
                        base - 1
                    }
                } else {
                    base + first_block - 1
                };

                for block_in_seg in first_block..segment_length {
                    let cur_index = base + block_in_seg;

                    // Get pseudo-random value.
                    let rand = if data_independent {
                        let addr_idx = block_in_seg % ADDRESSES_IN_BLOCK;
                        if addr_idx == 0 && block_in_seg > 0 {
                            // Need a new address block.
                            input_block.0[6] += 1;
                            address_block = compress(&zero_block, &input_block);
                            address_block = compress(&zero_block, &address_block);
                        }
                        address_block.0[addr_idx]
                    } else {
                        memory[prev_index].0[0]
                    };

                    // Reference lane.
                    let ref_lane = if pass == 0 && slice == 0 {
                        lane
                    } else {
                        (rand >> 32) as usize % lanes
                    };

                    // Reference area size.
                    let reference_area_size = if pass == 0 {
                        if slice == 0 {
                            block_in_seg - 1
                        } else if ref_lane == lane {
                            slice * segment_length + block_in_seg - 1
                        } else {
                            slice * segment_length - if block_in_seg == 0 { 1 } else { 0 }
                        }
                    } else if ref_lane == lane {
                        lane_length - segment_length + block_in_seg - 1
                    } else {
                        lane_length - segment_length - if block_in_seg == 0 { 1 } else { 0 }
                    };

                    if reference_area_size == 0 {
                        // No valid reference: use the block itself (degenerate case).
                        // In practice, this should not happen with m_cost >= 8.
                        let new_block = compress(&memory[prev_index], &memory[prev_index]);
                        if pass == 0 {
                            memory[cur_index] = new_block;
                        } else {
                            memory[cur_index] ^= &new_block;
                        }
                        prev_index = cur_index;
                        continue;
                    }

                    // Map rand[0..31] → position within reference area.
                    let mut map = rand & 0xffff_ffff;
                    map = (map * map) >> 32;
                    let relative_pos = reference_area_size
                        - 1
                        - ((reference_area_size as u64 * map) >> 32) as usize;

                    // Starting position.
                    let start_pos = if pass != 0 && slice != 3 {
                        (slice + 1) * segment_length
                    } else {
                        0
                    };

                    let lane_index = (start_pos + relative_pos) % lane_length;
                    let ref_index = ref_lane * lane_length + lane_index;

                    let new_block = compress(&memory[prev_index], &memory[ref_index]);
                    if pass == 0 {
                        memory[cur_index] = new_block;
                    } else {
                        memory[cur_index] ^= &new_block;
                    }

                    prev_index = cur_index;
                }
            }
        }
    }

    // Finalization: XOR last blocks of all lanes.
    let last_col = lane_length - 1;
    let mut fin = memory[last_col]; // lane 0 last block
    for lane in 1..lanes {
        fin ^= &memory[lane * lane_length + last_col];
    }

    let c_bytes = fin.to_bytes();
    blake2b_long(&c_bytes, output_len)
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Compute an Argon2id hash.
///
/// - `password`: password bytes
/// - `salt`: random salt (minimum 8 bytes, recommended 16)
/// - `m_cost`: memory in KiB
/// - `t_cost`: iteration count
/// - `p_cost`: parallelism
/// - `output_len`: output length in bytes (minimum 4)
pub fn argon2id_hash(
    password: &[u8],
    salt: &[u8],
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
    output_len: usize,
) -> Vec<u8> {
    argon2id_internal(password, salt, m_cost, t_cost, p_cost, output_len, &[], &[])
}

/// Compute an Argon2id hash with secret and associated data.
///
/// Used for RFC 9106 test vector verification.
pub fn argon2id_hash_with_secret(
    password: &[u8],
    salt: &[u8],
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
    output_len: usize,
    secret: &[u8],
    associated_data: &[u8],
) -> Vec<u8> {
    argon2id_internal(
        password,
        salt,
        m_cost,
        t_cost,
        p_cost,
        output_len,
        secret,
        associated_data,
    )
}

/// Hash a password and encode as a PHC string.
///
/// Generates a random 16-byte salt via `getrandom`.
/// Format: `$argon2id$v=19$m={m},t={t},p={p}${salt_b64}${hash_b64}`
pub fn argon2id_hash_encoded(password: &[u8], params: Argon2Params) -> String {
    let mut salt = vec![0u8; 16];
    getrandom::fill(&mut salt).expect("getrandom failed");
    encode_with_salt(password, &salt, params)
}

/// Encode with explicit salt (for testing).
pub(crate) fn encode_with_salt(password: &[u8], salt: &[u8], params: Argon2Params) -> String {
    let hash = argon2id_hash(
        password,
        salt,
        params.m_cost,
        params.t_cost,
        params.p_cost,
        params.output_len,
    );
    format!(
        "$argon2id$v=19$m={},t={},p={}${}${}",
        params.m_cost,
        params.t_cost,
        params.p_cost,
        encode_url(salt),
        encode_url(&hash),
    )
}

/// Verify a password against a PHC string.
pub fn argon2id_verify(encoded: &str, password: &[u8]) -> bool {
    match parse_phc(encoded) {
        Some((salt, expected, params)) => {
            let actual = argon2id_hash(
                password,
                &salt,
                params.m_cost,
                params.t_cost,
                params.p_cost,
                params.output_len,
            );
            if actual.len() != expected.len() {
                return false;
            }
            let mut diff = 0u8;
            for (a, b) in actual.iter().zip(expected.iter()) {
                diff |= a ^ b;
            }
            diff == 0
        }
        None => false,
    }
}

/// Parse a PHC string into (salt, hash, params).
fn parse_phc(encoded: &str) -> Option<(Vec<u8>, Vec<u8>, Argon2Params)> {
    let parts: Vec<&str> = encoded.splitn(6, '$').collect();
    if parts.len() != 6 || !parts[0].is_empty() || parts[1] != "argon2id" {
        return None;
    }
    parts[2].strip_prefix("v=")?;

    let mut m_cost = None;
    let mut t_cost = None;
    let mut p_cost = None;
    for kv in parts[3].split(',') {
        if let Some(v) = kv.strip_prefix("m=") {
            m_cost = v.parse().ok();
        } else if let Some(v) = kv.strip_prefix("t=") {
            t_cost = v.parse().ok();
        } else if let Some(v) = kv.strip_prefix("p=") {
            p_cost = v.parse().ok();
        }
    }

    let salt = decode_url(parts[4]).ok()?;
    let hash = decode_url(parts[5]).ok()?;
    let output_len = hash.len();

    Some((
        salt,
        hash,
        Argon2Params {
            m_cost: m_cost?,
            t_cost: t_cost?,
            p_cost: p_cost?,
            output_len,
        },
    ))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_to_bytes(s: &str) -> Vec<u8> {
        let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    /// RFC 9106 Appendix A.5: Argon2id (with secret and AD).
    #[test]
    fn test_argon2id_rfc9106_full() {
        let password = vec![0x01u8; 32];
        let salt = vec![0x02u8; 16];
        let secret = vec![0x03u8; 8];
        let ad = vec![0x04u8; 12];
        // t=3, m=32, p=4, output_len=32
        let expected = hex_to_bytes(
            "0d640df58d78766c08c037a34a8b53c9\
             d01ef0452d75b65eb52520e96b01e659",
        );
        let result = argon2id_hash_with_secret(&password, &salt, 32, 3, 4, 32, &secret, &ad);
        assert_eq!(result, expected);
    }

    /// Determinism check.
    #[test]
    fn test_deterministic() {
        let password = b"password";
        let salt = b"saltsaltsalt1234";
        let h1 = argon2id_hash(password, salt, 64, 1, 1, 32);
        let h2 = argon2id_hash(password, salt, 64, 1, 1, 32);
        assert_eq!(h1, h2);
    }

    /// Different passwords → different hashes.
    #[test]
    fn test_different_passwords() {
        let salt = b"saltsaltsalt1234";
        let h1 = argon2id_hash(b"password1", salt, 64, 1, 1, 32);
        let h2 = argon2id_hash(b"password2", salt, 64, 1, 1, 32);
        assert_ne!(h1, h2);
    }

    /// PHC encode/verify round-trip.
    #[test]
    fn test_phc_roundtrip() {
        let password = b"hunter2";
        let salt = b"saltsaltsalt1234";
        let params = Argon2Params {
            m_cost: 64,
            t_cost: 1,
            p_cost: 1,
            output_len: 32,
        };
        let encoded = encode_with_salt(password, salt, params);
        assert!(encoded.starts_with("$argon2id$v=19$m=64,t=1,p=1$"));
        assert!(argon2id_verify(&encoded, password));
        assert!(!argon2id_verify(&encoded, b"wrong"));
    }

    /// Invalid PHC strings.
    #[test]
    fn test_phc_invalid() {
        assert!(!argon2id_verify("", b"pw"));
        assert!(!argon2id_verify(
            "$argon2i$v=19$m=32,t=1,p=1$aaa$bbb",
            b"pw"
        ));
        assert!(!argon2id_verify("notphc", b"pw"));
    }

    /// Vary output length.
    #[test]
    fn test_output_lengths() {
        let salt = b"saltsaltsalt1234";
        for len in [16usize, 32, 64] {
            let h = argon2id_hash(b"pw", salt, 64, 1, 1, len);
            assert_eq!(h.len(), len);
        }
    }
}
