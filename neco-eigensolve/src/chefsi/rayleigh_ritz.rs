use neco_sparse::CsrMat;

use crate::dense::{symmetric_dense_kernel, SymmetricEigenResult};

/// Rayleigh-Ritz output.
pub struct RayleighRitzResult {
    pub eigenvalues: Vec<f64>,
    pub ritz_vectors: Vec<f32>,
    pub residuals: Vec<f64>,
    pub n_converged: usize,
}

#[inline]
fn csr_matvec_f64(y: &mut [f64], a: &CsrMat<f64>, x: &[f64]) {
    let offsets = a.row_offsets();
    let cols = a.col_indices();
    let vals = a.values();
    for row in 0..a.nrows() {
        let start = offsets[row];
        let end = offsets[row + 1];
        let mut sum = 0.0;
        for idx in start..end {
            sum += vals[idx] * x[cols[idx]];
        }
        y[row] = sum;
    }
}

/// M-inner-product orthogonalization via CholQR.
pub fn cholqr_m_orthogonalize(y: &mut [f32], m_diag: &[f64], n: usize, m: usize) -> bool {
    assert_eq!(y.len(), n * m);
    assert_eq!(m_diag.len(), n);

    let mut gram = vec![0.0; m * m];
    for i in 0..m {
        for j in i..m {
            let mut dot = 0.0;
            for k in 0..n {
                dot += y[k + i * n] as f64 * m_diag[k] * y[k + j * n] as f64;
            }
            gram[i * m + j] = dot;
            gram[j * m + i] = dot;
        }
    }

    let Some(r) = symmetric_dense_kernel().cholesky_upper(&gram, m) else {
        return false;
    };
    let mut q_buf = vec![0.0f64; n * m];

    for j in 0..m {
        let mut coeffs = vec![0.0f64; m];
        coeffs[j] = 1.0;
        for i in (0..m).rev() {
            let mut sum = coeffs[i];
            for p in (i + 1)..m {
                sum -= r[i * m + p] * coeffs[p];
            }
            coeffs[i] = sum / r[i * m + i];
        }

        for k in 0..n {
            let mut value = 0.0;
            for p in 0..m {
                value += y[k + p * n] as f64 * coeffs[p];
            }
            q_buf[k + j * n] = value;
        }
    }

    for (dst, &src) in y.iter_mut().zip(&q_buf) {
        *dst = src as f32;
    }
    true
}

/// Extract Ritz pairs from a filtered subspace.
pub fn rayleigh_ritz(
    k_mat: &CsrMat<f64>,
    m_diag: &[f64],
    y: &mut [f32],
    n: usize,
    m: usize,
) -> Result<RayleighRitzResult, String> {
    assert_eq!(y.len(), n * m);

    if !cholqr_m_orthogonalize(y, m_diag, n, m) && !cholqr_m_orthogonalize(y, m_diag, n, m) {
        return Err("CholQR2 failed: Gram matrix is not positive definite".into());
    }

    let mut h = vec![0.0; m * m];
    let mut tmp = vec![0.0f64; n];
    for j in 0..m {
        let q_j: Vec<f64> = (0..n).map(|k| y[k + j * n] as f64).collect();
        csr_matvec_f64(&mut tmp, k_mat, &q_j);
        for i in 0..m {
            let mut dot = 0.0;
            for k in 0..n {
                dot += y[k + i * n] as f64 * tmp[k];
            }
            h[i * m + j] = dot;
        }
    }

    for i in 0..m {
        for j in (i + 1)..m {
            let avg = (h[i * m + j] + h[j * m + i]) / 2.0;
            h[i * m + j] = avg;
            h[j * m + i] = avg;
        }
    }

    let SymmetricEigenResult {
        eigenvalues: eigenvalues_unsorted,
        eigenvectors: eigvecs,
    } = symmetric_dense_kernel().symmetric_eigen(&h, m);

    let mut indices: Vec<usize> = (0..m).collect();
    indices.sort_by(|&a, &b| eigenvalues_unsorted[a].total_cmp(&eigenvalues_unsorted[b]));
    let eigenvalues: Vec<f64> = indices
        .iter()
        .map(|&index| eigenvalues_unsorted[index])
        .collect();

    let mut ritz_vectors = vec![0.0f32; n * m];
    for (out_j, &orig_j) in indices.iter().enumerate() {
        for k in 0..n {
            let mut value = 0.0;
            for p in 0..m {
                value += y[k + p * n] as f64 * eigvecs[p * m + orig_j];
            }
            ritz_vectors[k + out_j * n] = value as f32;
        }
    }

    let mut residuals = Vec::with_capacity(m);
    let mut kpsi = vec![0.0f64; n];
    for j in 0..m {
        let psi: Vec<f64> = (0..n).map(|k| ritz_vectors[k + j * n] as f64).collect();
        csr_matvec_f64(&mut kpsi, k_mat, &psi);
        let theta = eigenvalues[j];
        let r_norm_sq = (0..n)
            .map(|k| {
                let residual = kpsi[k] - theta * m_diag[k] * psi[k];
                residual * residual
            })
            .sum::<f64>();
        let r_norm = r_norm_sq.sqrt();
        residuals.push(if theta.abs() > 1e-30 {
            r_norm / theta.abs()
        } else {
            r_norm
        });
    }

    Ok(RayleighRitzResult {
        eigenvalues,
        ritz_vectors,
        residuals,
        n_converged: m,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use neco_sparse::CsrMat;

    #[test]
    fn cholqr_produces_m_orthonormal_basis() {
        let n = 4;
        let m = 2;
        let mut y = vec![1.0f32, 0.5, 0.2, 0.1, 0.3, 1.0, 0.4, 0.2];
        let m_diag = vec![1.0, 2.0, 1.5, 0.5];

        assert!(cholqr_m_orthogonalize(&mut y, &m_diag, n, m));

        for i in 0..m {
            for j in 0..m {
                let dot = (0..n)
                    .map(|k| y[k + i * n] as f64 * m_diag[k] * y[k + j * n] as f64)
                    .sum::<f64>();
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!((dot - expected).abs() < 1e-6, "i={i}, j={j}, dot={dot}");
            }
        }
    }

    #[test]
    fn rayleigh_ritz_sorts_diagonal_eigenvalues() {
        let n = 3;
        let offsets: Vec<usize> = (0..=n).collect();
        let indices: Vec<usize> = (0..n).collect();
        let values = vec![9.0, 1.0, 4.0];
        let k = CsrMat::try_from_csr_data(n, n, offsets, indices, values).unwrap();
        let m_diag = vec![1.0; n];
        let mut y = vec![
            1.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, //
            0.0, 0.0, 1.0, //
        ];

        let result = rayleigh_ritz(&k, &m_diag, &mut y, n, n).unwrap();
        assert_eq!(result.eigenvalues, vec![1.0, 4.0, 9.0]);
        assert_eq!(result.n_converged, n);
    }

    #[test]
    fn rayleigh_ritz_preserves_identity_basis_on_diagonal_problem() {
        let n = 3;
        let offsets: Vec<usize> = (0..=n).collect();
        let indices: Vec<usize> = (0..n).collect();
        let values = vec![9.0, 1.0, 4.0];
        let k = CsrMat::try_from_csr_data(n, n, offsets, indices, values).unwrap();
        let m_diag = vec![1.0; n];
        let mut y = vec![
            1.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, //
            0.0, 0.0, 1.0, //
        ];

        let result = rayleigh_ritz(&k, &m_diag, &mut y, n, n).unwrap();
        for (j, &expected_idx) in [1usize, 2, 0].iter().enumerate() {
            for i in 0..n {
                let expected = if i == expected_idx { 1.0 } else { 0.0 };
                let got = result.ritz_vectors[j * n + i] as f64;
                assert!(
                    (got.abs() - expected).abs() < 1e-6,
                    "j={j}, i={i}, got={got}"
                );
            }
            assert!(
                result.residuals[j] < 1e-10,
                "j={j}, residual={}",
                result.residuals[j]
            );
        }
    }
}
