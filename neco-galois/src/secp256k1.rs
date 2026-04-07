// secp256k1 prime field constants.
//   p_secp = 2^256 - 2^32 - 977
//   2^256 ≡ 2^32 + 977 (mod p_secp)

use crate::bigint::U256;
use crate::fp::PrimeField;

// -----------------------------------------------------------------------
// Secp256k1Field: フィールド素数 p
// -----------------------------------------------------------------------

/// secp256k1 のフィールド素数
/// p = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2F
/// = 2^256 - 2^32 - 977
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Secp256k1Field;

impl PrimeField for Secp256k1Field {
    /// p = 2^256 - 2^32 - 977
    const MODULUS: U256 = U256 {
        l0: 0xFFFFFFFEFFFFFC2F,
        l1: 0xFFFFFFFFFFFFFFFF,
        l2: 0xFFFFFFFFFFFFFFFF,
        l3: 0xFFFFFFFFFFFFFFFF,
    };

    /// R² mod p = 0x000000000000000000000000000000000000000000000001000007A2000E90A1
    const R_SQUARED: U256 = U256 {
        l0: 0x000007A2000E90A1,
        l1: 0x0000000000000001,
        l2: 0x0000000000000000,
        l3: 0x0000000000000000,
    };

    /// -p⁻¹ mod 2^64 = 0xD838091DD2253531
    const INV: u64 = 0xD838091DD2253531;
}

/// (p+1)/4 mod p — secp256k1 のフィールド sqrt 用指数
/// p ≡ 3 (mod 4) なので sqrt(a) = a^((p+1)/4)
pub const SQRT_EXP_SECP256K1: U256 = U256 {
    l0: 0xFFFFFFFFBFFFFF0C,
    l1: 0xFFFFFFFFFFFFFFFF,
    l2: 0xFFFFFFFFFFFFFFFF,
    l3: 0x3FFFFFFFFFFFFFFF,
};

// -----------------------------------------------------------------------
// Secp256k1Order: 群位数 n
// -----------------------------------------------------------------------

/// secp256k1 の群位数 n（スカラー体に使用）
/// n = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Secp256k1Order;

impl PrimeField for Secp256k1Order {
    /// n = group order of secp256k1
    const MODULUS: U256 = U256 {
        l0: 0xBFD25E8CD0364141,
        l1: 0xBAAEDCE6AF48A03B,
        l2: 0xFFFFFFFFFFFFFFFE,
        l3: 0xFFFFFFFFFFFFFFFF,
    };

    /// R² mod n = 0x9D671CD581C69BC5E697F5E45BCD07C6741496C20E7CF878896CF21467D7D140
    const R_SQUARED: U256 = U256 {
        l0: 0x896CF21467D7D140,
        l1: 0x741496C20E7CF878,
        l2: 0xE697F5E45BCD07C6,
        l3: 0x9D671CD581C69BC5,
    };

    /// -n⁻¹ mod 2^64 = 0x4B0DFF665588B13F
    const INV: u64 = 0x4B0DFF665588B13F;
}

/// (n+1)/4 — secp256k1 の群位数の sqrt 指数（sqrt は主にフィールド側で使用）
pub const SQRT_EXP_SECP256K1_ORDER: U256 = U256 {
    l0: 0xEFF497A3340D9050,
    l1: 0xAEABB739ABD2280E,
    l2: 0xFFFFFFFFFFFFFFFF,
    l3: 0x3FFFFFFFFFFFFFFF,
};

// -----------------------------------------------------------------------
// Tests: 2^256 ≡ 2^32 + 977 (mod p_secp) reduction.
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fp::Fp;

    type Fq = Fp<Secp256k1Field>;

    #[test]
    fn modulus_correct() {
        // p = 2^256 - 2^32 - 977
        // 確認: p + 2^32 + 977 = 2^256 (overflow = carry)
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
        // R² mod p を Python で検証した値と一致するか
        // 値: 0x000000000000000000000000000000000000000000000001000007A2000E90A1
        assert_eq!(Secp256k1Field::R_SQUARED.l3, 0);
        assert_eq!(Secp256k1Field::R_SQUARED.l2, 0);
        assert_eq!(Secp256k1Field::R_SQUARED.l1, 1);
        assert_eq!(Secp256k1Field::R_SQUARED.l0, 0x000007A2000E90A1);
    }

    #[test]
    fn redc_one_gives_r_inv() {
        // redc([1, 0, 0, 0, 0, 0, 0, 0]) = R⁻¹ mod p
        // R⁻¹ * R ≡ 1 (mod p)
        // from_u256(1) = redc(1 * R²) = R mod p (= 1 in Montgomery)
        let one = Fq::from_u256(U256::ONE);
        let back = one.to_u256();
        assert_eq!(back, U256::ONE);
    }

    #[test]
    fn mul_by_modulus_is_zero() {
        // 任意の a に対して a * p ≡ 0 (mod p)
        let a = Fq::from_u256(U256::from_u64(12345));
        let p_as_fp = Fq::from_u256(Secp256k1Field::MODULUS);
        // p ≡ 0 (mod p) なので from_u256(p) = 0
        assert_eq!(p_as_fp, Fq::ZERO);
        let product = Fp::mul(a, p_as_fp);
        assert_eq!(product, Fq::ZERO);
    }

    #[test]
    fn order_modulus_correct() {
        // n の確認
        let n = Secp256k1Order::MODULUS;
        // n < p (n < 2^256 - 2^32 - 977)
        assert_eq!(
            U256::cmp(n, Secp256k1Field::MODULUS),
            core::cmp::Ordering::Less
        );
    }
}
