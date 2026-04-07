// P-256 (NIST) prime field constants.
//   p_p256 = 2^256 - 2^224 + 2^192 + 2^96 - 1
//   2^256 ≡ 2^224 - 2^192 - 2^96 + 1 (mod p_p256)

use crate::bigint::U256;
use crate::fp::PrimeField;

// -----------------------------------------------------------------------
// P256Field: フィールド素数 p
// -----------------------------------------------------------------------

/// P-256 のフィールド素数
/// p = 0xFFFFFFFF00000001000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFF
/// = 2^256 - 2^224 + 2^192 + 2^96 - 1
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct P256Field;

impl PrimeField for P256Field {
    /// p = NIST P-256 prime
    const MODULUS: U256 = U256 {
        l0: 0xFFFFFFFFFFFFFFFF,
        l1: 0x00000000FFFFFFFF,
        l2: 0x0000000000000000,
        l3: 0xFFFFFFFF00000001,
    };

    /// R² mod p = 0x00000004FFFFFFFDFFFFFFFFFFFFFFFEFFFFFFFBFFFFFFFF0000000000000003
    const R_SQUARED: U256 = U256 {
        l0: 0x0000000000000003,
        l1: 0xFFFFFFFBFFFFFFFF,
        l2: 0xFFFFFFFFFFFFFFFE,
        l3: 0x00000004FFFFFFFD,
    };

    /// -p⁻¹ mod 2^64 = 0x0000000000000001
    const INV: u64 = 0x0000000000000001;
}

/// (p+1)/4 — P-256 のフィールド sqrt 用指数
/// p ≡ 3 (mod 4) なので sqrt(a) = a^((p+1)/4)
pub const SQRT_EXP_P256: U256 = U256 {
    l0: 0x0000000000000000,
    l1: 0x0000000040000000,
    l2: 0x4000000000000000,
    l3: 0x3FFFFFFFC0000000,
};

// -----------------------------------------------------------------------
// P256Order: 群位数 n
// -----------------------------------------------------------------------

/// P-256 の群位数 n（スカラー体に使用）
/// n = 0xFFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct P256Order;

impl PrimeField for P256Order {
    /// n = group order of P-256
    const MODULUS: U256 = U256 {
        l0: 0xF3B9CAC2FC632551,
        l1: 0xBCE6FAADA7179E84,
        l2: 0xFFFFFFFFFFFFFFFF,
        l3: 0xFFFFFFFF00000000,
    };

    /// R² mod n = 0x66E12D94F3D956202845B2392B6BEC594699799C49BD6FA683244C95BE79EEA2
    const R_SQUARED: U256 = U256 {
        l0: 0x83244C95BE79EEA2,
        l1: 0x4699799C49BD6FA6,
        l2: 0x2845B2392B6BEC59,
        l3: 0x66E12D94F3D95620,
    };

    /// -n⁻¹ mod 2^64 = 0xCCD1C8AAEE00BC4F
    const INV: u64 = 0xCCD1C8AAEE00BC4F;
}

/// (n+1)/4 — P-256 の群位数の sqrt 指数
pub const SQRT_EXP_P256_ORDER: U256 = U256 {
    l0: 0x3CEE72B0BF18C954,
    l1: 0xEF39BEAB69C5E7A1,
    l2: 0x3FFFFFFFFFFFFFFF,
    l3: 0x3FFFFFFFC0000000,
};

// -----------------------------------------------------------------------
// Tests: 2^256 ≡ 2^224 - 2^192 - 2^96 + 1 (mod p_p256) reduction.
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fp::Fp;

    type Fp256 = Fp<P256Field>;

    #[test]
    fn modulus_correct() {
        // p = 2^256 - 2^224 + 2^192 + 2^96 - 1
        // 確認: p の l0 は 0xFFFFFFFFFFFFFFFF
        let p = P256Field::MODULUS;
        assert_eq!(p.l0, 0xFFFFFFFFFFFFFFFF);
        assert_eq!(p.l1, 0x00000000FFFFFFFF);
        assert_eq!(p.l2, 0x0000000000000000);
        assert_eq!(p.l3, 0xFFFFFFFF00000001);
    }

    #[test]
    fn r_squared_correct() {
        // R² mod p の検証
        assert_eq!(P256Field::R_SQUARED.l0, 0x0000000000000003);
        assert_eq!(P256Field::R_SQUARED.l1, 0xFFFFFFFBFFFFFFFF);
    }

    #[test]
    fn redc_one_gives_one() {
        // from_u256(1).to_u256() == 1
        let one = Fp256::from_u256(U256::ONE);
        assert_eq!(one.to_u256(), U256::ONE);
    }

    #[test]
    fn mul_by_modulus_is_zero() {
        let a = Fp256::from_u256(U256::from_u64(999));
        let p_as_fp = Fp256::from_u256(P256Field::MODULUS);
        assert_eq!(p_as_fp, Fp256::ZERO);
        assert_eq!(Fp::mul(a, p_as_fp), Fp256::ZERO);
    }
}
