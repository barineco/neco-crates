use neco_sparse::CsrMat;

use crate::c64::C64;

use super::backend::PreparedLinearSolver;
use super::complex_csr::ComplexCsr;
use super::complex_ilu0::ComplexIlu0;

pub(crate) struct GmresShiftedSolver {
    a: ComplexCsr,
    precond: ComplexIlu0,
}

impl GmresShiftedSolver {
    pub(crate) fn new(k_mat: &CsrMat<f64>, m_mat: &CsrMat<f64>, z: C64) -> Result<Self, String> {
        let a = ComplexCsr::from_shift(k_mat, m_mat, z);
        let precond = ComplexIlu0::new(&a)?;
        Ok(Self { a, precond })
    }
}

impl PreparedLinearSolver for GmresShiftedSolver {
    fn solve_block(&self, rhs: &[f64], n: usize, m0: usize, tol: f64) -> Result<Vec<C64>, String> {
        if rhs.len() != n * m0 {
            return Err(format!(
                "rhs length {} does not match n * m0 = {}",
                rhs.len(),
                n * m0
            ));
        }

        let mut result = Vec::with_capacity(n * m0);
        let mut x = vec![C64::zero(); n];

        for j in 0..m0 {
            let col_start = j * n;
            let rhs_c: Vec<C64> = (0..n).map(|i| C64::new(rhs[col_start + i], 0.0)).collect();

            for value in &mut x {
                *value = C64::zero();
            }

            gmres_solve(&self.a, &self.precond, &rhs_c, &mut x, 50, 10, tol);
            result.extend_from_slice(&x);
        }

        Ok(result)
    }
}

/// Right-preconditioned complex GMRES(m).
pub fn gmres_solve(
    a: &ComplexCsr,
    precond: &ComplexIlu0,
    rhs: &[C64],
    x: &mut [C64],
    restart: usize,
    max_restarts: usize,
    tol: f64,
) -> usize {
    let n = a.n();
    let mut total_iters = 0;

    for _restart_idx in 0..max_restarts {
        let mut r = vec![C64::zero(); n];
        a.matvec(x, &mut r);
        for i in 0..n {
            r[i] = rhs[i] - r[i];
        }

        let r_norm = c_norm(&r);
        if r_norm < tol {
            return total_iters;
        }

        let m = restart.min(n);
        let mut v: Vec<Vec<C64>> = Vec::with_capacity(m + 1);
        let mut h = vec![vec![C64::zero(); m]; m + 1];
        let mut g = vec![C64::zero(); m + 1];
        let mut cs = vec![0.0f64; m];
        let mut sn = vec![C64::zero(); m];

        let inv_norm = 1.0 / r_norm;
        v.push(r.iter().map(|&ri| ri * inv_norm).collect());
        g[0] = C64::new(r_norm, 0.0);

        let mut k = 0;
        for j in 0..m {
            k = j;
            total_iters += 1;

            let mut z = vec![C64::zero(); n];
            precond.solve(&v[j], &mut z);
            let mut w = vec![C64::zero(); n];
            a.matvec(&z, &mut w);

            for i in 0..=j {
                h[i][j] = c_dot(&w, &v[i]);
                for idx in 0..n {
                    w[idx] -= h[i][j] * v[i][idx];
                }
            }
            let w_norm = c_norm(&w);
            h[j + 1][j] = C64::new(w_norm, 0.0);

            if w_norm > 1e-14 {
                let inv = 1.0 / w_norm;
                v.push(w.iter().map(|&wi| wi * inv).collect());
            } else {
                v.push(vec![C64::zero(); n]);
            }

            for i in 0..j {
                let tmp = C64::from_real(cs[i] * h[i][j].re) + sn[i] * h[i + 1][j];
                h[i + 1][j] = C64::new(-sn[i].re, sn[i].im) * C64::new(h[i][j].re, 0.0)
                    + C64::new(cs[i], 0.0) * h[i + 1][j];
                h[i][j] = tmp;
            }

            let a_jj = h[j][j];
            let a_j1j = h[j + 1][j];
            let denom =
                (a_jj.re * a_jj.re + a_jj.im * a_jj.im + a_j1j.re * a_j1j.re + a_j1j.im * a_j1j.im)
                    .sqrt();
            if denom > 1e-30 {
                cs[j] = a_jj.norm() / denom;
                sn[j] = a_j1j
                    * C64::new(
                        a_jj.norm() / (a_jj.re * a_jj.re + a_jj.im * a_jj.im).sqrt(),
                        0.0,
                    );
                let r_val = C64::new(denom, 0.0);
                h[j][j] = r_val;
                h[j + 1][j] = C64::zero();
                let tmp = C64::new(cs[j], 0.0) * g[j] + sn[j] * g[j + 1];
                g[j + 1] = C64::new(-cs[j], 0.0) * g[j + 1] + C64::new(sn[j].re, -sn[j].im) * g[j];
                g[j] = tmp;
            }

            let res_est = g[j + 1].norm();
            if res_est < tol {
                k = j;
                break;
            }
        }

        let dim = k + 1;
        let mut y = vec![C64::zero(); dim];
        for i in (0..dim).rev() {
            let mut sum = g[i];
            for j in (i + 1)..dim {
                sum -= h[i][j] * y[j];
            }
            let d = h[i][i];
            if d.norm() > 1e-30 {
                sum /= d;
            }
            y[i] = sum;
        }

        let mut vy = vec![C64::zero(); n];
        for j in 0..dim {
            for i in 0..n {
                vy[i] += y[j] * v[j][i];
            }
        }
        let mut update = vec![C64::zero(); n];
        precond.solve(&vy, &mut update);
        for i in 0..n {
            x[i] += update[i];
        }

        let mut r_check = vec![C64::zero(); n];
        a.matvec(x, &mut r_check);
        for i in 0..n {
            r_check[i] = rhs[i] - r_check[i];
        }
        if c_norm(&r_check) < tol {
            return total_iters;
        }
    }

    total_iters
}

/// Block GMRES: solve m0 right-hand sides sequentially.
#[cfg(test)]
pub(crate) fn solve_complex_block_gmres(
    k_mat: &CsrMat<f64>,
    m_mat: &CsrMat<f64>,
    z: C64,
    rhs: &[f64],
    n: usize,
    m0: usize,
    tol: f64,
) -> Result<Vec<C64>, String> {
    GmresShiftedSolver::new(k_mat, m_mat, z)?.solve_block(rhs, n, m0, tol)
}

/// Complex inner product: <a, b> = sum(conj(a_i) * b_i).
#[inline]
pub(crate) fn c_dot(a: &[C64], b: &[C64]) -> C64 {
    a.iter()
        .zip(b.iter())
        .map(|(&ai, &bi)| C64::new(ai.re, -ai.im) * bi)
        .fold(C64::zero(), |acc, v| acc + v)
}

/// L2 norm of a complex vector.
#[inline]
fn c_norm(v: &[C64]) -> f64 {
    v.iter()
        .map(|&vi| vi.re * vi.re + vi.im * vi.im)
        .sum::<f64>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diagonal_csr(n: usize, diag: &[f64]) -> CsrMat<f64> {
        let offsets: Vec<usize> = (0..=n).collect();
        let indices: Vec<usize> = (0..n).collect();
        CsrMat::try_from_csr_data(n, n, offsets, indices, diag.to_vec()).unwrap()
    }

    #[test]
    fn gmres_diagonal_system() {
        let k = diagonal_csr(3, &[1.0, 2.0, 3.0]);
        let m = diagonal_csr(3, &[1.0, 1.0, 1.0]);
        let z = C64::new(2.5, 1.0);

        let rhs = vec![1.0f64; 3];
        let result = solve_complex_block_gmres(&k, &m, z, &rhs, 3, 1, 1e-10).unwrap();

        // (z*M - K) = diag(z-1, z-2, z-3) = diag(1.5+i, 0.5+i, -0.5+i)
        // x = 1 / (z - k_i)
        let expected = [
            C64::new(1.0, 0.0) / C64::new(1.5, 1.0),
            C64::new(1.0, 0.0) / C64::new(0.5, 1.0),
            C64::new(1.0, 0.0) / C64::new(-0.5, 1.0),
        ];
        for i in 0..3 {
            let diff = result[i] - expected[i];
            let err = (diff.re * diff.re + diff.im * diff.im).sqrt();
            assert!(err < 1e-8, "x[{i}]: err={err}");
        }
    }
}
