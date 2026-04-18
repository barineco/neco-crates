use alloc::vec::Vec;

use crate::Gf256;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Poly {
    coeffs: Vec<Gf256>,
}

impl Poly {
    pub fn new(mut coeffs: Vec<Gf256>) -> Self {
        while coeffs.last() == Some(&Gf256::ZERO) {
            coeffs.pop();
        }

        Self { coeffs }
    }

    pub fn eval(&self, x: Gf256) -> Gf256 {
        let mut result = Gf256::ZERO;
        let mut index = self.coeffs.len();

        while index > 0 {
            index -= 1;
            result = (result * x) + self.coeffs[index];
        }

        result
    }

    pub fn mul(&self, rhs: &Self) -> Self {
        if self.degree().is_none() || rhs.degree().is_none() {
            return Self::new(Vec::new());
        }

        let mut coeffs = alloc::vec![Gf256::ZERO; self.coeffs.len() + rhs.coeffs.len() - 1];

        for (lhs_index, &lhs_coeff) in self.coeffs.iter().enumerate() {
            for (rhs_index, &rhs_coeff) in rhs.coeffs.iter().enumerate() {
                coeffs[lhs_index + rhs_index] =
                    coeffs[lhs_index + rhs_index] + (lhs_coeff * rhs_coeff);
            }
        }

        Self::new(coeffs)
    }

    pub fn add(&self, rhs: &Self) -> Self {
        let len = self.coeffs.len().max(rhs.coeffs.len());
        let mut coeffs = Vec::with_capacity(len);

        for index in 0..len {
            let lhs = self.coeffs.get(index).copied().unwrap_or(Gf256::ZERO);
            let rhs = rhs.coeffs.get(index).copied().unwrap_or(Gf256::ZERO);
            coeffs.push(lhs + rhs);
        }

        Self::new(coeffs)
    }

    pub fn div_rem(&self, rhs: &Self) -> (Self, Self) {
        let rhs_degree = rhs.degree().expect("cannot divide by zero polynomial");
        let rhs_leading = rhs.coeffs[rhs_degree];

        if self.degree().is_none() {
            return (Self::new(Vec::new()), Self::new(Vec::new()));
        }

        let mut remainder = self.coeffs.clone();
        let mut quotient =
            alloc::vec![Gf256::ZERO; self.coeffs.len().saturating_sub(rhs.coeffs.len()) + 1];

        while let Some(remainder_degree) = degree_of(&remainder) {
            if remainder_degree < rhs_degree {
                break;
            }

            let shift = remainder_degree - rhs_degree;
            let factor = remainder[remainder_degree] / rhs_leading;
            quotient[shift] = quotient[shift] + factor;

            for (rhs_index, &rhs_coeff) in rhs.coeffs.iter().enumerate() {
                let target = shift + rhs_index;
                remainder[target] = remainder[target] + (factor * rhs_coeff);
            }

            while remainder.last() == Some(&Gf256::ZERO) {
                remainder.pop();
            }
        }

        (Self::new(quotient), Self::new(remainder))
    }

    pub fn degree(&self) -> Option<usize> {
        degree_of(&self.coeffs)
    }

    pub fn scale(&self, factor: Gf256) -> Self {
        if factor == Gf256::ZERO {
            return Self::new(Vec::new());
        }

        let mut coeffs = Vec::with_capacity(self.coeffs.len());
        for &coeff in &self.coeffs {
            coeffs.push(coeff * factor);
        }

        Self::new(coeffs)
    }

    pub fn coeffs(&self) -> &[Gf256] {
        &self.coeffs
    }
}

fn degree_of(coeffs: &[Gf256]) -> Option<usize> {
    coeffs.iter().rposition(|&coeff| coeff != Gf256::ZERO)
}
