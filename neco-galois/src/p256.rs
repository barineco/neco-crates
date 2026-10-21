//! P-256 の有限体とスカラー体の定数です。

use crate::bigint::U256;
use crate::fp::PrimeField;

/// フィールド素数は `p = 2^256 - 2^224 + 2^192 + 2^96 - 1` であり、`2^256 ≡ 2^224 - 2^192 - 2^96 + 1 (mod p)` を満たします。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct P256Field;

impl PrimeField for P256Field {
    const MODULUS: U256 = U256 {
        l0: 0xFFFFFFFFFFFFFFFF,
        l1: 0x00000000FFFFFFFF,
        l2: 0x0000000000000000,
        l3: 0xFFFFFFFF00000001,
    };
    const R_SQUARED: U256 = U256 {
        l0: 0x0000000000000003,
        l1: 0xFFFFFFFBFFFFFFFF,
        l2: 0xFFFFFFFFFFFFFFFE,
        l3: 0x00000004FFFFFFFD,
    };
    const INV: u64 = 0x0000000000000001;
}

/// `p ≡ 3 (mod 4)` における `(p + 1) / 4` の平方根指数です。
pub const SQRT_EXP_P256: U256 = U256 {
    l0: 0x0000000000000000,
    l1: 0x0000000040000000,
    l2: 0x4000000000000000,
    l3: 0x3FFFFFFFC0000000,
};

/// 対応する曲線のスカラー体の位数です。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct P256Order;

impl PrimeField for P256Order {
    const MODULUS: U256 = U256 {
        l0: 0xF3B9CAC2FC632551,
        l1: 0xBCE6FAADA7179E84,
        l2: 0xFFFFFFFFFFFFFFFF,
        l3: 0xFFFFFFFF00000000,
    };
    const R_SQUARED: U256 = U256 {
        l0: 0x83244C95BE79EEA2,
        l1: 0x4699799C49BD6FA6,
        l2: 0x2845B2392B6BEC59,
        l3: 0x66E12D94F3D95620,
    };
    const INV: u64 = 0xCCD1C8AAEE00BC4F;
}

/// スカラー体の `(n - 1) / 4` です。
pub const SQRT_EXP_P256_ORDER: U256 = U256 {
    l0: 0x3CEE72B0BF18C954,
    l1: 0xEF39BEAB69C5E7A1,
    l2: 0x3FFFFFFFFFFFFFFF,
    l3: 0x3FFFFFFFC0000000,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fp::Fp;
    type Fp256 = Fp<P256Field>;
    #[test]
    fn modulus_correct() {
        let p = P256Field::MODULUS;
        assert_eq!(p.l0, 0xFFFFFFFFFFFFFFFF);
        assert_eq!(p.l1, 0x00000000FFFFFFFF);
        assert_eq!(p.l2, 0x0000000000000000);
        assert_eq!(p.l3, 0xFFFFFFFF00000001);
    }
    #[test]
    fn r_squared_correct() {
        assert_eq!(P256Field::R_SQUARED.l0, 0x0000000000000003);
        assert_eq!(P256Field::R_SQUARED.l1, 0xFFFFFFFBFFFFFFFF);
    }
    #[test]
    fn redc_one_gives_one() {
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
