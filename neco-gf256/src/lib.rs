//! Zero dependency GF(2^8) finite field arithmetic.

#![no_std]

extern crate alloc;

mod poly;
mod tables;

use core::ops::{Add, Div, Mul, Sub};

pub use poly::Poly;

#[cfg(test)]
extern crate std;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Gf256(pub u8);

impl Gf256 {
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(1);

    #[inline]
    pub const fn add(self, rhs: Self) -> Self {
        Self(self.0 ^ rhs.0)
    }

    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn mul(self, rhs: Self) -> Self {
        if self.0 == 0 || rhs.0 == 0 {
            return Self::ZERO;
        }

        let lhs_log = tables::LOG_TABLE[self.0 as usize] as usize;
        let rhs_log = tables::LOG_TABLE[rhs.0 as usize] as usize;

        Self(tables::EXP_TABLE[lhs_log + rhs_log])
    }

    pub fn inv(self) -> Self {
        assert!(self.0 != 0, "zero has no multiplicative inverse");

        let log = tables::LOG_TABLE[self.0 as usize] as usize;
        Self(tables::EXP_TABLE[255 - log])
    }

    pub fn pow(self, exp: u8) -> Self {
        if exp == 0 {
            return Self::ONE;
        }

        if self.0 == 0 {
            return Self::ZERO;
        }

        let log = tables::LOG_TABLE[self.0 as usize] as usize;
        let index = (log * exp as usize) % 255;

        Self(tables::EXP_TABLE[index])
    }

    pub fn log(self) -> u8 {
        assert!(self.0 != 0, "zero has no discrete logarithm");
        tables::LOG_TABLE[self.0 as usize]
    }

    pub fn exp(exp: u8) -> Self {
        Self(tables::EXP_TABLE[exp as usize])
    }
}

impl Add for Gf256 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        self.add(rhs)
    }
}

impl Sub for Gf256 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        self.add(rhs)
    }
}

impl Mul for Gf256 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        self.mul(rhs)
    }
}

impl Div for Gf256 {
    type Output = Self;

    #[allow(clippy::suspicious_arithmetic_impl)]
    fn div(self, rhs: Self) -> Self::Output {
        self * rhs.inv()
    }
}

#[cfg(test)]
mod tests {
    use super::{Gf256, Poly};

    #[test]
    fn zero_pow_zero_returns_one() {
        assert_eq!(Gf256::ZERO.pow(0), Gf256::ONE);
    }

    #[test]
    fn exp_log_roundtrip_holds_for_nonzero_elements() {
        for value in 1u8..=255 {
            let element = Gf256(value);
            assert_eq!(Gf256::exp(element.log()), element);
        }
    }

    #[test]
    fn field_axioms_hold_for_all_nonzero_elements() {
        let all_nonzero: alloc::vec::Vec<Gf256> = (1u8..=255).map(Gf256).collect();

        for &a in &all_nonzero {
            assert_eq!(a + Gf256::ZERO, a);
            assert_eq!(a * Gf256::ONE, a);
            assert_eq!(a / a, Gf256::ONE);
            assert_eq!(a * a.inv(), Gf256::ONE);

            for &b in &all_nonzero {
                assert_eq!(a + b, b + a);
                assert_eq!(a * b, b * a);
                assert_eq!(a - b, a + b);

                for &c in &all_nonzero {
                    assert_eq!((a + b) + c, a + (b + c));
                    assert_eq!((a * b) * c, a * (b * c));
                    assert_eq!(a * (b + c), (a * b) + (a * c));
                }
            }
        }
    }

    #[test]
    fn poly_eval_matches_manual_calculation() {
        let poly = Poly::new(alloc::vec![Gf256(7), Gf256(3), Gf256(5)]);
        let x = Gf256(11);
        let expected = Gf256(7) + (Gf256(3) * x) + (Gf256(5) * x * x);

        assert_eq!(poly.eval(x), expected);
    }

    #[test]
    fn poly_add_is_self_inverse() {
        let poly = Poly::new(alloc::vec![Gf256(1), Gf256(2), Gf256(3), Gf256(4)]);

        assert_eq!(poly.add(&poly), Poly::new(alloc::vec![]));
    }

    #[test]
    fn zero_polynomial_has_no_degree() {
        assert_eq!(
            Poly::new(alloc::vec![Gf256::ZERO, Gf256::ZERO]).degree(),
            None
        );
    }

    #[test]
    fn poly_mul_and_div_rem_roundtrip() {
        let a = Poly::new(alloc::vec![Gf256(9), Gf256(0), Gf256(4), Gf256(2)]);
        let b = Poly::new(alloc::vec![Gf256(3), Gf256(1), Gf256(5)]);
        let product = a.mul(&b);
        let (quotient, remainder) = product.div_rem(&b);

        assert_eq!(quotient, a);
        assert_eq!(remainder, Poly::new(alloc::vec![]));
        assert_eq!(quotient.mul(&b).add(&remainder), product);
    }
}
