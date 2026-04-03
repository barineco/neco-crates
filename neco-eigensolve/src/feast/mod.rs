//! FEAST contour-integral eigensolver (Polizzi 2009).

mod backend;
pub(crate) mod complex_csr;
pub(crate) mod complex_ilu0;
pub mod contour;
#[cfg(feature = "faer-lu")]
mod direct_lu;
pub(crate) mod gmres;
pub(crate) mod internal_lu;

use neco_sparse::CsrMat;

use crate::c64::C64;
use crate::dense::{symmetric_dense_kernel, SymmetricEigenResult};

/// Linear solver backend used for contour-point shifted systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FeastBackend {
    /// GMRES(m) with complex ILU(0) preconditioning.
    Gmres,
    /// Internal direct sparse LU backend under development.
    #[allow(dead_code)]
    InternalLu,
    /// Direct sparse LU powered by `faer` when the `faer-lu` feature is enabled.
    #[cfg(feature = "faer-lu")]
    #[allow(dead_code)]
    DirectLu,
}

/// FEAST solver configuration.
pub struct FeastConfig {
    /// Subspace size (upper bound on the number of eigenvalues to search for).
    pub m0: usize,
    /// Number of Gauss-Legendre quadrature points (4, 8, or 16).
    pub n_quadrature: usize,
    /// Convergence threshold for relative trace change.
    pub tol: f64,
    /// Maximum number of outer iterations.
    pub max_loops: usize,
    /// Seed for the initial random matrix.
    pub seed: u64,
}

impl Default for FeastConfig {
    fn default() -> Self {
        Self {
            m0: 30,
            n_quadrature: 8,
            tol: 1e-10,
            max_loops: 20,
            seed: 0xCAFE_BEEF,
        }
    }
}

/// Per-iteration progress info.
pub struct FeastIterationInfo {
    pub loop_idx: usize,
    pub trace: f64,
    pub trace_change: f64,
    pub n_converged: usize,
    pub converged: bool,
}

#[derive(Debug)]
/// Result of a FEAST solve.
pub struct FeastIntervalResult {
    pub eigenvalues: Vec<f64>,
    /// Eigenvectors (N x n_found, column-major).
    pub eigenvectors: Vec<f64>,
    pub residuals: Vec<f64>,
}

fn transpose_mul_col_major(
    a: &[f64],
    a_rows: usize,
    a_cols: usize,
    b: &[f64],
    b_cols: usize,
) -> Vec<f64> {
    assert_eq!(a.len(), a_rows * a_cols);
    assert_eq!(b.len(), a_rows * b_cols);
    let mut out = vec![0.0; a_cols * b_cols];
    for i in 0..a_cols {
        for j in 0..b_cols {
            let mut sum = 0.0;
            for k in 0..a_rows {
                sum += a[i * a_rows + k] * b[j * a_rows + k];
            }
            out[i * b_cols + j] = sum;
        }
    }
    out
}

fn symmetrize_row_major(a: &mut [f64], n: usize) {
    assert_eq!(a.len(), n * n);
    for i in 0..n {
        for j in (i + 1)..n {
            let avg = (a[i * n + j] + a[j * n + i]) / 2.0;
            a[i * n + j] = avg;
            a[j * n + i] = avg;
        }
    }
}

fn right_multiply_col_major_by_row_major_cols(
    a: &[f64],
    a_rows: usize,
    a_cols: usize,
    rhs_cols_as_row_major: &[f64],
    b_cols: usize,
) -> Vec<f64> {
    assert_eq!(a.len(), a_rows * a_cols);
    assert_eq!(rhs_cols_as_row_major.len(), a_cols * b_cols);
    let mut out = vec![0.0; a_rows * b_cols];
    for j in 0..b_cols {
        for p in 0..a_cols {
            let coeff = rhs_cols_as_row_major[p * b_cols + j];
            for i in 0..a_rows {
                out[j * a_rows + i] += a[p * a_rows + i] * coeff;
            }
        }
    }
    out
}

fn build_orthonormal_subspace(
    q_mat: &[f64],
    n: usize,
    m0: usize,
    gram_eigen: &SymmetricEigenResult,
    keep: &[usize],
) -> Vec<f64> {
    let m_eff = keep.len();
    let mut out = vec![0.0; n * m_eff];
    for out_col in 0..m_eff {
        let eig_col = keep[out_col];
        let scale = 1.0 / gram_eigen.eigenvalues[eig_col].sqrt();
        for basis_col in 0..m0 {
            let coeff = gram_eigen.eigenvectors[basis_col * m0 + eig_col] * scale;
            for row in 0..n {
                out[out_col * n + row] += q_mat[basis_col * n + row] * coeff;
            }
        }
    }
    out
}

fn contour_reduce<S, F>(
    solvers: &[S],
    cpts: &[contour::ContourPoint],
    identity: impl Fn() -> Vec<f64> + Sync + Send,
    f: F,
) -> Result<Vec<f64>, String>
where
    S: Sync,
    F: Fn(&S, &contour::ContourPoint) -> Result<Vec<f64>, String> + Sync + Send,
{
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        let results: Vec<Result<Vec<f64>, String>> = solvers
            .par_iter()
            .zip(cpts.par_iter())
            .map(|(solver, cp)| f(solver, cp))
            .collect();
        let mut acc = identity();
        for r in results {
            let partial = r?;
            for (a, p) in acc.iter_mut().zip(partial.iter()) {
                *a += p;
            }
        }
        Ok(acc)
    }
    #[cfg(not(feature = "parallel"))]
    {
        let mut acc = identity();
        for (solver, cp) in solvers.iter().zip(cpts.iter()) {
            let partial = f(solver, cp)?;
            for (a, p) in acc.iter_mut().zip(partial.iter()) {
                *a += p;
            }
        }
        Ok(acc)
    }
}

/// Solve the generalized eigenproblem K x = λ M x via FEAST contour integration.
pub fn feast_solve_interval(
    k_mat: &CsrMat<f64>,
    m_mat: &CsrMat<f64>,
    interval: &contour::FeastInterval,
    config: &FeastConfig,
    on_progress: Option<&mut dyn FnMut(&FeastIterationInfo)>,
) -> Result<FeastIntervalResult, String> {
    feast_solve_interval_with_backend(
        k_mat,
        m_mat,
        interval,
        config,
        FeastBackend::Gmres,
        on_progress,
    )
}

/// Solve the generalized eigenproblem K x = λ M x via FEAST contour integration
/// using an explicit contour-point linear solver backend.
pub(crate) fn feast_solve_interval_with_backend(
    k_mat: &CsrMat<f64>,
    m_mat: &CsrMat<f64>,
    interval: &contour::FeastInterval,
    config: &FeastConfig,
    backend: FeastBackend,
    mut on_progress: Option<&mut dyn FnMut(&FeastIterationInfo)>,
) -> Result<FeastIntervalResult, String> {
    if !matches!(config.n_quadrature, 4 | 8 | 16) {
        return Err(format!(
            "n_quadrature={} is not supported (only 4, 8, 16)",
            config.n_quadrature
        ));
    }
    if config.m0 == 0 {
        return Err("m0 must be >= 1".to_string());
    }

    let n = k_mat.nrows();
    let m0 = config.m0;
    let r = (interval.lambda_max - interval.lambda_min) / 2.0;
    let cpts = contour::contour_points(interval, config.n_quadrature)?;
    let solvers: Vec<Box<dyn backend::PreparedLinearSolver>> = cpts
        .iter()
        .map(|cp| backend::prepare_shifted_solver(backend, k_mat, m_mat, cp.z))
        .collect::<Result<_, _>>()?;

    let mut rng = crate::rng::Rng::new(config.seed);
    let mut y_mat: Vec<f64> = (0..n * m0).map(|_| rng.next_f64() - 0.5).collect();
    let mut prev_trace = f64::MAX;
    let mut last_result: Option<FeastIntervalResult> = None;
    let mut last_trace = None;
    let mut last_trace_change = None;
    let mut last_n_found = None;

    for iter_idx in 0..config.max_loops {
        let y_snapshot = y_mat.clone();
        let q_mat = contour_reduce(
            &solvers,
            &cpts,
            || vec![0.0f64; n * m0],
            |solver, cp| {
                let q_e = solver.solve_block(&y_snapshot, n, m0, 1e-8)?;
                let exp_theta = C64::new(cp.theta.cos(), cp.theta.sin());
                let coeff = C64::new(-cp.weight / 2.0, 0.0) * C64::new(r, 0.0) * exp_theta;
                let mut partial = vec![0.0f64; n * m0];
                for idx in 0..n * m0 {
                    partial[idx] = (coeff * q_e[idx]).re;
                }
                Ok(partial)
            },
        )?;

        let mut mq_mat = vec![0.0f64; n * m0];
        crate::spmv::block_spmv_dispatch(&mut mq_mat, m_mat, &q_mat, n, m0);
        let mut gram = transpose_mul_col_major(&q_mat, n, m0, &mq_mat, m0);
        symmetrize_row_major(&mut gram, m0);
        let gram_eigen = symmetric_dense_kernel().symmetric_eigen(&gram, m0);

        let max_eval = gram_eigen
            .eigenvalues
            .iter()
            .copied()
            .fold(0.0f64, f64::max);
        let threshold = max_eval * 1e-12;
        let keep: Vec<usize> = gram_eigen
            .eigenvalues
            .iter()
            .enumerate()
            .filter(|(_, &v)| v > threshold)
            .map(|(i, _)| i)
            .collect();
        let m_eff = keep.len();
        if m_eff == 0 {
            return Err("Gram matrix has no significant eigenvalues".to_string());
        }

        let q_orth = build_orthonormal_subspace(&q_mat, n, m0, &gram_eigen, &keep);
        let mut kq_mat = vec![0.0f64; n * m_eff];
        crate::spmv::block_spmv_dispatch(&mut kq_mat, k_mat, &q_orth, n, m_eff);

        let mut a_q = transpose_mul_col_major(&q_orth, n, m_eff, &kq_mat, m_eff);
        symmetrize_row_major(&mut a_q, m_eff);
        let eigen = symmetric_dense_kernel().symmetric_eigen(&a_q, m_eff);

        let all_evals: Vec<f64> = eigen.eigenvalues.clone();
        let x_full = right_multiply_col_major_by_row_major_cols(
            &q_orth,
            n,
            m_eff,
            &eigen.eigenvectors,
            m_eff,
        );

        let mut indices_in = Vec::new();
        for (i, &lam) in all_evals.iter().enumerate() {
            if lam >= interval.lambda_min && lam <= interval.lambda_max {
                indices_in.push(i);
            }
        }
        indices_in.sort_by(|&a, &b| all_evals[a].total_cmp(&all_evals[b]));

        let n_found = indices_in.len();
        let mut eigenvalues = Vec::with_capacity(n_found);
        let mut eigenvectors = Vec::with_capacity(n * n_found);
        let mut residuals = Vec::with_capacity(n_found);

        let mut x_block = vec![0.0f64; n * n_found];
        for (out_idx, &idx) in indices_in.iter().enumerate() {
            for i in 0..n {
                x_block[out_idx * n + i] = x_full[idx * n + i];
            }
        }

        let mut kx_block = vec![0.0f64; n * n_found];
        let mut mx_block = vec![0.0f64; n * n_found];
        if n_found > 0 {
            crate::spmv::block_spmv_dispatch(&mut kx_block, k_mat, &x_block, n, n_found);
            crate::spmv::block_spmv_dispatch(&mut mx_block, m_mat, &x_block, n, n_found);
        }

        for (out_idx, &idx) in indices_in.iter().enumerate() {
            let lam = all_evals[idx];
            eigenvalues.push(lam);
            let col_start = out_idx * n;
            eigenvectors.extend_from_slice(&x_block[col_start..col_start + n]);

            let kx_tmp = &kx_block[col_start..col_start + n];
            let mx_tmp = &mx_block[col_start..col_start + n];
            let mut res_norm_sq = 0.0;
            let mut kx_norm_sq = 0.0;
            for i in 0..n {
                let diff = kx_tmp[i] - lam * mx_tmp[i];
                res_norm_sq += diff * diff;
                kx_norm_sq += kx_tmp[i] * kx_tmp[i];
            }
            let res = if kx_norm_sq > 0.0 {
                res_norm_sq.sqrt() / kx_norm_sq.sqrt()
            } else {
                0.0
            };
            residuals.push(res);
        }

        last_result = Some(FeastIntervalResult {
            eigenvalues: eigenvalues.clone(),
            eigenvectors: eigenvectors.clone(),
            residuals: residuals.clone(),
        });

        let trace: f64 = eigenvalues.iter().sum();
        let trace_change = if prev_trace < f64::MAX && trace.abs() > 1e-30 {
            ((trace - prev_trace) / trace.abs()).abs()
        } else {
            f64::MAX
        };
        let converged = trace_change < config.tol && n_found > 0;
        last_trace = Some(trace);
        last_trace_change = Some(trace_change);
        last_n_found = Some(n_found);

        if let Some(ref mut cb) = on_progress {
            cb(&FeastIterationInfo {
                loop_idx: iter_idx,
                trace,
                trace_change,
                n_converged: n_found,
                converged,
            });
        }

        if converged {
            return Ok(FeastIntervalResult {
                eigenvalues,
                eigenvectors,
                residuals,
            });
        }
        prev_trace = trace;

        let n_copy = m_eff.min(m0);
        {
            let mut my_block = vec![0.0f64; n * n_copy];
            crate::spmv::block_spmv_dispatch(
                &mut my_block,
                m_mat,
                &x_full[..n * n_copy],
                n,
                n_copy,
            );
            y_mat[..n * n_copy].copy_from_slice(&my_block);
        }
        for j in n_copy..m0 {
            let col_start = j * n;
            for i in 0..n {
                y_mat[col_start + i] = rng.next_f64() - 0.5;
            }
        }
    }

    let last_result = last_result.unwrap_or(FeastIntervalResult {
        eigenvalues: vec![],
        eigenvectors: vec![],
        residuals: vec![],
    });
    Err(format!(
        "FEAST did not converge within max_loops={}; last_n_found={}, last_trace_change={}, last_trace={}, last_eigenvalues={:?}",
        config.max_loops,
        last_n_found.unwrap_or(0),
        last_trace_change.unwrap_or(f64::MAX),
        last_trace.unwrap_or(f64::NAN),
        last_result.eigenvalues
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "faer-lu")]
    use neco_sparse::CooMat;
    use neco_sparse::CsrMat;

    fn diagonal_csr(n: usize, f: impl Fn(usize) -> f64) -> CsrMat<f64> {
        let offsets: Vec<usize> = (0..=n).collect();
        let indices: Vec<usize> = (0..n).collect();
        let diag: Vec<f64> = (0..n).map(f).collect();
        CsrMat::try_from_csr_data(n, n, offsets, indices, diag).unwrap()
    }

    fn permuted_diagonal_csr(values: &[f64], perm: &[usize]) -> CsrMat<f64> {
        let n = values.len();
        let offsets: Vec<usize> = (0..=n).collect();
        let indices: Vec<usize> = (0..n).collect();
        let diag: Vec<f64> = perm.iter().map(|&i| values[i]).collect();
        CsrMat::try_from_csr_data(n, n, offsets, indices, diag).unwrap()
    }

    #[cfg(feature = "faer-lu")]
    fn tridiagonal_laplacian_csr(n: usize) -> CsrMat<f64> {
        let mut coo = CooMat::new(n, n);
        for i in 0..n {
            coo.push(i, i, 2.0);
            if i > 0 {
                coo.push(i, i - 1, -1.0);
            }
            if i + 1 < n {
                coo.push(i, i + 1, -1.0);
            }
        }
        CsrMat::from(&coo)
    }

    #[cfg(feature = "faer-lu")]
    fn symmetric_pivot_window_case_k(pivot_row: usize, eps: f64) -> CsrMat<f64> {
        let n = pivot_row + 1;
        let mut coo = CooMat::new(n, n);
        coo.push(0, 0, eps);
        for i in 1..n {
            coo.push(i, i, 1.0);
        }
        coo.push(0, pivot_row, 1.0);
        coo.push(pivot_row, 0, 1.0);
        CsrMat::from(&coo)
    }

    fn sorted(mut values: Vec<f64>) -> Vec<f64> {
        values.sort_by(f64::total_cmp);
        values
    }

    #[test]
    fn feast_single_interval_diagonal() {
        let n = 20;
        let k = diagonal_csr(n, |i| ((i + 1) * (i + 1)) as f64);
        let m = diagonal_csr(n, |_| 1.0);

        let interval = contour::FeastInterval {
            lambda_min: 0.5,
            lambda_max: 100.5,
        };
        let config = FeastConfig {
            m0: 15,
            ..Default::default()
        };

        let result = feast_solve_interval(&k, &m, &interval, &config, None).unwrap();

        assert_eq!(result.eigenvalues.len(), 10);
        for (i, &lam) in result.eigenvalues.iter().enumerate() {
            let expected = ((i + 1) * (i + 1)) as f64;
            let err = (lam - expected).abs() / expected;
            assert!(
                err < 1e-6,
                "eig[{i}] = {lam}, expected {expected}, err={err}"
            );
        }

        for (i, &res) in result.residuals.iter().enumerate() {
            assert!(res < 1e-4, "residual[{i}]={res}");
        }
    }

    #[test]
    fn feast_interval_contains_only_requested_eigenvalues() {
        let n = 20;
        let values: Vec<f64> = (1..=n).map(|i| (i * i) as f64).collect();
        let k = diagonal_csr(n, |i| values[i]);
        let m = diagonal_csr(n, |_| 1.0);
        let config = FeastConfig {
            m0: 12,
            seed: 42,
            ..Default::default()
        };
        let interval = contour::FeastInterval {
            lambda_min: 10.0,
            lambda_max: 50.0,
        };

        let result = feast_solve_interval(&k, &m, &interval, &config, None).unwrap();
        assert_eq!(result.eigenvalues.len(), 4);
        for &lam in &result.eigenvalues {
            assert!(
                lam >= interval.lambda_min - 1e-10 && lam <= interval.lambda_max + 1e-10,
                "lambda out of interval: {lam}"
            );
        }
        let expected = [16.0, 25.0, 36.0, 49.0];
        let got = sorted(result.eigenvalues);
        for i in 0..expected.len() {
            assert!((got[i] - expected[i]).abs() < 1e-6, "i={i}: {}", got[i]);
        }
    }

    #[test]
    fn feast_invalid_config_returns_err() {
        let n = 8;
        let k = diagonal_csr(n, |i| (i + 1) as f64);
        let m = diagonal_csr(n, |_| 1.0);
        let interval = contour::FeastInterval {
            lambda_min: 0.0,
            lambda_max: 10.0,
        };

        let bad_quad = FeastConfig {
            m0: 4,
            n_quadrature: 6,
            ..Default::default()
        };
        assert!(feast_solve_interval(&k, &m, &interval, &bad_quad, None).is_err());

        let bad_m0 = FeastConfig {
            m0: 0,
            ..Default::default()
        };
        assert!(feast_solve_interval(&k, &m, &interval, &bad_m0, None).is_err());
    }

    #[test]
    fn feast_reports_nonconvergence_in_err() {
        let n = 6;
        let k = diagonal_csr(n, |i| (i + 1) as f64);
        let m = diagonal_csr(n, |_| 1.0);
        let interval = contour::FeastInterval {
            lambda_min: 100.0,
            lambda_max: 101.0,
        };
        let config = FeastConfig {
            m0: 4,
            max_loops: 2,
            seed: 5,
            ..Default::default()
        };

        let err = feast_solve_interval(&k, &m, &interval, &config, None).unwrap_err();
        assert!(
            err.contains("did not converge"),
            "nonconvergence should be explicit: {err}"
        );
        assert!(
            err.contains("last_n_found=0"),
            "error should include last found count: {err}"
        );
    }

    #[test]
    fn internal_lu_backend_matches_gmres_on_diagonal_problem() {
        let n = 20;
        let k = diagonal_csr(n, |i| ((i + 1) * (i + 1)) as f64);
        let m = diagonal_csr(n, |_| 1.0);
        let interval = contour::FeastInterval {
            lambda_min: 0.5,
            lambda_max: 100.5,
        };
        let config = FeastConfig {
            m0: 15,
            seed: 7,
            ..Default::default()
        };

        let gmres = feast_solve_interval_with_backend(
            &k,
            &m,
            &interval,
            &config,
            FeastBackend::Gmres,
            None,
        )
        .unwrap();
        let internal = feast_solve_interval_with_backend(
            &k,
            &m,
            &interval,
            &config,
            FeastBackend::InternalLu,
            None,
        )
        .unwrap();

        let a = sorted(gmres.eigenvalues);
        let b = sorted(internal.eigenvalues);
        assert_eq!(a.len(), b.len());
        for i in 0..a.len() {
            assert!((a[i] - b[i]).abs() < 1e-6, "i={i}: {} vs {}", a[i], b[i]);
        }
    }

    #[test]
    #[cfg(feature = "faer-lu")]
    fn direct_lu_backend_matches_gmres_on_diagonal_problem() {
        let n = 20;
        let k = diagonal_csr(n, |i| ((i + 1) * (i + 1)) as f64);
        let m = diagonal_csr(n, |_| 1.0);
        let interval = contour::FeastInterval {
            lambda_min: 0.5,
            lambda_max: 100.5,
        };
        let gmres = feast_solve_interval(
            &k,
            &m,
            &interval,
            &FeastConfig {
                m0: 15,
                seed: 7,
                ..Default::default()
            },
            None,
        )
        .unwrap();
        let direct = feast_solve_interval_with_backend(
            &k,
            &m,
            &interval,
            &FeastConfig {
                m0: 15,
                seed: 7,
                ..Default::default()
            },
            FeastBackend::DirectLu,
            None,
        )
        .unwrap();

        assert_eq!(gmres.eigenvalues.len(), direct.eigenvalues.len());
        let a = sorted(gmres.eigenvalues);
        let b = sorted(direct.eigenvalues);
        for i in 0..a.len() {
            assert!((a[i] - b[i]).abs() < 1e-6, "i={i}: {} vs {}", a[i], b[i]);
        }
    }

    #[test]
    #[cfg(feature = "faer-lu")]
    fn internal_lu_backend_matches_direct_lu_on_tridiagonal_interval() {
        let n = 16;
        let k = tridiagonal_laplacian_csr(n);
        let m = diagonal_csr(n, |_| 1.0);
        let interval = contour::FeastInterval {
            lambda_min: 0.05,
            lambda_max: 2.5,
        };
        let config = FeastConfig {
            m0: 8,
            seed: 19,
            ..Default::default()
        };

        let internal = feast_solve_interval_with_backend(
            &k,
            &m,
            &interval,
            &config,
            FeastBackend::InternalLu,
            None,
        )
        .unwrap();
        let direct = feast_solve_interval_with_backend(
            &k,
            &m,
            &interval,
            &config,
            FeastBackend::DirectLu,
            None,
        )
        .unwrap();

        let a = sorted(internal.eigenvalues);
        let b = sorted(direct.eigenvalues);
        assert_eq!(a.len(), b.len());
        for i in 0..a.len() {
            assert!((a[i] - b[i]).abs() < 1e-6, "i={i}: {} vs {}", a[i], b[i]);
        }
    }

    #[test]
    fn feast_permutation_similarity_keeps_eigenvalues() {
        let n = 20;
        let values: Vec<f64> = (1..=n).map(|i| (i * i) as f64).collect();
        let perm: Vec<usize> = (0..n).rev().collect();
        let k = diagonal_csr(n, |i| values[i]);
        let kp = permuted_diagonal_csr(&values, &perm);
        let m = diagonal_csr(n, |_| 1.0);
        let interval = contour::FeastInterval {
            lambda_min: 0.5,
            lambda_max: 100.5,
        };
        let config = FeastConfig {
            m0: 15,
            seed: 7,
            ..Default::default()
        };

        let base = feast_solve_interval(&k, &m, &interval, &config, None).unwrap();
        let permuted = feast_solve_interval(&kp, &m, &interval, &config, None).unwrap();
        let a = sorted(base.eigenvalues);
        let b = sorted(permuted.eigenvalues);
        assert_eq!(a.len(), b.len());
        for i in 0..a.len() {
            assert!((a[i] - b[i]).abs() < 1e-6, "i={i}: {} vs {}", a[i], b[i]);
        }
    }

    #[test]
    fn internal_lu_backend_handles_mismatched_sparsity_patterns() {
        let k =
            CsrMat::try_from_csr_data(3, 3, vec![0, 1, 2, 3], vec![0, 1, 2], vec![2.0, 3.0, 5.0])
                .unwrap();
        let m = CsrMat::try_from_csr_data(
            3,
            3,
            vec![0, 2, 5, 7],
            vec![0, 1, 0, 1, 2, 1, 2],
            vec![2.0, 0.25, 0.25, 3.0, 0.5, 0.5, 4.0],
        )
        .unwrap();
        let interval = contour::FeastInterval {
            lambda_min: 0.2,
            lambda_max: 1.6,
        };
        let config = FeastConfig {
            m0: 3,
            seed: 11,
            ..Default::default()
        };

        let gmres = feast_solve_interval_with_backend(
            &k,
            &m,
            &interval,
            &config,
            FeastBackend::Gmres,
            None,
        )
        .unwrap();
        let internal = feast_solve_interval_with_backend(
            &k,
            &m,
            &interval,
            &config,
            FeastBackend::InternalLu,
            None,
        )
        .unwrap();

        let a = sorted(gmres.eigenvalues);
        let b = sorted(internal.eigenvalues);
        assert_eq!(a.len(), b.len());
        for i in 0..a.len() {
            assert!((a[i] - b[i]).abs() < 1e-6, "i={i}: {} vs {}", a[i], b[i]);
        }
    }

    #[test]
    #[cfg(feature = "faer-lu")]
    fn direct_lu_backend_handles_mismatched_sparsity_patterns() {
        let k =
            CsrMat::try_from_csr_data(3, 3, vec![0, 1, 2, 3], vec![0, 1, 2], vec![2.0, 3.0, 5.0])
                .unwrap();
        let m = CsrMat::try_from_csr_data(
            3,
            3,
            vec![0, 2, 5, 7],
            vec![0, 1, 0, 1, 2, 1, 2],
            vec![2.0, 0.25, 0.25, 3.0, 0.5, 0.5, 4.0],
        )
        .unwrap();
        let interval = contour::FeastInterval {
            lambda_min: 0.2,
            lambda_max: 1.6,
        };
        let config = FeastConfig {
            m0: 3,
            seed: 11,
            ..Default::default()
        };

        let gmres = feast_solve_interval_with_backend(
            &k,
            &m,
            &interval,
            &config,
            FeastBackend::Gmres,
            None,
        )
        .unwrap();
        let direct = feast_solve_interval_with_backend(
            &k,
            &m,
            &interval,
            &config,
            FeastBackend::DirectLu,
            None,
        )
        .unwrap();

        let a = sorted(gmres.eigenvalues);
        let b = sorted(direct.eigenvalues);
        assert_eq!(a.len(), b.len());
        for i in 0..a.len() {
            assert!((a[i] - b[i]).abs() < 1e-6, "i={i}: {} vs {}", a[i], b[i]);
        }
    }

    #[test]
    #[cfg(feature = "faer-lu")]
    fn internal_lu_backend_matches_direct_lu_on_mismatched_sparsity_interval() {
        let k =
            CsrMat::try_from_csr_data(3, 3, vec![0, 1, 2, 3], vec![0, 1, 2], vec![2.0, 3.0, 5.0])
                .unwrap();
        let m = CsrMat::try_from_csr_data(
            3,
            3,
            vec![0, 2, 5, 7],
            vec![0, 1, 0, 1, 2, 1, 2],
            vec![2.0, 0.25, 0.25, 3.0, 0.5, 0.5, 4.0],
        )
        .unwrap();
        let interval = contour::FeastInterval {
            lambda_min: 0.2,
            lambda_max: 1.6,
        };
        let config = FeastConfig {
            m0: 3,
            seed: 11,
            ..Default::default()
        };

        let internal = feast_solve_interval_with_backend(
            &k,
            &m,
            &interval,
            &config,
            FeastBackend::InternalLu,
            None,
        )
        .unwrap();
        let direct = feast_solve_interval_with_backend(
            &k,
            &m,
            &interval,
            &config,
            FeastBackend::DirectLu,
            None,
        )
        .unwrap();

        let a = sorted(internal.eigenvalues);
        let b = sorted(direct.eigenvalues);
        assert_eq!(a.len(), b.len());
        for i in 0..a.len() {
            assert!((a[i] - b[i]).abs() < 1e-6, "i={i}: {} vs {}", a[i], b[i]);
        }
    }

    #[test]
    #[cfg(feature = "faer-lu")]
    fn direct_lu_and_internal_lu_agree_on_permuted_diagonal_interval() {
        let n = 12;
        let values: Vec<f64> = (1..=n).map(|i| (i * i) as f64).collect();
        let perm: Vec<usize> = vec![3, 9, 0, 11, 4, 7, 1, 10, 5, 2, 8, 6];
        let k = permuted_diagonal_csr(&values, &perm);
        let m = diagonal_csr(n, |_| 1.0);
        let interval = contour::FeastInterval {
            lambda_min: 5.0,
            lambda_max: 90.0,
        };
        let config = FeastConfig {
            m0: 10,
            seed: 23,
            ..Default::default()
        };

        let internal = feast_solve_interval_with_backend(
            &k,
            &m,
            &interval,
            &config,
            FeastBackend::InternalLu,
            None,
        )
        .unwrap();
        let direct = feast_solve_interval_with_backend(
            &k,
            &m,
            &interval,
            &config,
            FeastBackend::DirectLu,
            None,
        )
        .unwrap();

        let a = sorted(internal.eigenvalues);
        let b = sorted(direct.eigenvalues);
        assert_eq!(a.len(), b.len());
        for i in 0..a.len() {
            assert!((a[i] - b[i]).abs() < 1e-6, "i={i}: {} vs {}", a[i], b[i]);
        }
    }

    #[test]
    #[cfg(feature = "faer-lu")]
    fn internal_lu_backend_matches_direct_lu_on_window_edge_interval() {
        let k = symmetric_pivot_window_case_k(7, 1e-32);
        let m = diagonal_csr(8, |_| 1.0);
        let interval = contour::FeastInterval {
            lambda_min: -1.0,
            lambda_max: 2.0,
        };
        let config = FeastConfig {
            m0: 8,
            n_quadrature: 8,
            seed: 31,
            ..Default::default()
        };

        let internal = feast_solve_interval_with_backend(
            &k,
            &m,
            &interval,
            &config,
            FeastBackend::InternalLu,
            None,
        )
        .unwrap();
        let direct = feast_solve_interval_with_backend(
            &k,
            &m,
            &interval,
            &config,
            FeastBackend::DirectLu,
            None,
        )
        .unwrap();

        let a = sorted(internal.eigenvalues);
        let b = sorted(direct.eigenvalues);
        assert_eq!(a.len(), b.len());
        for i in 0..a.len() {
            assert!((a[i] - b[i]).abs() < 1e-6, "i={i}: {} vs {}", a[i], b[i]);
        }
    }

    #[test]
    #[cfg(feature = "faer-lu")]
    fn direct_lu_backend_exceeds_internal_lu_on_outside_window_tiny_interval() {
        let k = symmetric_pivot_window_case_k(8, 1e-32);
        let m = diagonal_csr(9, |_| 1.0);
        let interval = contour::FeastInterval {
            lambda_min: -1e-32,
            lambda_max: 1e-32,
        };
        let config = FeastConfig {
            m0: 4,
            n_quadrature: 4,
            max_loops: 2,
            seed: 37,
            ..Default::default()
        };

        let internal_err = match feast_solve_interval_with_backend(
            &k,
            &m,
            &interval,
            &config,
            FeastBackend::InternalLu,
            None,
        ) {
            Ok(_) => panic!("internal LU should fail when FEAST contour shifts stay inside the bounded pivot limitation"),
            Err(err) => err,
        };
        assert!(
            internal_err.contains("pivot window") || internal_err.contains("near-zero diagonal"),
            "internal LU should expose bounded-window failure at FEAST level: {internal_err}"
        );

        let direct_err = feast_solve_interval_with_backend(
            &k,
            &m,
            &interval,
            &config,
            FeastBackend::DirectLu,
            None,
        )
        .unwrap_err();
        assert!(
            direct_err.contains("did not converge"),
            "direct backend should report FEAST-level nonconvergence explicitly: {direct_err}"
        );
    }
}
