use alloc::vec::Vec;

use neco_gf256::{Gf256, Poly};

use crate::bm::berlekamp_massey;
use crate::coefficient_position;
use crate::syndrome::calc_syndromes;
use crate::{CorrectionResult, ReedSolomon, RsError};

pub(crate) fn build_erasure_locator(positions: &[usize], n: usize) -> Poly {
    let mut locator = Poly::new(alloc::vec![Gf256::ONE]);

    for &position in positions {
        let factor = Poly::new(alloc::vec![
            Gf256::ONE,
            Gf256::exp(coefficient_position(position, n) as u8),
        ]);
        locator = locator.mul(&factor);
    }

    locator
}

pub(crate) fn forney_syndromes(
    syndromes: &[Gf256],
    _erasure_locator: &Poly,
    positions: &[usize],
    n: usize,
) -> Vec<Gf256> {
    let mut modified = syndromes.to_vec();

    for &position in positions {
        let root = Gf256::exp(coefficient_position(position, n) as u8);

        for index in 0..modified.len().saturating_sub(1) {
            modified[index] = (modified[index] * root) + modified[index + 1];
        }

        modified.pop();
    }

    modified
}

pub(crate) fn error_evaluator(syndromes: &[Gf256], locator: &Poly, nsym: usize) -> Poly {
    let product = Poly::new(syndromes.to_vec()).mul(locator);
    let limit = nsym.min(product.coeffs().len());

    Poly::new(product.coeffs()[..limit].to_vec())
}

pub(crate) fn find_errors(error_locator: &Poly, n: usize) -> Result<Vec<usize>, RsError> {
    let expected = error_locator.degree().unwrap_or(0);
    let mut positions = Vec::new();

    for index in 0..n {
        let root = Gf256::exp((255 - index) as u8);
        if error_locator.eval(root) == Gf256::ZERO {
            positions.push(n - 1 - index);
        }
    }

    if positions.len() != expected {
        return Err(RsError::TooManyErrors);
    }

    Ok(positions)
}

pub(crate) fn correct_errata(
    received: &mut [Gf256],
    syndromes: &[Gf256],
    positions: &[usize],
) -> Result<(), RsError> {
    let locator = build_erasure_locator(positions, received.len());
    let evaluator = error_evaluator(syndromes, &locator, syndromes.len());
    let mut magnitudes = alloc::vec![Gf256::ZERO; received.len()];

    for &position in positions {
        let coefficient = coefficient_position(position, received.len());
        let inverse = Gf256::exp((255 - coefficient) as u8);
        let mut denominator = Gf256::ONE;

        for &other in positions {
            if other == position {
                continue;
            }

            let other_x = Gf256::exp(coefficient_position(other, received.len()) as u8);
            denominator = denominator * (Gf256::ONE + (other_x * inverse));
        }

        if denominator == Gf256::ZERO {
            return Err(RsError::TooManyErrors);
        }

        let magnitude = evaluator.eval(inverse) / denominator;
        magnitudes[position] = magnitude;
    }

    for (index, value) in magnitudes.into_iter().enumerate() {
        received[index] = received[index] + value;
    }

    Ok(())
}

impl ReedSolomon {
    pub fn correct_erasures(
        &self,
        received: &mut [Gf256],
        positions: &[usize],
    ) -> Result<CorrectionResult, RsError> {
        self.correct(received, positions)
    }

    pub fn correct_errors(&self, received: &mut [Gf256]) -> Result<CorrectionResult, RsError> {
        self.correct(received, &[])
    }

    pub fn correct(
        &self,
        received: &mut [Gf256],
        erasure_positions: &[usize],
    ) -> Result<CorrectionResult, RsError> {
        if received.len() != self.n {
            return Err(RsError::InvalidParameters);
        }

        if erasure_positions.len() > self.parity_symbols() {
            return Err(RsError::TooManyErrors);
        }

        validate_positions(erasure_positions, self.n)?;

        let syndromes = calc_syndromes(received, self.parity_symbols());
        if syndromes.iter().all(|&syndrome| syndrome == Gf256::ZERO) {
            return Ok(CorrectionResult::default());
        }

        let erasure_locator = build_erasure_locator(erasure_positions, self.n);
        let modified_syndromes =
            forney_syndromes(&syndromes, &erasure_locator, erasure_positions, self.n);
        let error_locator = berlekamp_massey(&modified_syndromes);
        let error_positions = find_errors(&error_locator, self.n)?;

        let mut errata_positions = erasure_positions.to_vec();
        for position in error_positions {
            if !errata_positions.contains(&position) {
                errata_positions.push(position);
            }
        }

        let errors_corrected = errata_positions
            .len()
            .saturating_sub(erasure_positions.len());
        if (2 * errors_corrected) + erasure_positions.len() > self.parity_symbols() {
            return Err(RsError::TooManyErrors);
        }

        correct_errata(received, &syndromes, &errata_positions)?;

        let check = calc_syndromes(received, self.parity_symbols());
        if check.iter().any(|&syndrome| syndrome != Gf256::ZERO) {
            return Err(RsError::TooManyErrors);
        }

        Ok(CorrectionResult {
            errors_corrected,
            erasures_corrected: erasure_positions.len(),
        })
    }
}

fn validate_positions(positions: &[usize], n: usize) -> Result<(), RsError> {
    for (index, &position) in positions.iter().enumerate() {
        if position >= n {
            return Err(RsError::InvalidParameters);
        }

        if positions[index + 1..].contains(&position) {
            return Err(RsError::InvalidParameters);
        }
    }

    Ok(())
}
