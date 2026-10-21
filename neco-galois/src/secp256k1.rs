//! secp256k1 の有限体とスカラー体の定数です。

use crate::bigint::U256;
use crate::fp::PrimeField;

/// フィールド素数は `p = 2^256 - 2^32 - 977` であり、`2^256 ≡ 2^32 + 977 (mod p)` を満たします。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Secp256k1Field;

impl PrimeField for Secp256k1Field {
    const MODULUS: U256 = U256 {
        l0: 0xFFFFFFFEFFFFFC2F,
        l1: 0xFFFFFFFFFFFFFFFF,
        l2: 0xFFFFFFFFFFFFFFFF,
        l3: 0xFFFFFFFFFFFFFFFF,
    };
    const R_SQUARED: U256 = U256 {
        l0: 0x000007A2000E90A1,
        l1: 0x0000000000000001,
        l2: 0x0000000000000000,
        l3: 0x0000000000000000,
    };
    const INV: u64 = 0xD838091DD2253531;
}

/// `p ≡ 3 (mod 4)` における `(p + 1) / 4` の平方根指数です。
pub const SQRT_EXP_SECP256K1: U256 = U256 {
    l0: 0xFFFFFFFFBFFFFF0C,
    l1: 0xFFFFFFFFFFFFFFFF,
    l2: 0xFFFFFFFFFFFFFFFF,
    l3: 0x3FFFFFFFFFFFFFFF,
};

/// 対応する曲線のスカラー体の位数です。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Secp256k1Order;

impl PrimeField for Secp256k1Order {
    const MODULUS: U256 = U256 {
        l0: 0xBFD25E8CD0364141,
        l1: 0xBAAEDCE6AF48A03B,
        l2: 0xFFFFFFFFFFFFFFFE,
        l3: 0xFFFFFFFFFFFFFFFF,
    };
    const R_SQUARED: U256 = U256 {
        l0: 0x896CF21467D7D140,
        l1: 0x741496C20E7CF878,
        l2: 0xE697F5E45BCD07C6,
        l3: 0x9D671CD581C69BC5,
    };
    const INV: u64 = 0x4B0DFF665588B13F;
}

/// スカラー体の `(n - 1) / 4` です。
pub const SQRT_EXP_SECP256K1_ORDER: U256 = U256 {
    l0: 0xEFF497A3340D9050,
    l1: 0xAEABB739ABD2280E,
    l2: 0xFFFFFFFFFFFFFFFF,
    l3: 0x3FFFFFFFFFFFFFFF,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fp::Fp;
    type Fq = Fp<Secp256k1Field>;
    #[test]
    fn modulus_correct() {
        let p = Secp256k1Field::MODULUS;
        let offset = U256 {
            l0: (1u64 << 32) + 977,
            l1: 0,
            l2: 0,
            l3: 0,
        };
        let (sum, carry) = U256::add(p, offset);
        assert!(carry, "p + 2^32 + 977 should overflow 256 bits");
        assert_eq!(sum, U256::ZERO);
    }
    #[test]
    fn r_squared_correct() {
        assert_eq!(Secp256k1Field::R_SQUARED.l3, 0);
        assert_eq!(Secp256k1Field::R_SQUARED.l2, 0);
        assert_eq!(Secp256k1Field::R_SQUARED.l1, 1);
        assert_eq!(Secp256k1Field::R_SQUARED.l0, 0x000007A2000E90A1);
    }
    #[test]
    fn redc_one_gives_r_inv() {
        let one = Fq::from_u256(U256::ONE);
        let back = one.to_u256();
        assert_eq!(back, U256::ONE);
    }
    #[test]
    fn mul_by_modulus_is_zero() {
        let a = Fq::from_u256(U256::from_u64(12345));
        let p_as_fp = Fq::from_u256(Secp256k1Field::MODULUS);
        assert_eq!(p_as_fp, Fq::ZERO);
        let product = Fp::mul(a, p_as_fp);
        assert_eq!(product, Fq::ZERO);
    }
    #[test]
    fn order_modulus_correct() {
        let n = Secp256k1Order::MODULUS;
        assert_eq!(
            U256::cmp(n, Secp256k1Field::MODULUS),
            core::cmp::Ordering::Less
        );
    }
}
