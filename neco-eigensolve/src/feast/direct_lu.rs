#[cfg(feature = "faer-lu")]
use faer::c64 as FaerC64;
#[cfg(feature = "faer-lu")]
use faer::col::Col;
#[cfg(feature = "faer-lu")]
use faer::linalg::solvers::Solve;
#[cfg(feature = "faer-lu")]
use faer::sparse::linalg::solvers::{Lu, SymbolicLu};
#[cfg(feature = "faer-lu")]
use faer::sparse::{SparseColMat, Triplet};

use neco_sparse::CsrMat;

use crate::c64::C64;

use super::backend::PreparedLinearSolver;
use super::internal_lu::ShiftedCsrData;

#[cfg(feature = "faer-lu")]
fn to_faer_c64(v: C64) -> FaerC64 {
    FaerC64::new(v.re, v.im)
}

#[cfg(feature = "faer-lu")]
fn from_faer_c64(v: FaerC64) -> C64 {
    C64::new(v.re, v.im)
}

#[cfg(feature = "faer-lu")]
fn build_complex_shifted_matrix(
    k_mat: &CsrMat<f64>,
    m_mat: &CsrMat<f64>,
    z: C64,
) -> Result<SparseColMat<usize, FaerC64>, String> {
    let shifted = ShiftedCsrData::from_shift(k_mat, m_mat, z);
    shifted
        .diagonal_positions()
        .map_err(|err| format!("direct LU shifted matrix is invalid: {err}"))?;
    let mut triplets = Vec::with_capacity(shifted.values.len());
    for row_idx in 0..shifted.n {
        let start = shifted.row_offsets[row_idx];
        let end = shifted.row_offsets[row_idx + 1];
        for pos in start..end {
            let val = shifted.values[pos];
            let abs2 = val.re * val.re + val.im * val.im;
            let col = shifted.col_indices[pos];
            if abs2 > 1e-40 || col == row_idx {
                triplets.push(Triplet::new(row_idx, col, to_faer_c64(val)));
            }
        }
    }

    SparseColMat::try_new_from_triplets(shifted.n, shifted.n, &triplets)
        .map_err(|e| format!("failed to build direct LU shift matrix: {e}"))
}

#[cfg(feature = "faer-lu")]
pub(crate) struct DirectLuShiftedSolver {
    lu: Lu<usize, FaerC64>,
}

#[cfg(feature = "faer-lu")]
impl DirectLuShiftedSolver {
    pub(crate) fn new(k_mat: &CsrMat<f64>, m_mat: &CsrMat<f64>, z: C64) -> Result<Self, String> {
        let shifted = build_complex_shifted_matrix(k_mat, m_mat, z)?;
        let symbolic_ref = shifted.as_ref().symbolic();
        let symbolic = SymbolicLu::try_new(symbolic_ref)
            .map_err(|e| format!("direct LU symbolic factorization failed: {e}"))?;
        let lu = Lu::try_new_with_symbolic(symbolic, shifted.as_ref())
            .map_err(|e| format!("direct LU numeric factorization failed: {e}"))?;
        Ok(Self { lu })
    }
}

#[cfg(feature = "faer-lu")]
impl PreparedLinearSolver for DirectLuShiftedSolver {
    fn solve_block(&self, rhs: &[f64], n: usize, m0: usize, _tol: f64) -> Result<Vec<C64>, String> {
        if rhs.len() != n * m0 {
            return Err(format!(
                "rhs length {} does not match n * m0 = {}",
                rhs.len(),
                n * m0
            ));
        }

        let mut result = Vec::with_capacity(n * m0);
        let mut col_buf = Col::<FaerC64>::zeros(n);
        for j in 0..m0 {
            let col_start = j * n;
            for i in 0..n {
                col_buf[i] = FaerC64::new(rhs[col_start + i], 0.0);
            }
            let x = self.lu.solve(&col_buf);
            for i in 0..n {
                result.push(from_faer_c64(x[i]));
            }
        }
        Ok(result)
    }
}

#[cfg(all(test, feature = "faer-lu"))]
mod tests {
    use super::*;

    fn diagonal_csr(n: usize, diag: &[f64]) -> CsrMat<f64> {
        let offsets: Vec<usize> = (0..=n).collect();
        let indices: Vec<usize> = (0..n).collect();
        CsrMat::try_from_csr_data(n, n, offsets, indices, diag.to_vec()).unwrap()
    }

    #[test]
    fn direct_lu_diagonal_system() {
        let k = diagonal_csr(3, &[1.0, 2.0, 3.0]);
        let m = diagonal_csr(3, &[1.0, 1.0, 1.0]);
        let solver = DirectLuShiftedSolver::new(&k, &m, C64::new(2.5, 1.0)).unwrap();

        let rhs = vec![1.0f64; 3];
        let result = solver.solve_block(&rhs, 3, 1, 1e-10).unwrap();
        let expected = [
            C64::new(1.0, 0.0) / C64::new(1.5, 1.0),
            C64::new(1.0, 0.0) / C64::new(0.5, 1.0),
            C64::new(1.0, 0.0) / C64::new(-0.5, 1.0),
        ];
        for i in 0..3 {
            let diff = result[i] - expected[i];
            let err = (diff.re * diff.re + diff.im * diff.im).sqrt();
            assert!(err < 1e-10, "x[{i}] err={err}");
        }
    }
}
