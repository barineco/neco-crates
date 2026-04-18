use alloc::vec::Vec;

use neco_gf256::Gf256;

use crate::{poly_from_codeword, ReedSolomon, RsError};

impl ReedSolomon {
    pub fn encode(&self, data: &[Gf256]) -> Result<Vec<Gf256>, RsError> {
        if data.len() > self.k {
            return Err(RsError::DataTooLong);
        }

        if data.len() != self.k {
            return Err(RsError::InvalidParameters);
        }

        let mut message = alloc::vec![Gf256::ZERO; self.parity_symbols()];
        message.extend(data.iter().rev().copied());

        let (_, remainder) = poly_from_codeword(&message.iter().rev().copied().collect::<Vec<_>>())
            .div_rem(&self.generator);

        for (index, &coeff) in remainder.coeffs().iter().enumerate() {
            message[index] = message[index] + coeff;
        }

        message.reverse();
        Ok(message)
    }
}
