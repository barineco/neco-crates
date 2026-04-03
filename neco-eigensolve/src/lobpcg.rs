use neco_sparse::CsrMat;

use crate::dense::symmetric_dense_kernel;
use crate::DenseMatrix;

pub trait Preconditioner {
    fn apply(&self, residuals: &DenseMatrix) -> DenseMatrix;
}

pub struct JacobiPreconditioner {
    diag_inv: Vec<f64>,
}

impl JacobiPreconditioner {
    pub fn new(k_mat: &CsrMat<f64>) -> Self {
        let n = k_mat.nrows();
        let mut diag_inv = vec![1.0; n];
        for (i, diag_inv_i) in diag_inv.iter_mut().enumerate() {
            let row = k_mat.row(i);
            for (&col, &val) in row.col_indices().iter().zip(row.values().iter()) {
                if col == i && val.abs() > 1e-20 {
                    *diag_inv_i = 1.0 / val;
                    break;
                }
            }
        }
        Self { diag_inv }
    }

    fn apply_block(&self, residuals: &DenseMatrix) -> DenseMatrix {
        let mut out = residuals.clone();
        for j in 0..out.ncols() {
            for (value, &diag_inv) in out.column_mut(j).iter_mut().zip(&self.diag_inv) {
                *value *= diag_inv;
            }
        }
        out
    }
}

impl Preconditioner for JacobiPreconditioner {
    fn apply(&self, residuals: &DenseMatrix) -> DenseMatrix {
        self.apply_block(residuals)
    }
}

pub struct LobpcgResult {
    pub eigenvalues: Vec<f64>,
    /// Rows: modes, columns: DOFs
    pub eigenvectors: DenseMatrix,
    pub iterations: usize,
}

use crate::spmv;

fn dot(x: &[f64], y: &[f64]) -> f64 {
    x.iter().zip(y).map(|(a, b)| a * b).sum()
}

fn norm(x: &[f64]) -> f64 {
    dot(x, x).sqrt()
}

fn lobpcg_spmv(a: &CsrMat<f64>, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.nrows()];
    spmv::spmv_dispatch(&mut y, a, x);
    y
}

fn lobpcg_block_spmv_into(y: &mut DenseMatrix, a: &CsrMat<f64>, x: &DenseMatrix) {
    spmv::block_spmv_dispatch(y.as_mut_slice(), a, x.as_slice(), a.nrows(), x.ncols());
}

fn lobpcg_block_spmv_into_prefix(
    y: &mut DenseMatrix,
    a: &CsrMat<f64>,
    x: &DenseMatrix,
    ncols: usize,
) {
    let nrows = a.nrows();
    spmv::block_spmv_dispatch(
        &mut y.as_mut_slice()[..nrows * ncols],
        a,
        &x.as_slice()[..nrows * ncols],
        nrows,
        ncols,
    );
}

fn lobpcg_block_spmv_into_prefix_block(
    y: &mut DenseMatrix,
    a: &CsrMat<f64>,
    x: &DenseMatrix,
    ncols: usize,
) {
    if ncols == 0 {
        return;
    }
    let nrows = a.nrows();
    let mut tmp = vec![0.0; nrows * ncols];
    spmv::block_spmv_dispatch(&mut tmp, a, &x.as_slice()[..nrows * ncols], nrows, ncols);
    for col in 0..ncols {
        y.column_mut(col)
            .copy_from_slice(&tmp[col * nrows..(col + 1) * nrows]);
    }
}

fn orthonormalize_b(
    vecs: &mut DenseMatrix,
    m_mat: &CsrMat<f64>,
    m_vecs_buf: &mut DenseMatrix,
) -> bool {
    let cols = vecs.ncols();
    lobpcg_block_spmv_into_prefix_block(m_vecs_buf, m_mat, vecs, cols);

    let mut gram = vec![0.0; cols * cols];
    for i in 0..cols {
        for j in i..cols {
            let mut dot = 0.0;
            for k in 0..vecs.nrows() {
                dot += vecs[(k, i)] * m_vecs_buf.column(j)[k];
            }
            gram[i * cols + j] = dot;
            gram[j * cols + i] = dot;
        }
    }

    match symmetric_dense_kernel().cholesky_upper(&gram, cols) {
        Some(r) => {
            let nrows = vecs.nrows();
            let mut q_buf = vec![0.0; nrows * cols];
            for j in 0..cols {
                let mut coeffs = vec![0.0; cols];
                coeffs[j] = 1.0;
                for i in (0..cols).rev() {
                    let mut sum = coeffs[i];
                    for p in (i + 1)..cols {
                        sum -= r[i * cols + p] * coeffs[p];
                    }
                    coeffs[i] = sum / r[i * cols + i];
                }
                for k in 0..nrows {
                    let mut value = 0.0;
                    for p in 0..cols {
                        value += vecs[(k, p)] * coeffs[p];
                    }
                    q_buf[j * nrows + k] = value;
                }
            }
            *vecs = DenseMatrix::from_column_slice(nrows, cols, &q_buf);
            true
        }
        None => false,
    }
}

fn deflate_dc(x: &mut DenseMatrix, m_mat: &CsrMat<f64>) {
    let n = x.nrows();
    let ones = vec![1.0; n];
    let m_ones = lobpcg_spmv(m_mat, &ones);
    let norm = dot(&ones, &m_ones).sqrt();
    let dc: Vec<f64> = ones.iter().map(|value| value / norm).collect();
    let m_dc: Vec<f64> = m_ones.iter().map(|value| value / norm).collect();

    for j in 0..x.ncols() {
        let dot = dot(x.column(j), &m_dc);
        for i in 0..n {
            x[(i, j)] -= dot * dc[i];
        }
    }
}

fn select_columns(mat: &DenseMatrix, indices: &[usize]) -> DenseMatrix {
    mat.select_columns(indices)
}

struct SearchSpaceInputs<'a> {
    x: &'a DenseMatrix,
    w: &'a DenseMatrix,
    p: &'a DenseMatrix,
    kx_buf: &'a DenseMatrix,
    kw_buf: &'a DenseMatrix,
    kp: &'a DenseMatrix,
    mx_buf: &'a DenseMatrix,
    mw_buf: &'a DenseMatrix,
    mp_buf: &'a DenseMatrix,
}

fn assemble_search_space(
    inputs: SearchSpaceInputs<'_>,
    p_cols: usize,
    ma: usize,
) -> (DenseMatrix, DenseMatrix, DenseMatrix) {
    let SearchSpaceInputs {
        x,
        w,
        p,
        kx_buf,
        kw_buf,
        kp,
        mx_buf,
        mw_buf,
        mp_buf,
    } = inputs;
    if p_cols > 0 {
        let total = x.ncols() + ma + p_cols;
        let mut s = DenseMatrix::zeros(x.nrows(), total);
        let mut ks = DenseMatrix::zeros(x.nrows(), total);
        let mut ms = DenseMatrix::zeros(x.nrows(), total);

        s.copy_columns_from(0, x, 0, x.ncols());
        s.copy_columns_from(x.ncols(), w, 0, ma);
        s.copy_columns_from(x.ncols() + ma, p, 0, p_cols);

        ks.copy_columns_from(0, kx_buf, 0, x.ncols());
        ks.copy_columns_from(x.ncols(), kw_buf, 0, ma);
        ks.copy_columns_from(x.ncols() + ma, kp, 0, p_cols);

        ms.copy_columns_from(0, mx_buf, 0, x.ncols());
        ms.copy_columns_from(x.ncols(), mw_buf, 0, ma);
        ms.copy_columns_from(x.ncols() + ma, mp_buf, 0, p_cols);

        (s, ks, ms)
    } else {
        let total = x.ncols() + ma;
        let mut s = DenseMatrix::zeros(x.nrows(), total);
        let mut ks = DenseMatrix::zeros(x.nrows(), total);
        let mut ms = DenseMatrix::zeros(x.nrows(), total);

        s.copy_columns_from(0, x, 0, x.ncols());
        s.copy_columns_from(x.ncols(), w, 0, ma);

        ks.copy_columns_from(0, kx_buf, 0, x.ncols());
        ks.copy_columns_from(x.ncols(), kw_buf, 0, ma);

        ms.copy_columns_from(0, mx_buf, 0, x.ncols());
        ms.copy_columns_from(x.ncols(), mw_buf, 0, ma);

        (s, ks, ms)
    }
}

/// Solver configuration.
#[derive(Debug, Clone)]
pub struct LobpcgConfig {
    pub n_modes: usize,
    pub tol: f64,
    pub max_iter: usize,
    /// Deflate the constant (DC) mode from the search space.
    /// Appropriate for FEM (skip rigid-body modes), but should be disabled
    /// for problems where near-zero eigenvalues are informative (e.g., graph Laplacians).
    pub deflate_dc: bool,
}

impl LobpcgConfig {
    pub fn new(n_modes: usize, tol: f64, max_iter: usize) -> Self {
        Self {
            n_modes,
            tol,
            max_iter,
            deflate_dc: true,
        }
    }
}

/// Solve with DC deflation enabled (default, backwards-compatible).
pub fn lobpcg(
    k_mat: &CsrMat<f64>,
    m_mat: &CsrMat<f64>,
    n_modes: usize,
    tol: f64,
    max_iter: usize,
    precond: &dyn Preconditioner,
) -> LobpcgResult {
    lobpcg_with_progress(k_mat, m_mat, n_modes, tol, max_iter, precond, |_, _| {})
}

/// Solve with explicit configuration.
pub fn lobpcg_configured(
    k_mat: &CsrMat<f64>,
    m_mat: &CsrMat<f64>,
    config: &LobpcgConfig,
    precond: &dyn Preconditioner,
    on_progress: impl FnMut(usize, usize),
) -> LobpcgResult {
    lobpcg_core(
        k_mat,
        m_mat,
        precond,
        on_progress,
        LobpcgCoreConfig {
            n_modes: config.n_modes,
            tol: config.tol,
            max_iter: config.max_iter,
            do_deflate_dc: config.deflate_dc,
        },
    )
}

pub fn lobpcg_with_progress(
    k_mat: &CsrMat<f64>,
    m_mat: &CsrMat<f64>,
    n_modes: usize,
    tol: f64,
    max_iter: usize,
    precond: &dyn Preconditioner,
    on_progress: impl FnMut(usize, usize),
) -> LobpcgResult {
    lobpcg_core(
        k_mat,
        m_mat,
        precond,
        on_progress,
        LobpcgCoreConfig {
            n_modes,
            tol,
            max_iter,
            do_deflate_dc: true,
        },
    )
}

struct LobpcgCoreConfig {
    n_modes: usize,
    tol: f64,
    max_iter: usize,
    do_deflate_dc: bool,
}

fn symmetrize_in_place(mat: &mut DenseMatrix) {
    for i in 0..mat.nrows() {
        for j in (i + 1)..mat.ncols() {
            let value = 0.5 * (mat[(i, j)] + mat[(j, i)]);
            mat[(i, j)] = value;
            mat[(j, i)] = value;
        }
    }
}

fn lobpcg_core(
    k_mat: &CsrMat<f64>,
    m_mat: &CsrMat<f64>,
    precond: &dyn Preconditioner,
    mut on_progress: impl FnMut(usize, usize),
    config: LobpcgCoreConfig,
) -> LobpcgResult {
    let n = k_mat.nrows();
    let max_subspace = n.saturating_sub(1).max(1);
    let m = (config.n_modes + 5).min(max_subspace);
    let _ = &mut on_progress;

    let mut x = DenseMatrix::zeros(n, m);
    for j in 0..m {
        for i in 0..n {
            x[(i, j)] = (((i + j * 997) as f64 * 0.618033988749895) % 1.0) - 0.5;
        }
    }

    if config.do_deflate_dc {
        deflate_dc(&mut x, m_mat);
    }

    let mut orth_buf = DenseMatrix::zeros(n, m);
    assert!(
        orthonormalize_b(&mut x, m_mat, &mut orth_buf),
        "M-orthogonalization of initial vectors failed"
    );

    let mut kx_buf = DenseMatrix::zeros(n, m);
    lobpcg_block_spmv_into(&mut kx_buf, k_mat, &x);
    let mut lambda = vec![0.0; m];
    for (j, lambda_j) in lambda.iter_mut().enumerate().take(m) {
        *lambda_j = dot(x.column(j), kx_buf.column(j));
    }

    let mut p = DenseMatrix::zeros(n, m);
    let mut kp = DenseMatrix::zeros(n, m);
    let mut p_cols = 0usize;
    let mut converged = vec![false; m];
    let mut iter = 0;

    let mut mx_buf = DenseMatrix::zeros(n, m);
    let mut kw_buf = DenseMatrix::zeros(n, m);
    let mut mw_buf = DenseMatrix::zeros(n, m);
    let mut mp_buf = DenseMatrix::zeros(n, m);
    for iteration in 0..config.max_iter {
        iter = iteration + 1;

        lobpcg_block_spmv_into(&mut kx_buf, k_mat, &x);
        lobpcg_block_spmv_into(&mut mx_buf, m_mat, &x);
        let mut r = DenseMatrix::zeros(n, m);
        for j in 0..m {
            for i in 0..n {
                r[(i, j)] = kx_buf[(i, j)] - lambda[j] * mx_buf[(i, j)];
            }
        }

        let mut all_converged = true;
        let mut n_active = 0;
        for j in 0..m {
            let rnorm = norm(r.column(j));
            let lnorm = lambda[j].abs().max(1e-15);
            converged[j] = rnorm / lnorm < config.tol;
            if !converged[j] {
                all_converged = false;
                n_active += 1;
            }
        }

        if all_converged || n_active == 0 {
            break;
        }

        let active_idx: Vec<usize> = (0..m).filter(|&j| !converged[j]).collect();
        let ma = active_idx.len();

        let r_active = select_columns(&r, &active_idx);

        let mut w = precond.apply(&r_active);
        if config.do_deflate_dc {
            deflate_dc(&mut w, m_mat);
        }

        lobpcg_block_spmv_into_prefix(&mut mw_buf, m_mat, &w, w.ncols());
        let mw_active = mw_buf.select_columns(&(0..w.ncols()).collect::<Vec<_>>());
        let overlap = x.transpose_mul(&mw_active);
        let x_overlap = x.mul(&overlap);
        for col in 0..w.ncols() {
            for row in 0..n {
                w[(row, col)] -= x_overlap[(row, col)];
            }
        }

        if !orthonormalize_b(&mut w, m_mat, &mut orth_buf) {
            for j in 0..w.ncols() {
                let mwj = lobpcg_spmv(m_mat, w.column(j));
                let column_norm = dot(w.column(j), &mwj).sqrt();
                if column_norm > 1e-15 {
                    for value in w.column_mut(j) {
                        *value /= column_norm;
                    }
                }
            }
        }

        lobpcg_block_spmv_into_prefix(&mut kw_buf, k_mat, &w, w.ncols());
        lobpcg_block_spmv_into_prefix(&mut mw_buf, m_mat, &w, w.ncols());
        let active_p_cols = active_idx.len().min(p_cols);
        if p_cols > 0 {
            lobpcg_block_spmv_into_prefix(&mut mp_buf, m_mat, &p, active_p_cols);
        }
        let (s, ks, ms) = assemble_search_space(
            SearchSpaceInputs {
                x: &x,
                w: &w,
                p: &p,
                kx_buf: &kx_buf,
                kw_buf: &kw_buf,
                kp: &kp,
                mx_buf: &mx_buf,
                mw_buf: &mw_buf,
                mp_buf: &mp_buf,
            },
            active_p_cols,
            ma,
        );

        let mut gram_a = s.transpose_mul(&ks);
        let mut gram_b = s.transpose_mul(&ms);
        symmetrize_in_place(&mut gram_a);
        symmetrize_in_place(&mut gram_b);

        if let Some(eigen) = symmetric_dense_kernel().generalized_symmetric_eigen(
            &gram_a.to_row_major(),
            &gram_b.to_row_major(),
            gram_a.nrows(),
        ) {
            let eigenvectors =
                DenseMatrix::from_row_major(gram_a.nrows(), gram_a.ncols(), &eigen.eigenvectors);

            let mut eig_pairs: Vec<(f64, usize)> = eigen
                .eigenvalues
                .iter()
                .enumerate()
                .map(|(idx, &value)| (value, idx))
                .collect();
            eig_pairs.sort_by(|a, b| a.0.total_cmp(&b.0));

            let take = m.min(eig_pairs.len());
            for j in 0..take {
                lambda[j] = eig_pairs[j].0;
            }

            let mut c_mat = DenseMatrix::zeros(s.ncols(), take);
            for (j, pair) in eig_pairs.iter().take(take).enumerate() {
                c_mat.set_column(j, eigenvectors.column(pair.1));
            }

            let x_old = x.clone();
            let x_new = s.mul(&c_mat);
            // p = x_new - x_old
            for col in 0..take {
                for row in 0..n {
                    p[(row, col)] = x_new[(row, col)] - x_old[(row, col)];
                }
            }
            x = x_new;
            lobpcg_block_spmv_into_prefix(&mut kp, k_mat, &p, take);
            p_cols = take;
        } else {
            // Fallback: gram_b is not positive-definite, use eigendecomposition
            let n_sub = gram_b.nrows();
            let eig_b = symmetric_dense_kernel().symmetric_eigen(&gram_b.to_row_major(), n_sub);
            let eig_b_vecs = DenseMatrix::from_row_major(n_sub, n_sub, &eig_b.eigenvectors);

            let lambda_max = eig_b.eigenvalues.iter().copied().fold(0.0f64, f64::max);
            let tol_eig = n_sub as f64 * f64::EPSILON * lambda_max;

            let keep: Vec<usize> = eig_b
                .eigenvalues
                .iter()
                .enumerate()
                .filter(|(_, &v)| v > tol_eig)
                .map(|(i, _)| i)
                .collect();
            let rank = keep.len();

            if rank == 0 {
                p = DenseMatrix::zeros(n, 0);
                kp = DenseMatrix::zeros(n, 0);
                continue;
            }

            // Q_r: columns of eigenvectors corresponding to kept eigenvalues
            let q_r = DenseMatrix::from_fn(n_sub, rank, |r, c| eig_b_vecs[(r, keep[c])]);

            // T = diag(1/sqrt(lambda_keep)) * Q_r^T  (rank x n_sub)
            let mut t_mat = DenseMatrix::zeros(rank, n_sub);
            for (i, &keep_idx) in keep.iter().enumerate().take(rank) {
                let scale = 1.0 / eig_b.eigenvalues[keep_idx].sqrt();
                for col in 0..n_sub {
                    t_mat[(i, col)] = q_r[(col, i)] * scale;
                }
            }

            let mut reduced_a = t_mat.mul(&gram_a).mul(&t_mat.transpose());
            symmetrize_in_place(&mut reduced_a);
            let eig_reduced =
                symmetric_dense_kernel().symmetric_eigen(&reduced_a.to_row_major(), rank);
            let eig_reduced_vecs =
                DenseMatrix::from_row_major(rank, rank, &eig_reduced.eigenvectors);

            let mut eig_pairs: Vec<(f64, usize)> = eig_reduced
                .eigenvalues
                .iter()
                .enumerate()
                .map(|(idx, &value)| (value, idx))
                .collect();
            eig_pairs.sort_by(|a, b| a.0.total_cmp(&b.0));

            let take = m.min(rank);

            // Back-transform: c_j = T^T * z_j
            let mut c_mat = DenseMatrix::zeros(n_sub, take);
            for (j, pair) in eig_pairs.iter().take(take).enumerate() {
                let c = t_mat
                    .transpose()
                    .mul_vector(eig_reduced_vecs.column(pair.1));
                c_mat.set_column(j, &c);
            }

            let x_new = s.mul(&c_mat);

            if take == m {
                for j in 0..take {
                    lambda[j] = eig_pairs[j].0;
                }
                for col in 0..take {
                    for row in 0..n {
                        p[(row, col)] = x_new[(row, col)] - x[(row, col)];
                    }
                }
                lobpcg_block_spmv_into_prefix(&mut kp, k_mat, &p, take);
                x = x_new;
                p_cols = take;
            } else {
                // Rank-deficient: partial update with P reset
                for j in 0..take {
                    lambda[j] = eig_pairs[j].0;
                }
                x.copy_columns_from(0, &x_new, 0, take);
                if !orthonormalize_b(&mut x, m_mat, &mut orth_buf) {
                    p_cols = 0;
                    continue;
                }
                lobpcg_block_spmv_into(&mut kx_buf, k_mat, &x);
                for (j, lambda_j) in lambda.iter_mut().enumerate().take(m) {
                    *lambda_j = dot(x.column(j), kx_buf.column(j));
                }
                p_cols = 0;
            }
        }

        if iteration % 10 == 9 {
            if config.do_deflate_dc {
                deflate_dc(&mut x, m_mat);
            }
            if !orthonormalize_b(&mut x, m_mat, &mut orth_buf) {
                p_cols = 0;
            }
        }
    }

    let _ = orthonormalize_b(&mut x, m_mat, &mut orth_buf);

    lobpcg_block_spmv_into(&mut kx_buf, k_mat, &x);
    for (j, lambda_j) in lambda.iter_mut().enumerate().take(m) {
        *lambda_j = dot(x.column(j), kx_buf.column(j));
    }

    let mut pairs: Vec<(f64, usize)> = lambda.iter().enumerate().map(|(i, &v)| (v, i)).collect();
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));

    let take = config.n_modes.min(pairs.len());
    let eigenvalues: Vec<f64> = pairs[..take].iter().map(|pair| pair.0).collect();
    let mut eigenvectors = DenseMatrix::zeros(take, n);
    for (mode_idx, &(_, col_idx)) in pairs[..take].iter().enumerate() {
        for i in 0..n {
            eigenvectors[(mode_idx, i)] = x[(i, col_idx)];
        }
    }

    LobpcgResult {
        eigenvalues,
        eigenvectors,
        iterations: iter,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neco_sparse::CooMat;

    fn diag(values: &[f64]) -> CsrMat<f64> {
        let mut coo = CooMat::new(values.len(), values.len());
        for (i, value) in values.iter().enumerate() {
            coo.push(i, i, *value);
        }
        CsrMat::from(&coo)
    }

    fn permuted_diag(values: &[f64], perm: &[usize]) -> CsrMat<f64> {
        let mut coo = CooMat::new(values.len(), values.len());
        for (i, &src) in perm.iter().enumerate() {
            coo.push(i, i, values[src]);
        }
        CsrMat::from(&coo)
    }

    fn row_as_vec(mat: &DenseMatrix, row: usize) -> Vec<f64> {
        let n = mat.ncols();
        let mut v = vec![0.0; n];
        for i in 0..n {
            v[i] = mat[(row, i)];
        }
        v
    }

    fn csr_matvec(a: &CsrMat<f64>, x: &[f64]) -> Vec<f64> {
        let mut y = vec![0.0; a.nrows()];
        for (i, y_i) in y.iter_mut().enumerate().take(a.nrows()) {
            let row = a.row(i);
            let mut sum = 0.0;
            for (&col, &val) in row.col_indices().iter().zip(row.values().iter()) {
                sum += val * x[col];
            }
            *y_i = sum;
        }
        y
    }

    fn residual_norm(k: &CsrMat<f64>, m: &CsrMat<f64>, lambda: f64, x: &[f64]) -> f64 {
        let kx = csr_matvec(k, x);
        let mx = csr_matvec(m, x);
        let mut num_sq = 0.0;
        let mut den_sq = 0.0;
        for i in 0..x.len() {
            let diff = kx[i] - lambda * mx[i];
            num_sq += diff * diff;
            den_sq += kx[i] * kx[i];
        }
        num_sq.sqrt() / den_sq.sqrt().max(1e-15)
    }

    fn max_m_orth_error(eigenvectors: &DenseMatrix, m: &CsrMat<f64>) -> f64 {
        let n_modes = eigenvectors.nrows();
        let mut max_err: f64 = 0.0;
        for i in 0..n_modes {
            let xi = row_as_vec(eigenvectors, i);
            let m_xi = csr_matvec(m, &xi);
            for j in 0..n_modes {
                let xj = row_as_vec(eigenvectors, j);
                let val = dot(&xj, &m_xi);
                let target = if i == j { 1.0 } else { 0.0 };
                max_err = max_err.max((val - target).abs());
            }
        }
        max_err
    }

    fn sort_values(mut values: Vec<f64>) -> Vec<f64> {
        values.sort_by(f64::total_cmp);
        values
    }

    #[test]
    fn jacobi_preconditioner_uses_inverse_diagonal() {
        let k = diag(&[2.0, 4.0]);
        let precond = JacobiPreconditioner::new(&k);
        let r = DenseMatrix::from_row_slice(2, 1, &[2.0, 8.0]);
        let w = precond.apply(&r);
        assert!((w[(0, 0)] - 1.0).abs() < 1e-12);
        assert!((w[(1, 0)] - 2.0).abs() < 1e-12);
    }

    #[test]
    fn lobpcg_smoke_test_returns_requested_mode_count() {
        let vals: Vec<f64> = (1..=30).map(|i| i as f64).collect();
        let k = diag(&vals);
        let m = CsrMat::identity(30);
        let precond = JacobiPreconditioner::new(&k);
        let result = lobpcg(&k, &m, 2, 1e-8, 200, &precond);
        assert_eq!(result.eigenvalues.len(), 2);
        assert_eq!(result.eigenvectors.nrows(), 2);
        assert_eq!(result.eigenvectors.ncols(), 30);
        assert!(result.eigenvalues.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn lobpcg_solves_diagonal_generalized_problem() {
        // n=30 to ensure 3m < n (LOBPCG requires subspace [X,W,P] fits in R^n).
        // n=12 with m=7 gives 3*7=21 > 12 → rank-deficient Gram matrix.
        let vals: Vec<f64> = (1..=30).map(|i| i as f64).collect();
        let k = diag(&vals);
        let m = CsrMat::identity(30);
        let precond = JacobiPreconditioner::new(&k);
        let config = LobpcgConfig {
            n_modes: 2,
            tol: 1e-8,
            max_iter: 200,
            deflate_dc: false,
        };
        let result = lobpcg_configured(&k, &m, &config, &precond, |_, _| {});
        assert!(
            (result.eigenvalues[0] - 1.0).abs() < 1e-6,
            "got {}",
            result.eigenvalues[0]
        );
        assert!(
            (result.eigenvalues[1] - 2.0).abs() < 1e-6,
            "got {}",
            result.eigenvalues[1]
        );
    }

    #[test]
    fn lobpcg_rank_deficient_fallback_converges() {
        // n=12, n_modes=2 → m=min(7,11)=7
        // Subspace S=[X(7),W,P] can have up to 21 columns > 12
        // → gram_b becomes rank deficient → fallback path is triggered
        let vals: Vec<f64> = (1..=12).map(|i| i as f64).collect();
        let k = diag(&vals);
        let m = CsrMat::identity(12);
        let precond = JacobiPreconditioner::new(&k);
        let config = LobpcgConfig {
            n_modes: 2,
            tol: 1e-8,
            max_iter: 200,
            deflate_dc: false,
        };
        let result = lobpcg_configured(&k, &m, &config, &precond, |_, _| {});
        assert!(
            (result.eigenvalues[0] - 1.0).abs() < 1e-6,
            "got {}",
            result.eigenvalues[0]
        );
        assert!(
            (result.eigenvalues[1] - 2.0).abs() < 1e-6,
            "got {}",
            result.eigenvalues[1]
        );
        assert!(
            result.iterations < 200,
            "did not converge: {} iterations",
            result.iterations
        );
    }

    #[test]
    fn lobpcg_residual_and_m_orthogonality_are_small() {
        let vals: Vec<f64> = (1..=40).map(|i| i as f64).collect();
        let k = diag(&vals);
        let m = CsrMat::identity(vals.len());
        let precond = JacobiPreconditioner::new(&k);
        let config = LobpcgConfig {
            n_modes: 3,
            tol: 1e-9,
            max_iter: 200,
            deflate_dc: false,
        };
        let result = lobpcg_configured(&k, &m, &config, &precond, |_, _| {});

        let expected = [1.0, 2.0, 3.0];
        for (i, &e) in expected.iter().enumerate() {
            assert!(
                (result.eigenvalues[i] - e).abs() < 1e-6,
                "eig[{i}]={}, expected={e}",
                result.eigenvalues[i]
            );
        }

        let orth_err = max_m_orth_error(&result.eigenvectors, &m);
        assert!(orth_err < 1e-6, "orth_err={orth_err}");

        for mode in 0..result.eigenvalues.len() {
            let x = row_as_vec(&result.eigenvectors, mode);
            let res = residual_norm(&k, &m, result.eigenvalues[mode], &x);
            assert!(res < 1e-8, "mode={mode}, residual={res}");
        }
    }

    #[test]
    fn lobpcg_permutation_similarity_keeps_eigenvalues() {
        let vals: Vec<f64> = (1..=50).map(|i| i as f64).collect();
        let perm: Vec<usize> = (0..vals.len()).rev().collect();
        let k = diag(&vals);
        let kp = permuted_diag(&vals, &perm);
        let m = CsrMat::identity(vals.len());
        let precond_k = JacobiPreconditioner::new(&k);
        let precond_kp = JacobiPreconditioner::new(&kp);
        let config = LobpcgConfig {
            n_modes: 4,
            tol: 1e-9,
            max_iter: 200,
            deflate_dc: false,
        };

        let base = lobpcg_configured(&k, &m, &config, &precond_k, |_, _| {});
        let permuted = lobpcg_configured(&kp, &m, &config, &precond_kp, |_, _| {});
        let a = sort_values(base.eigenvalues);
        let b = sort_values(permuted.eigenvalues);

        for i in 0..a.len() {
            assert!((a[i] - b[i]).abs() < 1e-6, "i={i}, {} vs {}", a[i], b[i]);
        }
    }

    #[test]
    fn lobpcg_deflate_dc_false_keeps_near_zero_mode() {
        let n = 32usize;
        let mut coo = CooMat::new(n, n);
        for i in 0..n {
            let mut diag = 0.0;
            if i > 0 {
                coo.push(i, i - 1, -1.0);
                diag += 1.0;
            }
            if i + 1 < n {
                coo.push(i, i + 1, -1.0);
                diag += 1.0;
            }
            coo.push(i, i, diag);
        }
        let k = CsrMat::from(&coo);
        let m = CsrMat::identity(n);
        let precond = JacobiPreconditioner::new(&k);
        let config = LobpcgConfig {
            n_modes: 2,
            tol: 1e-8,
            max_iter: 300,
            deflate_dc: false,
        };
        let result = lobpcg_configured(&k, &m, &config, &precond, |_, _| {});
        let values = sort_values(result.eigenvalues);
        assert!(
            values[0].abs() < 1e-6,
            "first eigenvalue should be near zero"
        );
    }
}
