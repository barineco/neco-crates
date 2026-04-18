//! Reed-Solomon error correction over GF(2^8).

#![no_std]

extern crate alloc;

mod bm;
mod decoder;
mod encoder;
mod syndrome;

use alloc::vec::Vec;
use neco_gf256::Poly;
use syndrome::calc_syndromes;

#[cfg(test)]
extern crate std;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RsError {
    InvalidParameters,
    TooManyErrors,
    DataTooLong,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CorrectionResult {
    pub errors_corrected: usize,
    pub erasures_corrected: usize,
}

#[derive(Clone, Debug)]
pub struct ReedSolomon {
    n: usize,
    k: usize,
    generator: Poly,
}

impl ReedSolomon {
    pub fn new(n: usize, k: usize) -> Result<Self, RsError> {
        if n > 255 || k == 0 || k >= n {
            return Err(RsError::InvalidParameters);
        }

        Ok(Self {
            n,
            k,
            generator: generator_poly(n - k),
        })
    }

    pub fn n(&self) -> usize {
        self.n
    }

    pub fn k(&self) -> usize {
        self.k
    }

    pub fn parity_symbols(&self) -> usize {
        self.n - self.k
    }

    pub fn calc_syndromes(&self, received: &[neco_gf256::Gf256]) -> Vec<neco_gf256::Gf256> {
        calc_syndromes(received, self.parity_symbols())
    }
}

pub(crate) fn generator_poly(nsym: usize) -> Poly {
    let mut generator = Poly::new(alloc::vec![neco_gf256::Gf256::ONE]);

    for power in 0..nsym {
        let factor = Poly::new(alloc::vec![
            neco_gf256::Gf256::exp(power as u8),
            neco_gf256::Gf256::ONE,
        ]);
        generator = generator.mul(&factor);
    }

    generator
}

pub(crate) fn poly_from_codeword(codeword: &[neco_gf256::Gf256]) -> Poly {
    Poly::new(codeword.iter().rev().copied().collect())
}

pub(crate) fn coefficient_position(index: usize, n: usize) -> usize {
    n - 1 - index
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use neco_gf256::Gf256;

    use super::{CorrectionResult, ReedSolomon, RsError};

    fn gf_slice(bytes: &[u8]) -> Vec<Gf256> {
        bytes.iter().copied().map(Gf256).collect()
    }

    fn assert_decoded_prefix(codeword: &[Gf256], expected_data: &[Gf256], k: usize) {
        assert_eq!(&codeword[..k], expected_data);
    }

    #[test]
    fn encoded_codeword_has_zero_syndromes() {
        let rs = ReedSolomon::new(15, 11).unwrap();
        let data = gf_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
        let codeword = rs.encode(&data).unwrap();

        assert!(rs
            .calc_syndromes(&codeword)
            .iter()
            .all(|&s| s == Gf256::ZERO));
    }

    #[test]
    fn corrupted_symbol_produces_nonzero_syndrome() {
        let rs = ReedSolomon::new(15, 11).unwrap();
        let data = gf_slice(&[3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5]);
        let mut codeword = rs.encode(&data).unwrap();
        codeword[4] = codeword[4] + Gf256(99);

        assert!(rs
            .calc_syndromes(&codeword)
            .iter()
            .any(|&s| s != Gf256::ZERO));
    }

    #[test]
    fn single_erasure_is_corrected() {
        let rs = ReedSolomon::new(15, 11).unwrap();
        let data = gf_slice(&[1, 3, 3, 7, 0, 2, 4, 8, 1, 9, 6]);
        let mut codeword = rs.encode(&data).unwrap();
        codeword[2] = Gf256::ZERO;

        let result = rs.correct_erasures(&mut codeword, &[2]).unwrap();

        assert_eq!(
            result,
            CorrectionResult {
                errors_corrected: 0,
                erasures_corrected: 1,
            }
        );
        assert_decoded_prefix(&codeword, &data, rs.k());
    }

    #[test]
    fn maximum_erasures_are_corrected() {
        let rs = ReedSolomon::new(15, 11).unwrap();
        let data = gf_slice(&[2, 7, 1, 8, 2, 8, 1, 8, 2, 8, 4]);
        let mut codeword = rs.encode(&data).unwrap();

        for &index in &[0usize, 3, 8, 12] {
            codeword[index] = Gf256::ZERO;
        }

        let result = rs.correct_erasures(&mut codeword, &[0, 3, 8, 12]).unwrap();

        assert_eq!(result.erasures_corrected, 4);
        assert_decoded_prefix(&codeword, &data, rs.k());
    }

    #[test]
    fn single_error_is_corrected() {
        let rs = ReedSolomon::new(15, 11).unwrap();
        let data = gf_slice(&[8, 6, 7, 5, 3, 0, 9, 1, 2, 4, 6]);
        let mut codeword = rs.encode(&data).unwrap();
        codeword[6] = codeword[6] + Gf256(55);

        let result = rs.correct_errors(&mut codeword).unwrap();

        assert_eq!(result.errors_corrected, 1);
        assert_decoded_prefix(&codeword, &data, rs.k());
    }

    #[test]
    fn maximum_errors_are_corrected() {
        let rs = ReedSolomon::new(15, 9).unwrap();
        let data = gf_slice(&[1, 1, 2, 3, 5, 8, 13, 21, 34]);
        let mut codeword = rs.encode(&data).unwrap();

        for (index, delta) in [(0usize, 17u8), (4, 33), (7, 99)].iter().copied() {
            codeword[index] = codeword[index] + Gf256(delta);
        }

        let result = rs.correct_errors(&mut codeword).unwrap();

        assert_eq!(result.errors_corrected, 3);
        assert_decoded_prefix(&codeword, &data, rs.k());
    }

    #[test]
    fn mixed_erasures_and_errors_are_corrected() {
        let rs = ReedSolomon::new(15, 9).unwrap();
        let data = gf_slice(&[9, 8, 7, 6, 5, 4, 3, 2, 1]);
        let mut codeword = rs.encode(&data).unwrap();

        codeword[1] = Gf256::ZERO;
        codeword[5] = Gf256::ZERO;
        codeword[11] = codeword[11] + Gf256(120);
        codeword[13] = codeword[13] + Gf256(77);

        let result = rs.correct(&mut codeword, &[1, 5]).unwrap();

        assert_eq!(result.erasures_corrected, 2);
        assert_eq!(result.errors_corrected, 2);
        assert_decoded_prefix(&codeword, &data, rs.k());
    }

    #[test]
    fn excessive_damage_returns_too_many_errors() {
        let rs = ReedSolomon::new(15, 11).unwrap();
        let data = gf_slice(&[4, 2, 4, 2, 4, 2, 4, 2, 4, 2, 4]);
        let mut codeword = rs.encode(&data).unwrap();

        codeword[0] = Gf256::ZERO;
        codeword[3] = Gf256::ZERO;
        codeword[5] = codeword[5] + Gf256(1);
        codeword[7] = codeword[7] + Gf256(2);

        let error = rs.correct(&mut codeword, &[0, 3]).unwrap_err();

        assert_eq!(error, RsError::TooManyErrors);
    }

    #[test]
    fn rs_255_223_corrects_standard_single_error() {
        let rs = ReedSolomon::new(255, 223).unwrap();
        let data: Vec<Gf256> = (0u8..223).map(Gf256).collect();
        let mut codeword = rs.encode(&data).unwrap();
        codeword[200] = codeword[200] + Gf256(0x53);

        let result = rs.correct_errors(&mut codeword).unwrap();

        assert_eq!(result.errors_corrected, 1);
        assert_decoded_prefix(&codeword, &data, rs.k());
    }

    #[test]
    fn rs_7_3_matches_manual_parity_and_recovers() {
        let rs = ReedSolomon::new(7, 3).unwrap();
        let data = gf_slice(&[1, 2, 3]);
        let codeword = rs.encode(&data).unwrap();

        assert_eq!(codeword, gf_slice(&[1, 2, 3, 158, 237, 54, 69]));

        let mut damaged = codeword.clone();
        damaged[4] = damaged[4] + Gf256(5);

        let result = rs.correct_errors(&mut damaged).unwrap();

        assert_eq!(result.errors_corrected, 1);
        assert_eq!(damaged, codeword);
    }

    #[test]
    fn rejects_invalid_parameters() {
        assert!(matches!(
            ReedSolomon::new(256, 223),
            Err(RsError::InvalidParameters)
        ));
        assert!(matches!(
            ReedSolomon::new(10, 10),
            Err(RsError::InvalidParameters)
        ));
    }
}
