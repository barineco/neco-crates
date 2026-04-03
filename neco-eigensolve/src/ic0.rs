use neco_sparse::CsrMat;

use crate::DenseMatrix;
use crate::Preconditioner;

/// Incomplete Cholesky IC(0) preconditioner.
///
/// Builds `L` with the same sparsity pattern as `K` such that `L * L^T ≈ K`.
/// When `m_diag` is provided, a small diagonal perturbation `eps * diag(M)` is added
/// before factorization to stabilize semi-definite systems.
#[derive(Debug)]
pub struct Ic0Preconditioner {
    l_values: Vec<f64>,
    row_offsets: Vec<usize>,
    col_indices: Vec<usize>,
    n: usize,
}

impl Ic0Preconditioner {
    /// Build IC(0) from `K`, optionally perturbed by `eps * diag(M)`.
    pub fn new(k_mat: &CsrMat<f64>, m_diag: Option<&[f64]>) -> Result<Self, String> {
        let n = k_mat.nrows();
        let eps = 1e-8;
        if n != k_mat.ncols() {
            return Err(format!(
                "IC(0) requires a square matrix, got {n}x{}",
                k_mat.ncols()
            ));
        }

        let mut values = k_mat.values().to_vec();
        if let Some(m_diag) = m_diag {
            if m_diag.len() != n {
                return Err(format!(
                    "m_diag length {} does not match matrix size {n}",
                    m_diag.len()
                ));
            }
            for (i, &m_diag_i) in m_diag.iter().enumerate() {
                if !m_diag_i.is_finite() {
                    return Err(format!("m_diag[{i}] is not finite"));
                }
                let row = k_mat.row(i);
                let Some(diag_pos) = row.col_indices().iter().position(|&col| col == i) else {
                    return Err(format!("IC(0) requires a diagonal entry at row {i}"));
                };
                let offset = k_mat.row_offsets()[i];
                values[offset + diag_pos] += eps * m_diag_i;
            }
        }

        let row_offsets = k_mat.row_offsets().to_vec();
        let col_indices = k_mat.col_indices().to_vec();

        for i in 0..n {
            let row_start = row_offsets[i];
            let row_end = row_offsets[i + 1];
            let Some(diag_pos) = (row_start..row_end).find(|&pos| col_indices[pos] == i) else {
                return Err(format!("IC(0) requires a diagonal entry at row {i}"));
            };

            for pos in row_start..diag_pos {
                let j = col_indices[pos];
                let j_row_start = row_offsets[j];
                let j_row_end = row_offsets[j + 1];

                let mut sum = 0.0;
                let mut pi = row_start;
                let mut pj = j_row_start;
                while pi < pos && pj < j_row_end && col_indices[pj] < j {
                    let ci = col_indices[pi];
                    let cj = col_indices[pj];
                    if ci == cj {
                        sum += values[pi] * values[pj];
                        pi += 1;
                        pj += 1;
                    } else if ci < cj {
                        pi += 1;
                    } else {
                        pj += 1;
                    }
                }

                let Some(j_diag_pos) = (j_row_start..j_row_end).find(|&p| col_indices[p] == j)
                else {
                    return Err(format!("IC(0) requires a diagonal entry at row {j}"));
                };
                let l_jj = values[j_diag_pos];
                if l_jj.abs() <= 1e-30 {
                    return Err(format!(
                        "IC(0) encountered a near-zero pivot at row {j}: {l_jj:e}"
                    ));
                }
                values[pos] = (values[pos] - sum) / l_jj;
            }

            let diag_sum = values[row_start..diag_pos]
                .iter()
                .map(|value| value * value)
                .sum::<f64>();
            let diag_value = values[diag_pos] - diag_sum;
            if diag_value <= 1e-30 || !diag_value.is_finite() {
                return Err(format!(
                    "IC(0) lost positive definiteness at row {i}: {diag_value:e}"
                ));
            }
            values[diag_pos] = diag_value.sqrt();
        }

        Ok(Self {
            l_values: values,
            row_offsets,
            col_indices,
            n,
        })
    }

    fn forward_solve(&self, rhs: &[f64], y: &mut [f64]) {
        for i in 0..self.n {
            let row_start = self.row_offsets[i];
            let row_end = self.row_offsets[i + 1];
            let diag_pos = (row_start..row_end)
                .find(|&pos| self.col_indices[pos] == i)
                .expect("diagonal entry is required for triangular solve");

            let mut sum = rhs[i];
            for pos in row_start..diag_pos {
                let j = self.col_indices[pos];
                sum -= self.l_values[pos] * y[j];
            }
            y[i] = sum / self.l_values[diag_pos];
        }
    }

    fn backward_solve(&self, y: &[f64], x: &mut [f64]) {
        x.copy_from_slice(y);

        for i in (0..self.n).rev() {
            let row_start = self.row_offsets[i];
            let row_end = self.row_offsets[i + 1];
            let diag_pos = (row_start..row_end)
                .find(|&pos| self.col_indices[pos] == i)
                .expect("diagonal entry is required for triangular solve");

            x[i] /= self.l_values[diag_pos];
            for pos in row_start..diag_pos {
                let j = self.col_indices[pos];
                x[j] -= self.l_values[pos] * x[i];
            }
        }
    }

    fn apply_block(&self, residuals: &DenseMatrix) -> DenseMatrix {
        let mut result = DenseMatrix::zeros(residuals.nrows(), residuals.ncols());
        for col in 0..residuals.ncols() {
            let rhs = residuals.column(col);
            let mut y = vec![0.0; self.n];
            let mut x = vec![0.0; self.n];
            self.forward_solve(rhs, &mut y);
            self.backward_solve(&y, &mut x);
            result.column_mut(col).copy_from_slice(&x);
        }
        result
    }
}

impl Preconditioner for Ic0Preconditioner {
    fn apply(&self, residuals: &DenseMatrix) -> DenseMatrix {
        self.apply_block(residuals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neco_sparse::CooMat;

    fn csr_from_triplets(n: usize, triplets: &[(usize, usize, f64)]) -> CsrMat<f64> {
        let mut coo = CooMat::new(n, n);
        for &(i, j, value) in triplets {
            coo.push(i, j, value);
        }
        CsrMat::from(&coo)
    }

    fn dense_from_csr(mat: &CsrMat<f64>) -> DenseMatrix {
        let n = mat.nrows();
        let mut dense = DenseMatrix::zeros(n, n);
        for i in 0..n {
            let row = mat.row(i);
            for (&col, &value) in row.col_indices().iter().zip(row.values()) {
                dense[(i, col)] = value;
            }
        }
        dense
    }

    #[test]
    fn ic0_factorization_is_exact_for_tridiagonal_spd_matrix() {
        let k = csr_from_triplets(
            4,
            &[
                (0, 0, 2.0),
                (0, 1, -1.0),
                (1, 0, -1.0),
                (1, 1, 2.0),
                (1, 2, -1.0),
                (2, 1, -1.0),
                (2, 2, 2.0),
                (2, 3, -1.0),
                (3, 2, -1.0),
                (3, 3, 2.0),
            ],
        );
        let ic0 = Ic0Preconditioner::new(&k, None).unwrap();

        let mut l_dense = DenseMatrix::zeros(4, 4);
        for i in 0..4 {
            for pos in ic0.row_offsets[i]..ic0.row_offsets[i + 1] {
                let j = ic0.col_indices[pos];
                if j <= i {
                    l_dense[(i, j)] = ic0.l_values[pos];
                }
            }
        }

        let reconstructed = l_dense.mul(&l_dense.transpose());
        let expected = dense_from_csr(&k);
        let max_abs_err = reconstructed
            .as_slice()
            .iter()
            .zip(expected.as_slice())
            .map(|(actual, expected)| (actual - expected).abs())
            .fold(0.0, f64::max);
        assert!(
            max_abs_err < 1e-12,
            "IC(0) should match tridiagonal Cholesky"
        );
    }

    #[test]
    fn ic0_with_mass_perturbation_handles_semidefinite_laplacian() {
        let k = csr_from_triplets(
            3,
            &[
                (0, 0, 1.0),
                (0, 1, -1.0),
                (1, 0, -1.0),
                (1, 1, 2.0),
                (1, 2, -1.0),
                (2, 1, -1.0),
                (2, 2, 1.0),
            ],
        );
        let ic0 = Ic0Preconditioner::new(&k, Some(&[1.0, 1.0, 1.0])).unwrap();
        let residual = DenseMatrix::from_column_slice(3, 1, &[1.0, -0.5, 0.25]);
        let result = ic0.apply(&residual);

        assert!(result.as_slice().iter().all(|value| value.is_finite()));
        assert!(result.as_slice().iter().any(|value| value.abs() > 1e-10));
    }

    #[test]
    fn ic0_rejects_missing_diagonal_entry() {
        let k = csr_from_triplets(2, &[(0, 1, 1.0), (1, 0, 1.0), (1, 1, 2.0)]);
        let err = Ic0Preconditioner::new(&k, None).unwrap_err();
        assert!(err.contains("diagonal entry"), "err={err}");
    }
}
