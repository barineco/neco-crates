use alloc::vec::Vec;

use neco_gf256::Gf256;

use crate::poly_from_codeword;

pub(crate) fn calc_syndromes(received: &[Gf256], nsym: usize) -> Vec<Gf256> {
    let polynomial = poly_from_codeword(received);
    let mut syndromes = Vec::with_capacity(nsym);

    for power in 0..nsym {
        syndromes.push(polynomial.eval(Gf256::exp(power as u8)));
    }

    syndromes
}
