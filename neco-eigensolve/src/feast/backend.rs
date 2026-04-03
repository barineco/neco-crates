use neco_sparse::CsrMat;

use crate::c64::C64;

use super::gmres::GmresShiftedSolver;
use super::internal_lu::InternalLuShiftedSolver;
use super::FeastBackend;

pub(crate) trait PreparedLinearSolver: Send + Sync {
    fn solve_block(&self, rhs: &[f64], n: usize, m0: usize, tol: f64) -> Result<Vec<C64>, String>;
}

pub(crate) fn prepare_shifted_solver(
    backend: FeastBackend,
    k_mat: &CsrMat<f64>,
    m_mat: &CsrMat<f64>,
    z: C64,
) -> Result<Box<dyn PreparedLinearSolver>, String> {
    match backend {
        FeastBackend::Gmres => Ok(Box::new(GmresShiftedSolver::new(k_mat, m_mat, z)?)),
        FeastBackend::InternalLu => Ok(Box::new(InternalLuShiftedSolver::new(k_mat, m_mat, z)?)),
        #[cfg(feature = "faer-lu")]
        FeastBackend::DirectLu => prepare_direct_lu_solver(k_mat, m_mat, z),
    }
}

#[cfg(feature = "faer-lu")]
fn prepare_direct_lu_solver(
    k_mat: &CsrMat<f64>,
    m_mat: &CsrMat<f64>,
    z: C64,
) -> Result<Box<dyn PreparedLinearSolver>, String> {
    Ok(Box::new(super::direct_lu::DirectLuShiftedSolver::new(
        k_mat, m_mat, z,
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "faer-lu")]
    use neco_sparse::CooMat;

    fn diagonal_csr(n: usize, diag: &[f64]) -> CsrMat<f64> {
        let offsets: Vec<usize> = (0..=n).collect();
        let indices: Vec<usize> = (0..n).collect();
        CsrMat::try_from_csr_data(n, n, offsets, indices, diag.to_vec()).unwrap()
    }

    #[cfg(feature = "faer-lu")]
    fn tridiagonal_csr(n: usize, diag: f64, offdiag: f64) -> CsrMat<f64> {
        let mut coo = CooMat::new(n, n);
        for i in 0..n {
            coo.push(i, i, diag);
            if i > 0 {
                coo.push(i, i - 1, offdiag);
            }
            if i + 1 < n {
                coo.push(i, i + 1, offdiag);
            }
        }
        CsrMat::from(&coo)
    }

    #[cfg(feature = "faer-lu")]
    fn pivot_window_case_k(pivot_row: usize, eps: f64) -> CsrMat<f64> {
        let n = pivot_row + 1;
        let mut coo = CooMat::new(n, n);
        coo.push(0, 0, -eps);
        for i in 1..n {
            coo.push(i, i, -1.0);
        }
        coo.push(pivot_row, 0, -1.0);
        CsrMat::from(&coo)
    }

    #[test]
    fn gmres_backend_rejects_rhs_length_mismatch() {
        let k = diagonal_csr(3, &[1.0, 2.0, 3.0]);
        let m = diagonal_csr(3, &[1.0, 1.0, 1.0]);
        let solver =
            prepare_shifted_solver(FeastBackend::Gmres, &k, &m, C64::new(2.5, 1.0)).unwrap();

        let err = solver.solve_block(&[1.0, 2.0], 3, 1, 1e-10).unwrap_err();
        assert!(
            err.contains("rhs length"),
            "length mismatch should mention rhs length: {err}"
        );
    }

    #[test]
    fn gmres_backend_solves_multiple_rhs_in_column_major_order() {
        let k = diagonal_csr(2, &[1.0, 2.0]);
        let m = diagonal_csr(2, &[1.0, 1.0]);
        let z = C64::new(3.0, 0.5);
        let solver = prepare_shifted_solver(FeastBackend::Gmres, &k, &m, z).unwrap();

        let rhs = vec![1.0, 3.0, 2.0, 4.0];
        let result = solver.solve_block(&rhs, 2, 2, 1e-10).unwrap();
        let d0 = z - C64::new(1.0, 0.0);
        let d1 = z - C64::new(2.0, 0.0);
        let expected = [
            C64::new(1.0, 0.0) / d0,
            C64::new(3.0, 0.0) / d1,
            C64::new(2.0, 0.0) / d0,
            C64::new(4.0, 0.0) / d1,
        ];
        for i in 0..expected.len() {
            let diff = result[i] - expected[i];
            let err = (diff.re * diff.re + diff.im * diff.im).sqrt();
            assert!(err < 1e-8, "entry {i}: err={err}");
        }
    }

    #[test]
    fn internal_lu_backend_rejects_rhs_length_mismatch() {
        let k = diagonal_csr(3, &[1.0, 2.0, 3.0]);
        let m = diagonal_csr(3, &[1.0, 1.0, 1.0]);
        let solver =
            prepare_shifted_solver(FeastBackend::InternalLu, &k, &m, C64::new(2.5, 1.0)).unwrap();

        let err = solver.solve_block(&[1.0, 2.0], 3, 1, 1e-10).unwrap_err();
        assert!(
            err.contains("rhs length"),
            "length mismatch should mention rhs length: {err}"
        );
    }

    #[test]
    fn internal_lu_backend_matches_gmres_for_multiple_rhs() {
        let k = diagonal_csr(2, &[1.0, 2.0]);
        let m = diagonal_csr(2, &[1.0, 1.0]);
        let z = C64::new(3.0, 0.5);
        let rhs = vec![1.0, 3.0, 2.0, 4.0];

        let gmres = prepare_shifted_solver(FeastBackend::Gmres, &k, &m, z)
            .unwrap()
            .solve_block(&rhs, 2, 2, 1e-10)
            .unwrap();
        let internal = prepare_shifted_solver(FeastBackend::InternalLu, &k, &m, z)
            .unwrap()
            .solve_block(&rhs, 2, 2, 1e-10)
            .unwrap();

        for i in 0..gmres.len() {
            let diff = gmres[i] - internal[i];
            let err = (diff.re * diff.re + diff.im * diff.im).sqrt();
            assert!(err < 1e-10, "entry {i}: err={err}");
        }
    }

    #[test]
    fn internal_lu_backend_propagates_factorization_errors() {
        let k = diagonal_csr(2, &[1.0, 2.0]);
        let m = diagonal_csr(2, &[1.0, 1.0]);

        let err = match prepare_shifted_solver(FeastBackend::InternalLu, &k, &m, C64::new(1.0, 0.0))
        {
            Ok(_) => panic!("internal LU preparation should fail on a zero pivot"),
            Err(err) => err,
        };
        assert!(
            err.contains("near-zero diagonal"),
            "factorization error should propagate: {err}"
        );
    }

    #[test]
    #[cfg(feature = "faer-lu")]
    fn direct_lu_backend_rejects_rhs_length_mismatch() {
        let k = diagonal_csr(3, &[1.0, 2.0, 3.0]);
        let m = diagonal_csr(3, &[1.0, 1.0, 1.0]);
        let solver =
            prepare_shifted_solver(FeastBackend::DirectLu, &k, &m, C64::new(2.5, 1.0)).unwrap();

        let err = solver.solve_block(&[1.0, 2.0], 3, 1, 1e-10).unwrap_err();
        assert!(
            err.contains("rhs length"),
            "length mismatch should mention rhs length: {err}"
        );
    }

    #[test]
    #[cfg(feature = "faer-lu")]
    fn direct_lu_backend_matches_gmres_for_multiple_rhs() {
        let k = diagonal_csr(2, &[1.0, 2.0]);
        let m = diagonal_csr(2, &[1.0, 1.0]);
        let z = C64::new(3.0, 0.5);
        let rhs = vec![1.0, 3.0, 2.0, 4.0];

        let gmres = prepare_shifted_solver(FeastBackend::Gmres, &k, &m, z)
            .unwrap()
            .solve_block(&rhs, 2, 2, 1e-10)
            .unwrap();
        let direct = prepare_shifted_solver(FeastBackend::DirectLu, &k, &m, z)
            .unwrap()
            .solve_block(&rhs, 2, 2, 1e-10)
            .unwrap();

        for i in 0..gmres.len() {
            let diff = gmres[i] - direct[i];
            let err = (diff.re * diff.re + diff.im * diff.im).sqrt();
            assert!(err < 1e-10, "entry {i}: err={err}");
        }
    }

    #[test]
    #[cfg(feature = "faer-lu")]
    fn direct_lu_backend_matches_internal_lu_for_multiple_rhs() {
        let k = diagonal_csr(2, &[1.0, 2.0]);
        let m = diagonal_csr(2, &[1.0, 1.0]);
        let z = C64::new(3.0, 0.5);
        let rhs = vec![1.0, 3.0, 2.0, 4.0];

        let internal = prepare_shifted_solver(FeastBackend::InternalLu, &k, &m, z)
            .unwrap()
            .solve_block(&rhs, 2, 2, 1e-10)
            .unwrap();
        let direct = prepare_shifted_solver(FeastBackend::DirectLu, &k, &m, z)
            .unwrap()
            .solve_block(&rhs, 2, 2, 1e-10)
            .unwrap();

        for i in 0..internal.len() {
            let diff = internal[i] - direct[i];
            let err = (diff.re * diff.re + diff.im * diff.im).sqrt();
            assert!(err < 1e-10, "entry {i}: err={err}");
        }
    }

    #[test]
    #[cfg(feature = "faer-lu")]
    fn direct_lu_backend_is_oracle_for_complex_tridiagonal_multiple_rhs() {
        let k = tridiagonal_csr(5, 2.0, -1.0);
        let m = diagonal_csr(5, &[1.0, 1.5, 1.0, 0.75, 1.25]);
        let z = C64::new(1.8, 0.35);
        let rhs = vec![
            1.0, -2.0, 0.5, 1.5, -0.25, //
            -1.0, 0.25, 2.0, -0.5, 3.0,
        ];

        let internal = prepare_shifted_solver(FeastBackend::InternalLu, &k, &m, z)
            .unwrap()
            .solve_block(&rhs, 5, 2, 1e-10)
            .unwrap();
        let direct = prepare_shifted_solver(FeastBackend::DirectLu, &k, &m, z)
            .unwrap()
            .solve_block(&rhs, 5, 2, 1e-10)
            .unwrap();

        for i in 0..internal.len() {
            let diff = internal[i] - direct[i];
            let err = (diff.re * diff.re + diff.im * diff.im).sqrt();
            assert!(err < 1e-9, "entry {i}: err={err}");
        }
    }

    #[test]
    #[cfg(feature = "faer-lu")]
    fn internal_lu_matches_direct_lu_when_pivot_is_at_window_edge() {
        let mut coo = CooMat::new(8, 8);
        coo.push(0, 0, -1e-32);
        coo.push(0, 7, -1.0);
        for i in 1..7 {
            coo.push(i, i, -1.0);
        }
        coo.push(7, 0, -1.0);
        coo.push(7, 7, -1.0);
        let k = CsrMat::from(&coo);
        let m = CsrMat::zeros(8, 8);
        let rhs = vec![1.0 + 1e-32, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0];

        let internal = prepare_shifted_solver(FeastBackend::InternalLu, &k, &m, C64::zero())
            .unwrap()
            .solve_block(&rhs, 8, 1, 1e-10)
            .unwrap();
        let direct = prepare_shifted_solver(FeastBackend::DirectLu, &k, &m, C64::zero())
            .unwrap()
            .solve_block(&rhs, 8, 1, 1e-10)
            .unwrap();

        for i in 0..internal.len() {
            let diff = internal[i] - direct[i];
            let err = (diff.re * diff.re + diff.im * diff.im).sqrt();
            assert!(err < 1e-9, "entry {i}: err={err}");
        }
    }

    #[test]
    #[cfg(feature = "faer-lu")]
    fn direct_lu_exceeds_internal_lu_when_pivot_is_outside_window() {
        let k = pivot_window_case_k(8, 1e-32);
        let m = CsrMat::zeros(9, 9);
        let rhs = vec![1e-32, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0];

        let internal_err =
            match prepare_shifted_solver(FeastBackend::InternalLu, &k, &m, C64::zero()) {
                Ok(_) => panic!(
                    "internal LU should reject pivots that lie outside the bounded search window"
                ),
                Err(err) => err,
            };
        assert!(
            internal_err.contains("pivot window") || internal_err.contains("near-zero diagonal"),
            "internal LU should expose bounded-window failure: {internal_err}"
        );

        let direct = prepare_shifted_solver(FeastBackend::DirectLu, &k, &m, C64::zero())
            .unwrap()
            .solve_block(&rhs, 9, 1, 1e-10)
            .unwrap();
        for (i, value) in direct.iter().enumerate() {
            let expected = C64::new(1.0, 0.0);
            let diff = *value - expected;
            let err = (diff.re * diff.re + diff.im * diff.im).sqrt();
            assert!(err < 1e-9, "entry {i}: err={err}");
        }
    }

    #[test]
    #[cfg(feature = "faer-lu")]
    fn internal_lu_matches_direct_lu_on_outside_window_case_after_complex_shift_regularization() {
        let k = pivot_window_case_k(8, 1e-32);
        let m = diagonal_csr(9, &[1.0; 9]);
        let z = C64::new(0.25, 0.4);
        let rhs = vec![
            1.0, -1.0, 0.5, 2.0, -0.25, 1.5, 0.75, -2.0, 3.0, //
            -0.5, 0.25, 1.25, -1.5, 0.0, 2.5, -0.75, 1.0, -2.0,
        ];

        let internal = prepare_shifted_solver(FeastBackend::InternalLu, &k, &m, z)
            .unwrap()
            .solve_block(&rhs, 9, 2, 1e-10)
            .unwrap();
        let direct = prepare_shifted_solver(FeastBackend::DirectLu, &k, &m, z)
            .unwrap()
            .solve_block(&rhs, 9, 2, 1e-10)
            .unwrap();

        for i in 0..internal.len() {
            let diff = internal[i] - direct[i];
            let err = (diff.re * diff.re + diff.im * diff.im).sqrt();
            assert!(err < 1e-9, "entry {i}: err={err}");
        }
    }

    #[test]
    #[cfg(feature = "faer-lu")]
    fn internal_lu_uses_limited_row_swap_for_small_pivot_threshold() {
        let k = CsrMat::try_from_csr_data(
            2,
            2,
            vec![0, 2, 4],
            vec![0, 1, 0, 1],
            vec![-1e-32, -1.0, -1.0, -1.0],
        )
        .unwrap();
        let m = CsrMat::zeros(2, 2);
        let z = C64::new(0.0, 0.0);
        let rhs = vec![1.0, -0.5];

        let internal = prepare_shifted_solver(FeastBackend::InternalLu, &k, &m, z)
            .unwrap()
            .solve_block(&rhs, 2, 1, 1e-10)
            .unwrap();
        let direct = prepare_shifted_solver(FeastBackend::DirectLu, &k, &m, z).unwrap();
        let result = direct.solve_block(&rhs, 2, 1, 1e-10).unwrap();

        for i in 0..internal.len() {
            let diff = internal[i] - result[i];
            let err = (diff.re * diff.re + diff.im * diff.im).sqrt();
            assert!(err < 1e-9, "entry {i}: err={err}");
        }
    }

    #[test]
    #[cfg(feature = "faer-lu")]
    fn internal_lu_uses_limited_row_swap_for_zero_pivot_case() {
        let k = CsrMat::try_from_csr_data(2, 2, vec![0, 1, 3], vec![1, 0, 1], vec![1.0, 1.0, 1.0])
            .unwrap();
        let m = diagonal_csr(2, &[1.0, 1.0]);
        let z = C64::new(0.0, 0.0);
        let rhs = vec![1.0, 2.0];

        let internal = prepare_shifted_solver(FeastBackend::InternalLu, &k, &m, z)
            .unwrap()
            .solve_block(&rhs, 2, 1, 1e-10)
            .unwrap();
        let direct = prepare_shifted_solver(FeastBackend::DirectLu, &k, &m, z).unwrap();
        let result = direct.solve_block(&rhs, 2, 1, 1e-10).unwrap();

        for i in 0..result.len() {
            let diff = internal[i] - result[i];
            let err = (diff.re * diff.re + diff.im * diff.im).sqrt();
            assert!(err < 1e-10, "entry {i}: err={err}");
        }
    }
}
