//! Deterministic non-cryptographic random generators and stable bucket assignment.
//!
//! This crate is not suitable for cryptographic key generation, nonce generation,
//! token generation, or any other security-sensitive purpose.

/// SplitMix64 one-shot mixer and stream generator.
#[derive(Debug, Clone)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// Create a new generator from a deterministic seed.
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Generate the next `u64`.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix64(self.state)
    }
}

/// xoroshiro128+ pseudo-random number generator.
#[derive(Debug, Clone)]
pub struct Xoroshiro128Plus {
    s0: u64,
    s1: u64,
}

impl Xoroshiro128Plus {
    /// Create a new generator seeded through SplitMix64.
    pub fn new(seed: u64) -> Self {
        let mut seeder = SplitMix64::new(seed);
        let s0 = seeder.next_u64();
        let mut s1 = seeder.next_u64();
        if s0 == 0 && s1 == 0 {
            s1 = 1;
        }
        Self { s0, s1 }
    }

    /// Generate the next `u64`.
    pub fn next_u64(&mut self) -> u64 {
        let s0 = self.s0;
        let mut s1 = self.s1;
        let result = s0.wrapping_add(s1);

        s1 ^= s0;
        self.s0 = s0.rotate_left(24) ^ s1 ^ (s1 << 16);
        self.s1 = s1.rotate_left(37);

        result
    }

    /// Generate a uniformly distributed `f64` in `[0, 1)`.
    pub fn next_f64(&mut self) -> f64 {
        let bits = self.next_u64() >> 11;
        bits as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

/// Stable bucket assignment helpers.
pub mod bucket {
    /// Assign a stable `u64` value from key, experiment, and salt bytes.
    pub fn assign_u64(key: &[u8], experiment: &[u8], salt: &[u8]) -> u64 {
        let mut state = 0xCBF2_9CE4_8422_2325u64;
        state = mix_bytes(state, key);
        state = mix_bytes(state, &[0xFF]);
        state = mix_bytes(state, experiment);
        state = mix_bytes(state, &[0xFE]);
        state = mix_bytes(state, salt);
        super::mix64(state)
    }

    /// Assign a stable ratio in `[0, 1)`.
    pub fn assign_ratio(key: &[u8], experiment: &[u8], salt: &[u8]) -> f64 {
        let bits = assign_u64(key, experiment, salt) >> 11;
        bits as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Assign a stable bucket index in `0..bucket_count`.
    pub fn assign_bucket(key: &[u8], experiment: &[u8], salt: &[u8], bucket_count: u64) -> u64 {
        assert!(bucket_count > 0, "bucket_count must be positive");
        assign_u64(key, experiment, salt) % bucket_count
    }

    fn mix_bytes(mut state: u64, bytes: &[u8]) -> u64 {
        for &byte in bytes {
            state ^= u64::from(byte);
            state = state.wrapping_mul(0x0000_0100_0000_01B3);
        }
        state
    }
}

#[inline]
fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix_reproducible() {
        let mut a = SplitMix64::new(42);
        let mut b = SplitMix64::new(42);
        for _ in 0..128 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn xoroshiro_reproducible() {
        let mut a = Xoroshiro128Plus::new(1234);
        let mut b = Xoroshiro128Plus::new(1234);
        for _ in 0..128 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn xoroshiro_next_f64_in_range() {
        let mut rng = Xoroshiro128Plus::new(999);
        for _ in 0..10_000 {
            let value = rng.next_f64();
            assert!((0.0..1.0).contains(&value), "value={value}");
        }
    }

    #[test]
    fn bucket_ratio_is_in_unit_interval() {
        for seed in 0..256u64 {
            let ratio = bucket::assign_ratio(&seed.to_le_bytes(), b"exp-a", b"salt");
            assert!((0.0..1.0).contains(&ratio), "ratio={ratio}");
        }
    }

    #[test]
    fn bucket_assignment_is_stable() {
        let a = bucket::assign_bucket(b"user-1", b"exp-a", b"salt", 100);
        let b = bucket::assign_bucket(b"user-1", b"exp-a", b"salt", 100);
        assert_eq!(a, b);
    }

    #[test]
    fn bucket_assignment_respects_bucket_bounds() {
        for bucket_count in [1u64, 2, 7, 100] {
            let bucket = bucket::assign_bucket(b"user-1", b"exp-a", b"salt", bucket_count);
            assert!(
                bucket < bucket_count,
                "bucket={bucket}, bucket_count={bucket_count}"
            );
        }
    }

    #[test]
    fn bucket_assignment_changes_when_experiment_changes() {
        let a = bucket::assign_u64(b"user-1", b"exp-a", b"salt");
        let b = bucket::assign_u64(b"user-1", b"exp-b", b"salt");
        assert_ne!(a, b);
    }

    #[test]
    fn bucket_assignment_changes_when_salt_changes() {
        let a = bucket::assign_u64(b"user-1", b"exp-a", b"salt-a");
        let b = bucket::assign_u64(b"user-1", b"exp-a", b"salt-b");
        assert_ne!(a, b);
    }

    #[test]
    #[should_panic(expected = "bucket_count must be positive")]
    fn bucket_assignment_rejects_zero_bucket_count() {
        let _ = bucket::assign_bucket(b"user-1", b"exp-a", b"salt", 0);
    }
}
