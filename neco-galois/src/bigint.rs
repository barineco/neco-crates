use core::cmp::Ordering;

/// 256-bit unsigned integer. Little-endian 4 × u64 limb representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct U256 {
    pub l0: u64,
    pub l1: u64,
    pub l2: u64,
    pub l3: u64,
}

impl U256 {
    /// ゼロ
    pub const ZERO: U256 = U256 {
        l0: 0,
        l1: 0,
        l2: 0,
        l3: 0,
    };

    /// 一
    pub const ONE: U256 = U256 {
        l0: 1,
        l1: 0,
        l2: 0,
        l3: 0,
    };

    /// u64 からの変換
    pub const fn from_u64(v: u64) -> U256 {
        U256 {
            l0: v,
            l1: 0,
            l2: 0,
            l3: 0,
        }
    }

    /// big-endian バイト列からの変換
    pub fn from_be_bytes(b: [u8; 32]) -> U256 {
        let l3 = u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
        let l2 = u64::from_be_bytes([b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]]);
        let l1 = u64::from_be_bytes([b[16], b[17], b[18], b[19], b[20], b[21], b[22], b[23]]);
        let l0 = u64::from_be_bytes([b[24], b[25], b[26], b[27], b[28], b[29], b[30], b[31]]);
        U256 { l0, l1, l2, l3 }
    }

    /// big-endian バイト列への変換
    pub fn to_be_bytes(self) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[0..8].copy_from_slice(&self.l3.to_be_bytes());
        out[8..16].copy_from_slice(&self.l2.to_be_bytes());
        out[16..24].copy_from_slice(&self.l1.to_be_bytes());
        out[24..32].copy_from_slice(&self.l0.to_be_bytes());
        out
    }

    /// little-endian バイト列からの変換
    pub fn from_le_bytes(b: [u8; 32]) -> U256 {
        let l0 = u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
        let l1 = u64::from_le_bytes([b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]]);
        let l2 = u64::from_le_bytes([b[16], b[17], b[18], b[19], b[20], b[21], b[22], b[23]]);
        let l3 = u64::from_le_bytes([b[24], b[25], b[26], b[27], b[28], b[29], b[30], b[31]]);
        U256 { l0, l1, l2, l3 }
    }

    /// little-endian バイト列への変換
    pub fn to_le_bytes(self) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[0..8].copy_from_slice(&self.l0.to_le_bytes());
        out[8..16].copy_from_slice(&self.l1.to_le_bytes());
        out[16..24].copy_from_slice(&self.l2.to_le_bytes());
        out[24..32].copy_from_slice(&self.l3.to_le_bytes());
        out
    }

    /// Returns `(result, carry)` such that `result + carry × 2^256 = a + b`.
    #[inline]
    pub fn add(a: U256, b: U256) -> (U256, bool) {
        let (l0, c0) = a.l0.overflowing_add(b.l0);
        let (l1, c1a) = a.l1.overflowing_add(b.l1);
        let (l1, c1b) = l1.overflowing_add(c0 as u64);
        let (l2, c2a) = a.l2.overflowing_add(b.l2);
        let (l2, c2b) = l2.overflowing_add((c1a | c1b) as u64);
        let (l3, c3a) = a.l3.overflowing_add(b.l3);
        let (l3, c3b) = l3.overflowing_add((c2a | c2b) as u64);
        (U256 { l0, l1, l2, l3 }, c3a | c3b)
    }

    /// Returns `(result, borrow)` such that `result + b = a + borrow × 2^256`.
    #[inline]
    pub fn sub(a: U256, b: U256) -> (U256, bool) {
        let (l0, b0) = a.l0.overflowing_sub(b.l0);
        let (l1, b1a) = a.l1.overflowing_sub(b.l1);
        let (l1, b1b) = l1.overflowing_sub(b0 as u64);
        let (l2, b2a) = a.l2.overflowing_sub(b.l2);
        let (l2, b2b) = l2.overflowing_sub((b1a | b1b) as u64);
        let (l3, b3a) = a.l3.overflowing_sub(b.l3);
        let (l3, b3b) = l3.overflowing_sub((b2a | b2b) as u64);
        (U256 { l0, l1, l2, l3 }, b3a | b3b)
    }

    /// 512-bit schoolbook multiplication. Returns `[u64; 8]` little-endian limbs.
    pub fn mul_wide(a: U256, b: U256) -> [u64; 8] {
        // Row-by-row 乗算。各行の carry を次の列に伝播する。
        // r[i+j] += a[i] * b[j] の順で処理し、溢れを carry として伝播。
        let mut r = [0u64; 8];
        let la = [a.l0, a.l1, a.l2, a.l3];
        let lb = [b.l0, b.l1, b.l2, b.l3];

        for i in 0..4usize {
            let mut carry: u64 = 0;
            for (j, &lbj) in lb.iter().enumerate() {
                let pos = i + j;
                // r[pos] + la[i]*lb[j] + carry は最大:
                // (2^64-1) + (2^64-1)^2 + (2^64-1) = 2^128 - 1 (u128 に収まる)
                let cur = (r[pos] as u128) + (la[i] as u128) * (lbj as u128) + (carry as u128);
                r[pos] = cur as u64;
                carry = (cur >> 64) as u64;
            }
            // carry を r[i+4] に加算（前の行の carry と合算）
            let (new_r, oc) = r[i + 4].overflowing_add(carry);
            r[i + 4] = new_r;
            // i+4 の溢れは発生しないはず (積全体は 512-bit)
            // ただし安全のため assert は入れない (const の問題)
            let _ = oc;
        }

        r
    }

    /// Lexicographic comparison from the most significant limb.
    #[inline]
    pub fn cmp(a: U256, b: U256) -> Ordering {
        match a.l3.cmp(&b.l3) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match a.l2.cmp(&b.l2) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match a.l1.cmp(&b.l1) {
            Ordering::Equal => {}
            ord => return ord,
        }
        a.l0.cmp(&b.l0)
    }

    /// ゼロ判定
    #[inline]
    pub fn is_zero(a: U256) -> bool {
        a.l0 == 0 && a.l1 == 0 && a.l2 == 0 && a.l3 == 0
    }

    /// constant-time 条件選択: if c { b } else { a }
    #[inline]
    pub fn select(a: U256, b: U256, c: bool) -> U256 {
        let mask = (c as u64).wrapping_neg();
        U256 {
            l0: a.l0 ^ ((a.l0 ^ b.l0) & mask),
            l1: a.l1 ^ ((a.l1 ^ b.l1) & mask),
            l2: a.l2 ^ ((a.l2 ^ b.l2) & mask),
            l3: a.l3 ^ ((a.l3 ^ b.l3) & mask),
        }
    }

    /// 右シフト 1 bit（スカラー乗算の square-and-multiply 用）
    #[inline]
    pub fn shr1(a: U256) -> U256 {
        U256 {
            l0: (a.l0 >> 1) | (a.l1 << 63),
            l1: (a.l1 >> 1) | (a.l2 << 63),
            l2: (a.l2 >> 1) | (a.l3 << 63),
            l3: a.l3 >> 1,
        }
    }

    /// bit i を取得 (0-indexed, 0 = LSB)
    #[inline]
    pub fn bit(a: U256, i: u32) -> bool {
        let limb = match i / 64 {
            0 => a.l0,
            1 => a.l1,
            2 => a.l2,
            3 => a.l3,
            _ => 0,
        };
        (limb >> (i % 64)) & 1 == 1
    }
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- round-trip tests (10本) ---

    #[test]
    fn rt_zero_be() {
        let b = [0u8; 32];
        assert_eq!(U256::from_be_bytes(b).to_be_bytes(), b);
    }

    #[test]
    fn rt_one_be() {
        let mut b = [0u8; 32];
        b[31] = 1;
        let u = U256::from_be_bytes(b);
        assert_eq!(u, U256::ONE);
        assert_eq!(u.to_be_bytes(), b);
    }

    #[test]
    fn rt_max_be() {
        let b = [0xffu8; 32];
        assert_eq!(U256::from_be_bytes(b).to_be_bytes(), b);
    }

    #[test]
    fn rt_zero_le() {
        let b = [0u8; 32];
        assert_eq!(U256::from_le_bytes(b).to_le_bytes(), b);
    }

    #[test]
    fn rt_one_le() {
        let mut b = [0u8; 32];
        b[0] = 1;
        let u = U256::from_le_bytes(b);
        assert_eq!(u, U256::ONE);
        assert_eq!(u.to_le_bytes(), b);
    }

    #[test]
    fn rt_max_le() {
        let b = [0xffu8; 32];
        assert_eq!(U256::from_le_bytes(b).to_le_bytes(), b);
    }

    #[test]
    fn rt_known_be_le_consistency() {
        // BE と LE の一貫性
        let mut be = [0u8; 32];
        be[0] = 0xDE;
        be[1] = 0xAD;
        be[30] = 0xBE;
        be[31] = 0xEF;
        let from_be = U256::from_be_bytes(be);
        // BE→LE バイト列は逆順
        let le = from_be.to_le_bytes();
        let from_le = U256::from_le_bytes(le);
        assert_eq!(from_be, from_le);
    }

    #[test]
    fn rt_from_u64() {
        let u = U256::from_u64(0xDEADBEEF_CAFEBABE);
        assert_eq!(u.l0, 0xDEADBEEF_CAFEBABE);
        assert_eq!(u.l1, 0);
        assert_eq!(u.l2, 0);
        assert_eq!(u.l3, 0);
    }

    #[test]
    fn rt_multi_limb() {
        let u = U256 {
            l0: 1,
            l1: 2,
            l2: 3,
            l3: 4,
        };
        let be = u.to_be_bytes();
        assert_eq!(U256::from_be_bytes(be), u);
        let le = u.to_le_bytes();
        assert_eq!(U256::from_le_bytes(le), u);
    }

    #[test]
    fn rt_limb_order_be() {
        // l3 が MSB side に来る
        let u = U256 {
            l0: 0,
            l1: 0,
            l2: 0,
            l3: 1,
        };
        let be = u.to_be_bytes();
        assert_eq!(&be[0..8], &[0, 0, 0, 0, 0, 0, 0, 1]);
    }

    // --- 加減算 carry/borrow 境界値 (8本) ---

    #[test]
    fn add_no_carry() {
        let a = U256::from_u64(1);
        let b = U256::from_u64(2);
        let (r, carry) = U256::add(a, b);
        assert_eq!(r, U256::from_u64(3));
        assert!(!carry);
    }

    #[test]
    fn add_l0_overflow() {
        let a = U256 {
            l0: u64::MAX,
            l1: 0,
            l2: 0,
            l3: 0,
        };
        let b = U256::from_u64(1);
        let (r, carry) = U256::add(a, b);
        assert_eq!(
            r,
            U256 {
                l0: 0,
                l1: 1,
                l2: 0,
                l3: 0
            }
        );
        assert!(!carry);
    }

    #[test]
    fn add_all_limbs_overflow() {
        let max = U256 {
            l0: u64::MAX,
            l1: u64::MAX,
            l2: u64::MAX,
            l3: u64::MAX,
        };
        let (r, carry) = U256::add(max, U256::ONE);
        assert_eq!(r, U256::ZERO);
        assert!(carry);
    }

    #[test]
    fn add_zero() {
        let a = U256 {
            l0: 0xABCD,
            l1: 0,
            l2: 0,
            l3: 0,
        };
        let (r, carry) = U256::add(a, U256::ZERO);
        assert_eq!(r, a);
        assert!(!carry);
    }

    #[test]
    fn sub_no_borrow() {
        let a = U256::from_u64(5);
        let b = U256::from_u64(3);
        let (r, borrow) = U256::sub(a, b);
        assert_eq!(r, U256::from_u64(2));
        assert!(!borrow);
    }

    #[test]
    fn sub_borrow_l0() {
        let a = U256 {
            l0: 0,
            l1: 1,
            l2: 0,
            l3: 0,
        };
        let b = U256::from_u64(1);
        let (r, borrow) = U256::sub(a, b);
        assert_eq!(
            r,
            U256 {
                l0: u64::MAX,
                l1: 0,
                l2: 0,
                l3: 0
            }
        );
        assert!(!borrow);
    }

    #[test]
    fn sub_underflow() {
        let (r, borrow) = U256::sub(U256::ZERO, U256::ONE);
        assert!(borrow);
        // 2^256 - 1
        assert_eq!(
            r,
            U256 {
                l0: u64::MAX,
                l1: u64::MAX,
                l2: u64::MAX,
                l3: u64::MAX
            }
        );
    }

    #[test]
    fn sub_equal() {
        let a = U256 {
            l0: 0xFF,
            l1: 0xAB,
            l2: 0,
            l3: 0,
        };
        let (r, borrow) = U256::sub(a, a);
        assert_eq!(r, U256::ZERO);
        assert!(!borrow);
    }

    // --- 乗算既知ベクタ (5本) ---

    #[test]
    fn mul_wide_zero() {
        let t = U256::mul_wide(U256::ZERO, U256::from_u64(12345));
        assert_eq!(t, [0u64; 8]);
    }

    #[test]
    fn mul_wide_one() {
        let a = U256 {
            l0: 0xDEAD,
            l1: 0xBEEF,
            l2: 0,
            l3: 0,
        };
        let t = U256::mul_wide(a, U256::ONE);
        assert_eq!(t[0], 0xDEAD);
        assert_eq!(t[1], 0xBEEF);
        assert_eq!(&t[2..], &[0u64; 6]);
    }

    #[test]
    fn mul_wide_small() {
        let a = U256::from_u64(1000);
        let b = U256::from_u64(2000);
        let t = U256::mul_wide(a, b);
        assert_eq!(t[0], 2_000_000);
        assert_eq!(&t[1..], &[0u64; 7]);
    }

    #[test]
    fn mul_wide_l0_full() {
        // u64::MAX * u64::MAX = (2^64-1)^2 = 2^128 - 2^65 + 1
        let a = U256 {
            l0: u64::MAX,
            l1: 0,
            l2: 0,
            l3: 0,
        };
        let b = U256 {
            l0: u64::MAX,
            l1: 0,
            l2: 0,
            l3: 0,
        };
        let t = U256::mul_wide(a, b);
        // (2^64-1)^2 = 0xFFFFFFFFFFFFFFFE_0000000000000001
        assert_eq!(t[0], 1u64);
        assert_eq!(t[1], u64::MAX - 1);
        assert_eq!(&t[2..], &[0u64; 6]);
    }

    #[test]
    fn mul_wide_multidigit() {
        // 2^64 * 2^64 = 2^128
        let a = U256 {
            l0: 0,
            l1: 1,
            l2: 0,
            l3: 0,
        };
        let b = U256 {
            l0: 0,
            l1: 1,
            l2: 0,
            l3: 0,
        };
        let t = U256::mul_wide(a, b);
        // 2^128 → limb index 2
        assert_eq!(t[2], 1);
        assert_eq!(t[0], 0);
        assert_eq!(t[1], 0);
        assert_eq!(&t[3..], &[0u64; 5]);
    }

    // --- cmp tests ---

    #[test]
    fn cmp_equal() {
        let a = U256::from_u64(42);
        assert_eq!(U256::cmp(a, a), Ordering::Equal);
    }

    #[test]
    fn cmp_less() {
        assert_eq!(U256::cmp(U256::ONE, U256::from_u64(2)), Ordering::Less);
    }

    #[test]
    fn cmp_greater_high_limb() {
        let a = U256 {
            l0: 0,
            l1: 0,
            l2: 0,
            l3: 1,
        };
        let b = U256 {
            l0: u64::MAX,
            l1: u64::MAX,
            l2: u64::MAX,
            l3: 0,
        };
        assert_eq!(U256::cmp(a, b), Ordering::Greater);
    }
}
