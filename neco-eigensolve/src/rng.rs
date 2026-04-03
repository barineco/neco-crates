/// Xoroshiro128+ pseudo-random number generator.
///
/// Designed for FEAST initial random matrix generation.
/// Deterministic: same seed always produces the same sequence.
pub struct Rng {
    s0: u64,
    s1: u64,
}

/// Default seed used when `seed == 0`.
const DEFAULT_SEED: u64 = 0xDEAD_BEEF;

impl Rng {
    /// Create a new PRNG seeded via SplitMix64.
    ///
    /// If `seed` is 0, a default seed is used so that the state is never
    /// all-zeros (which is an absorbing state for xoroshiro).
    pub fn new(seed: u64) -> Self {
        let seed = if seed == 0 { DEFAULT_SEED } else { seed };
        let s0 = splitmix64(seed);
        let s1 = splitmix64(s0);
        Self { s0, s1 }
    }

    /// Generate the next `u64` using the xoroshiro128+ algorithm.
    #[inline]
    fn next_u64(&mut self) -> u64 {
        let s0 = self.s0;
        let mut s1 = self.s1;
        let result = s0.wrapping_add(s1);

        s1 ^= s0;
        self.s0 = s0.rotate_left(24) ^ s1 ^ (s1 << 16);
        self.s1 = s1.rotate_left(37);

        result
    }

    /// Generate a uniformly distributed `f64` in `[0, 1)`.
    ///
    /// Uses the upper 53 bits of `next_u64` to fill the mantissa.
    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        let bits = self.next_u64() >> 11; // 53 bits
        bits as f64 * (1.0_f64 / (1u64 << 53) as f64)
    }
}

/// SplitMix64 one-shot mixing function used for seeding.
#[inline]
fn splitmix64(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_reproducibility() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn seed_zero_uses_default() {
        let mut a = Rng::new(0);
        let mut b = Rng::new(DEFAULT_SEED);
        for _ in 0..100 {
            assert_eq!(a.next_f64(), b.next_f64());
        }
    }

    #[test]
    fn different_seeds_differ() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        let va: Vec<u64> = (0..10).map(|_| a.next_u64()).collect();
        let vb: Vec<u64> = (0..10).map(|_| b.next_u64()).collect();
        assert_ne!(va, vb);
    }

    #[test]
    fn next_f64_range() {
        let mut rng = Rng::new(12345);
        for _ in 0..100_000 {
            let v = rng.next_f64();
            assert!((0.0..1.0).contains(&v), "out of range: {v}");
        }
    }

    #[test]
    fn distribution_not_biased() {
        // Split [0, 1) into 10 bins and check that each bin gets a
        // reasonable share of 100k samples (expect ~10k each).
        let mut rng = Rng::new(7777);
        let n = 100_000;
        let mut bins = [0u32; 10];
        for _ in 0..n {
            let v = rng.next_f64();
            let idx = (v * 10.0) as usize;
            bins[idx.min(9)] += 1;
        }
        let expected = n as f64 / 10.0;
        for (i, &count) in bins.iter().enumerate() {
            let ratio = count as f64 / expected;
            assert!(
                (0.9..1.1).contains(&ratio),
                "bin {i}: count={count}, ratio={ratio:.3} — biased",
            );
        }
    }
}
