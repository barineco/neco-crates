#[derive(Debug, Clone)]
pub(crate) struct SymmetricEigenResult {
    pub(crate) eigenvalues: Vec<f64>,
    /// Row-major square matrix whose columns are eigenvectors.
    pub(crate) eigenvectors: Vec<f64>,
}

pub(crate) trait SymmetricDenseKernel {
    fn cholesky_upper(&self, a: &[f64], n: usize) -> Option<Vec<f64>>;
    fn symmetric_eigen(&self, a: &[f64], n: usize) -> SymmetricEigenResult;
    fn lu_solve_multiple(&self, a: &[f64], b: &[f64], n: usize, nrhs: usize) -> Option<Vec<f64>>;
    fn generalized_symmetric_eigen(
        &self,
        k: &[f64],
        m: &[f64],
        n: usize,
    ) -> Option<SymmetricEigenResult>;
    fn symmetric_tridiagonal_eigen(&self, diag: &[f64], offdiag: &[f64]) -> SymmetricEigenResult;
}

pub(crate) struct InternalSymmetricDenseKernel;

static INTERNAL_SYMMETRIC_DENSE_KERNEL: InternalSymmetricDenseKernel = InternalSymmetricDenseKernel;

pub(crate) fn symmetric_dense_kernel() -> &'static dyn SymmetricDenseKernel {
    &INTERNAL_SYMMETRIC_DENSE_KERNEL
}

impl SymmetricDenseKernel for InternalSymmetricDenseKernel {
    fn cholesky_upper(&self, a: &[f64], n: usize) -> Option<Vec<f64>> {
        cholesky_upper(a, n)
    }

    fn symmetric_eigen(&self, a: &[f64], n: usize) -> SymmetricEigenResult {
        jacobi_symmetric_eigen(a, n)
    }

    fn lu_solve_multiple(&self, a: &[f64], b: &[f64], n: usize, nrhs: usize) -> Option<Vec<f64>> {
        lu_solve_multiple(a, b, n, nrhs)
    }

    fn generalized_symmetric_eigen(
        &self,
        k: &[f64],
        m: &[f64],
        n: usize,
    ) -> Option<SymmetricEigenResult> {
        generalized_symmetric_eigen(k, m, n)
    }

    fn symmetric_tridiagonal_eigen(&self, diag: &[f64], offdiag: &[f64]) -> SymmetricEigenResult {
        symmetric_tridiagonal_eigen(diag, offdiag)
    }
}

fn cholesky_upper(a: &[f64], n: usize) -> Option<Vec<f64>> {
    assert_eq!(a.len(), n * n);
    let mut r = vec![0.0; n * n];
    for i in 0..n {
        for j in i..n {
            let mut sum = a[i * n + j];
            for k in 0..i {
                sum -= r[k * n + i] * r[k * n + j];
            }
            if i == j {
                if sum <= 0.0 {
                    return None;
                }
                r[i * n + i] = sum.sqrt();
            } else {
                r[i * n + j] = sum / r[i * n + i];
            }
        }
    }
    Some(r)
}

fn jacobi_symmetric_eigen(a: &[f64], n: usize) -> SymmetricEigenResult {
    assert_eq!(a.len(), n * n);
    let mut mat = a.to_vec();
    let mut vecs = identity(n);
    let base = mat.iter().map(|x| x * x).sum::<f64>().sqrt().max(1.0);
    let tol = 1e-14 * base;
    let max_sweeps = (50 * n.max(1)).max(32);

    for _ in 0..max_sweeps {
        let mut off_norm_sq = 0.0;
        for i in 0..n {
            for j in (i + 1)..n {
                let val = mat[i * n + j];
                off_norm_sq += 2.0 * val * val;
            }
        }
        if off_norm_sq.sqrt() <= tol {
            break;
        }

        for p in 0..n {
            for q in (p + 1)..n {
                let apq = mat[p * n + q];
                if apq.abs() <= tol {
                    continue;
                }

                let app = mat[p * n + p];
                let aqq = mat[q * n + q];
                let tau = (aqq - app) / (2.0 * apq);
                let t = if tau >= 0.0 {
                    1.0 / (tau + (1.0 + tau * tau).sqrt())
                } else {
                    -1.0 / (-tau + (1.0 + tau * tau).sqrt())
                };
                let c = 1.0 / (1.0 + t * t).sqrt();
                let s = t * c;

                for k in 0..n {
                    if k == p || k == q {
                        continue;
                    }
                    let akp = mat[k * n + p];
                    let akq = mat[k * n + q];
                    let new_kp = c * akp - s * akq;
                    let new_kq = s * akp + c * akq;
                    mat[k * n + p] = new_kp;
                    mat[p * n + k] = new_kp;
                    mat[k * n + q] = new_kq;
                    mat[q * n + k] = new_kq;
                }

                let new_pp = c * c * app - 2.0 * s * c * apq + s * s * aqq;
                let new_qq = s * s * app + 2.0 * s * c * apq + c * c * aqq;
                mat[p * n + p] = new_pp;
                mat[q * n + q] = new_qq;
                mat[p * n + q] = 0.0;
                mat[q * n + p] = 0.0;

                for k in 0..n {
                    let vkp = vecs[k * n + p];
                    let vkq = vecs[k * n + q];
                    vecs[k * n + p] = c * vkp - s * vkq;
                    vecs[k * n + q] = s * vkp + c * vkq;
                }
            }
        }
    }

    SymmetricEigenResult {
        eigenvalues: (0..n).map(|i| mat[i * n + i]).collect(),
        eigenvectors: vecs,
    }
}

fn identity(n: usize) -> Vec<f64> {
    let mut out = vec![0.0; n * n];
    for i in 0..n {
        out[i * n + i] = 1.0;
    }
    out
}

fn lu_solve_multiple(a: &[f64], b: &[f64], n: usize, nrhs: usize) -> Option<Vec<f64>> {
    assert_eq!(a.len(), n * n);
    assert_eq!(b.len(), n * nrhs);

    let (lu, pivots) = lu_factorize(a, n)?;
    let mut out = vec![0.0; n * nrhs];
    for rhs_idx in 0..nrhs {
        let mut rhs = vec![0.0; n];
        for row in 0..n {
            rhs[row] = b[row * nrhs + rhs_idx];
        }
        apply_pivots(&mut rhs, &pivots);
        forward_substitute_unit_lower(&lu, &mut rhs, n);
        backward_substitute_upper(&lu, &mut rhs, n)?;
        for row in 0..n {
            out[row * nrhs + rhs_idx] = rhs[row];
        }
    }
    Some(out)
}

fn generalized_symmetric_eigen(k: &[f64], m: &[f64], n: usize) -> Option<SymmetricEigenResult> {
    assert_eq!(k.len(), n * n);
    assert_eq!(m.len(), n * n);

    let r = cholesky_upper(m, n)?;
    let r_inv = invert_upper_triangular(&r, n)?;
    let r_inv_t = transpose(&r_inv, n);
    let standard = matmul(&matmul(&r_inv_t, k, n, n, n), &r_inv, n, n, n);
    let mut eigen = jacobi_symmetric_eigen(&standard, n);
    eigen.eigenvectors = matmul(&r_inv, &eigen.eigenvectors, n, n, n);
    Some(eigen)
}

fn symmetric_tridiagonal_eigen(diag: &[f64], offdiag: &[f64]) -> SymmetricEigenResult {
    let n = diag.len();
    let mut dense = vec![0.0; n * n];
    for i in 0..n {
        dense[i * n + i] = diag[i];
        if i + 1 < n {
            dense[i * n + (i + 1)] = offdiag[i];
            dense[(i + 1) * n + i] = offdiag[i];
        }
    }
    jacobi_symmetric_eigen(&dense, n)
}

fn lu_factorize(a: &[f64], n: usize) -> Option<(Vec<f64>, Vec<usize>)> {
    let mut lu = a.to_vec();
    let mut pivots: Vec<usize> = (0..n).collect();
    for k in 0..n {
        let mut pivot_row = k;
        let mut pivot_abs = lu[k * n + k].abs();
        for row in (k + 1)..n {
            let value = lu[row * n + k].abs();
            if value > pivot_abs {
                pivot_abs = value;
                pivot_row = row;
            }
        }
        if pivot_abs <= 1e-30 {
            return None;
        }
        if pivot_row != k {
            for col in 0..n {
                lu.swap(k * n + col, pivot_row * n + col);
            }
            pivots.swap(k, pivot_row);
        }
        let pivot = lu[k * n + k];
        for row in (k + 1)..n {
            let factor = lu[row * n + k] / pivot;
            lu[row * n + k] = factor;
            for col in (k + 1)..n {
                lu[row * n + col] -= factor * lu[k * n + col];
            }
        }
    }
    Some((lu, pivots))
}

fn apply_pivots(rhs: &mut [f64], pivots: &[usize]) {
    let mut visited = vec![false; pivots.len()];
    for start in 0..pivots.len() {
        if visited[start] || pivots[start] == start {
            continue;
        }
        let mut current = start;
        while !visited[current] {
            visited[current] = true;
            let next = pivots[current];
            if next != current {
                rhs.swap(current, next);
            }
            current = next;
        }
    }
}

fn forward_substitute_unit_lower(lu: &[f64], rhs: &mut [f64], n: usize) {
    for row in 0..n {
        for col in 0..row {
            rhs[row] -= lu[row * n + col] * rhs[col];
        }
    }
}

fn backward_substitute_upper(lu: &[f64], rhs: &mut [f64], n: usize) -> Option<()> {
    for row in (0..n).rev() {
        for col in (row + 1)..n {
            rhs[row] -= lu[row * n + col] * rhs[col];
        }
        let pivot = lu[row * n + row];
        if pivot.abs() <= 1e-30 {
            return None;
        }
        rhs[row] /= pivot;
    }
    Some(())
}

fn invert_upper_triangular(r: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut inv = vec![0.0; n * n];
    for col in 0..n {
        for row in (0..=col).rev() {
            let mut sum = if row == col { 1.0 } else { 0.0 };
            for k in (row + 1)..=col {
                sum -= r[row * n + k] * inv[k * n + col];
            }
            let diag = r[row * n + row];
            if diag.abs() <= 1e-30 {
                return None;
            }
            inv[row * n + col] = sum / diag;
        }
    }
    Some(inv)
}

fn transpose(a: &[f64], n: usize) -> Vec<f64> {
    let mut out = vec![0.0; n * n];
    for row in 0..n {
        for col in 0..n {
            out[col * n + row] = a[row * n + col];
        }
    }
    out
}

fn matmul(a: &[f64], b: &[f64], nrows: usize, inner: usize, ncols: usize) -> Vec<f64> {
    let mut out = vec![0.0; nrows * ncols];
    for row in 0..nrows {
        for k in 0..inner {
            let lhs = a[row * inner + k];
            if lhs.abs() <= 1e-30 {
                continue;
            }
            for col in 0..ncols {
                out[row * ncols + col] += lhs * b[k * ncols + col];
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cholesky_upper_reconstructs_spd_matrix() {
        let a = vec![
            4.0, 2.0, 2.0, //
            2.0, 10.0, 5.0, //
            2.0, 5.0, 9.0,
        ];
        let r = cholesky_upper(&a, 3).expect("SPD matrix");
        let mut recon = [0.0; 9];
        for i in 0..3 {
            for j in 0..3 {
                let mut sum = 0.0;
                for k in 0..3 {
                    sum += r[k * 3 + i] * r[k * 3 + j];
                }
                recon[i * 3 + j] = sum;
            }
        }
        for i in 0..9 {
            assert!(
                (recon[i] - a[i]).abs() < 1e-10,
                "i={i}, recon={}, expected={}",
                recon[i],
                a[i]
            );
        }
    }

    #[test]
    fn jacobi_symmetric_eigen_sorts_can_be_done_externally() {
        let a = vec![
            4.0, 1.0, 0.0, //
            1.0, 3.0, 0.0, //
            0.0, 0.0, 9.0,
        ];
        let eig = jacobi_symmetric_eigen(&a, 3);
        let mut vals = eig.eigenvalues.clone();
        vals.sort_by(f64::total_cmp);
        let expected = [2.381966011250105, 4.618033988749895, 9.0];
        for i in 0..3 {
            assert!(
                (vals[i] - expected[i]).abs() < 1e-10,
                "i={i}, got={}",
                vals[i]
            );
        }
    }

    #[test]
    fn lu_solve_multiple_solves_two_rhs() {
        let a = vec![
            4.0, 1.0, 2.0, //
            1.0, 3.0, 0.0, //
            2.0, 0.0, 5.0,
        ];
        let b = vec![
            7.0, 4.0, 9.0, //
            2.0, 1.0, 3.0,
        ];
        let x = lu_solve_multiple(&a, &b, 3, 2).expect("LU solve");
        let expected = [
            1.2558139534883723,
            0.7441860465116279,
            2.5813953488372094,
            0.41860465116279066,
            -0.3023255813953488,
            0.3023255813953489,
        ];
        for i in 0..expected.len() {
            assert!((x[i] - expected[i]).abs() < 1e-12, "i={i}, got={}", x[i]);
        }
    }

    #[test]
    fn generalized_symmetric_eigen_handles_diagonal_mass() {
        let k = vec![
            4.0, 0.0, //
            0.0, 9.0,
        ];
        let m = vec![
            2.0, 0.0, //
            0.0, 3.0,
        ];
        let eig = generalized_symmetric_eigen(&k, &m, 2).expect("generalized eig");
        let mut vals = eig.eigenvalues;
        vals.sort_by(f64::total_cmp);
        assert!((vals[0] - 2.0).abs() < 1e-12, "got={}", vals[0]);
        assert!((vals[1] - 3.0).abs() < 1e-12, "got={}", vals[1]);
    }
}
