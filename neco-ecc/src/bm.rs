use neco_gf256::{Gf256, Poly};

pub(crate) fn berlekamp_massey(syndromes: &[Gf256]) -> Poly {
    let mut current = alloc::vec![Gf256::ONE];
    let mut previous = alloc::vec![Gf256::ONE];
    let mut current_degree = 0usize;
    let mut shift = 1usize;
    let mut last_discrepancy = Gf256::ONE;

    for (step, &syndrome) in syndromes.iter().enumerate() {
        let mut discrepancy = syndrome;

        for index in 1..=current_degree {
            discrepancy = discrepancy + (current[index] * syndromes[step - index]);
        }

        if discrepancy == Gf256::ZERO {
            shift += 1;
            continue;
        }

        let scale = discrepancy / last_discrepancy;
        let mut candidate = current.clone();

        if candidate.len() < previous.len() + shift {
            candidate.resize(previous.len() + shift, Gf256::ZERO);
        }

        for (index, &coeff) in previous.iter().enumerate() {
            candidate[index + shift] = candidate[index + shift] + (scale * coeff);
        }

        if (step + 1) >= (2 * current_degree + 1) {
            previous = current;
            current = candidate;
            current_degree = step + 1 - current_degree;
            last_discrepancy = discrepancy;
            shift = 1;
        } else {
            current = candidate;
            shift += 1;
        }
    }

    Poly::new(current)
}
