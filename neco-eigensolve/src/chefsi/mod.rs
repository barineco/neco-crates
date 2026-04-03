pub mod cpu_backend;
pub mod filter;
pub mod rayleigh_ritz;
pub mod spectrum;

use neco_sparse::CsrMat;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::rng::Rng;

const RANDOM_SUBSPACE_BASE_SEED: u64 = 0x4348_4546_5349;
static NEXT_RANDOM_SUBSPACE_SEED: AtomicU64 = AtomicU64::new(RANDOM_SUBSPACE_BASE_SEED);

/// Random initial subspace with an explicit deterministic seed.
pub fn random_subspace_with_seed(n: usize, m: usize, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    (0..n * m).map(|_| (rng.next_f64() as f32) - 0.5).collect()
}

/// Random initial subspace stored column-major as `f32`.
pub fn random_subspace(n: usize, m: usize) -> Vec<f32> {
    let seed = NEXT_RANDOM_SUBSPACE_SEED.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed);
    random_subspace_with_seed(n, m, seed)
}

/// Lumped mass via row-sum lumping.
pub fn lump_mass(m_mat: &CsrMat<f64>) -> Vec<f64> {
    (0..m_mat.nrows())
        .map(|i| m_mat.row(i).values().iter().sum::<f64>())
        .collect()
}

/// Choose a Chebyshev filter degree from the cutoff ratio.
pub fn adaptive_degree(lambda_cutoff: f64, lambda_max: f64) -> usize {
    if lambda_max <= lambda_cutoff {
        return 10;
    }
    let delta = 2.0 * lambda_cutoff / (lambda_max - lambda_cutoff);
    let degree = if delta > 0.01 {
        (7.0 / (2.0 * delta).sqrt()).ceil() as usize
    } else {
        200
    };
    degree.clamp(10, 200)
}

/// Remove duplicated overlap modes by keeping the lower-residual copy.
pub fn deduplicate_overlap_modes(freqs: &mut Vec<f64>, residuals: &mut Vec<f64>) {
    if freqs.len() < 2 {
        return;
    }

    let mut remove = vec![false; freqs.len()];
    for i in 1..freqs.len() {
        if remove[i - 1] {
            continue;
        }
        let rel_diff = (freqs[i] - freqs[i - 1]).abs() / freqs[i].max(1e-12);
        if rel_diff < 1e-4 {
            if residuals[i] > residuals[i - 1] {
                remove[i] = true;
            } else {
                remove[i - 1] = true;
            }
        }
    }

    let mut write = 0;
    for read in 0..freqs.len() {
        if !remove[read] {
            freqs[write] = freqs[read];
            residuals[write] = residuals[read];
            write += 1;
        }
    }
    freqs.truncate(write);
    residuals.truncate(write);
}

#[cfg(test)]
mod tests {
    use super::*;
    use neco_sparse::CsrMat;

    #[test]
    fn lump_mass_preserves_total_mass() {
        let offsets = vec![0, 3, 6, 9];
        let indices = vec![0, 1, 2, 0, 1, 2, 0, 1, 2];
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
        let m = CsrMat::try_from_csr_data(3, 3, offsets, indices, values).unwrap();

        let lumped = lump_mass(&m);
        let total_consistent: f64 = m.triplet_iter().map(|(_, _, value)| value).sum();
        let total_lumped: f64 = lumped.iter().sum();

        assert_eq!(lumped, vec![6.0, 15.0, 24.0]);
        assert!((total_consistent - total_lumped).abs() < 1e-12);
    }

    #[test]
    fn random_subspace_shape_matches_request() {
        let subspace = random_subspace(7, 3);
        assert_eq!(subspace.len(), 21);
        assert!(subspace.iter().any(|value| value.abs() > 1e-6));
    }

    #[test]
    fn random_subspace_with_seed_is_reproducible() {
        let a = random_subspace_with_seed(5, 2, 123);
        let b = random_subspace_with_seed(5, 2, 123);
        assert_eq!(a, b);
    }

    #[test]
    fn random_subspace_uses_distinct_seeds_per_call() {
        let a = random_subspace(5, 2);
        let b = random_subspace(5, 2);
        assert_ne!(a, b);
    }

    #[test]
    fn deduplicate_overlap_modes_keeps_lower_residual_copy() {
        let mut freqs = vec![100.0, 100.000001, 200.0];
        let mut residuals = vec![1e-4, 1e-6, 1e-5];
        deduplicate_overlap_modes(&mut freqs, &mut residuals);

        assert_eq!(freqs, vec![100.000001, 200.0]);
        assert_eq!(residuals, vec![1e-6, 1e-5]);
    }
}
