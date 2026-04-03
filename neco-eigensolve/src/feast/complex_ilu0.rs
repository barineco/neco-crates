use crate::c64::C64;

use super::complex_csr::ComplexCsr;

/// Complex ILU(0) preconditioner.
pub struct ComplexIlu0 {
    n: usize,
    row_offsets: Vec<usize>,
    col_indices: Vec<usize>,
    lu_values: Vec<C64>,
    diag_pos: Vec<usize>,
}

impl ComplexIlu0 {
    pub fn new(a: &ComplexCsr) -> Result<Self, String> {
        let n = a.n();
        let mut lu_values = a.values.clone();
        let row_offsets = a.row_offsets.clone();
        let col_indices = a.col_indices.clone();
        let diag_pos = a
            .diagonal_positions()
            .map_err(|err| format!("ILU(0): {err}"))?;

        for i in 1..n {
            let row_start = row_offsets[i];
            let row_end = row_offsets[i + 1];

            for pos_k in row_start..row_end {
                let k = col_indices[pos_k];
                if k >= i {
                    break;
                }

                let diag_k = lu_values[diag_pos[k]];
                let abs2 = diag_k.re * diag_k.re + diag_k.im * diag_k.im;
                if abs2 < 1e-30 {
                    continue;
                }
                let factor = lu_values[pos_k] / diag_k;
                lu_values[pos_k] = factor;

                let k_start = row_offsets[k];
                let k_end = row_offsets[k + 1];
                for pos_j in (pos_k + 1)..row_end {
                    let j = col_indices[pos_j];
                    for (offset, &col_kj) in col_indices[k_start..k_end].iter().enumerate() {
                        let pos_kj = k_start + offset;
                        if col_kj == j {
                            let u_kj = lu_values[pos_kj];
                            lu_values[pos_j] -= factor * u_kj;
                            break;
                        }
                        if col_kj > j {
                            break;
                        }
                    }
                }
            }
        }

        Ok(Self {
            n,
            row_offsets,
            col_indices,
            lu_values,
            diag_pos,
        })
    }

    pub fn solve(&self, rhs: &[C64], out: &mut [C64]) {
        let n = self.n;

        let mut z = rhs.to_vec();
        for i in 0..n {
            let start = self.row_offsets[i];
            let diag = self.diag_pos[i];
            let mut sum = z[i];
            for pos in start..diag {
                let k = self.col_indices[pos];
                sum -= self.lu_values[pos] * z[k];
            }
            z[i] = sum;
        }

        out.copy_from_slice(&z);
        for i in (0..n).rev() {
            let diag = self.diag_pos[i];
            let end = self.row_offsets[i + 1];
            let mut sum = out[i];
            for pos in (diag + 1)..end {
                let j = self.col_indices[pos];
                sum -= self.lu_values[pos] * out[j];
            }
            let d = self.lu_values[diag];
            let abs2 = d.re * d.re + d.im * d.im;
            if abs2 > 1e-30 {
                sum /= d;
            }
            out[i] = sum;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neco_sparse::CsrMat;

    const EPS: f64 = 1e-12;

    fn approx_eq(a: C64, b: C64) -> bool {
        (a.re - b.re).abs() < EPS && (a.im - b.im).abs() < EPS
    }

    fn diagonal_csr(n: usize, diag: &[f64]) -> CsrMat<f64> {
        let offsets: Vec<usize> = (0..=n).collect();
        let indices: Vec<usize> = (0..n).collect();
        CsrMat::try_from_csr_data(n, n, offsets, indices, diag.to_vec()).unwrap()
    }

    /// Diagonal ILU(0) solve returns the exact solution.
    #[test]
    fn diagonal_ilu0_solve() {
        let n = 3;
        let diag_vals = [2.0, 5.0, 3.0];
        let k = diagonal_csr(n, &diag_vals);
        let m = diagonal_csr(n, &[1.0; 3]);

        // z*M - K with z = (1, 0) => diag(-1, -4, -2)
        let z = C64::new(1.0, 0.0);
        let a = ComplexCsr::from_shift(&k, &m, z);

        let ilu = ComplexIlu0::new(&a).expect("ILU(0) construction should succeed");

        // b = [(-1, 0), (-8, 0), (-6, 0)]
        // A = diag(-1, -4, -2)
        // expected x = [1, 2, 3]
        let b = vec![
            C64::new(-1.0, 0.0),
            C64::new(-8.0, 0.0),
            C64::new(-6.0, 0.0),
        ];
        let mut x = vec![C64::zero(); n];
        ilu.solve(&b, &mut x);

        assert!(approx_eq(x[0], C64::new(1.0, 0.0)));
        assert!(approx_eq(x[1], C64::new(2.0, 0.0)));
        assert!(approx_eq(x[2], C64::new(3.0, 0.0)));
    }

    /// Complex diagonal ILU(0) solve returns the exact solution.
    #[test]
    fn complex_diagonal_ilu0_solve() {
        let n = 2;
        // z = (1, 2), K = diag(1, 1), M = diag(1, 1)
        // A = z*M - K = diag((0, 2), (0, 2))
        let k = diagonal_csr(n, &[1.0, 1.0]);
        let m = diagonal_csr(n, &[1.0, 1.0]);
        let z = C64::new(1.0, 2.0);
        let a = ComplexCsr::from_shift(&k, &m, z);

        let ilu = ComplexIlu0::new(&a).unwrap();

        // b = [(0, 2), (0, 4)] → x = [1, 2]
        let b = vec![C64::new(0.0, 2.0), C64::new(0.0, 4.0)];
        let mut x = vec![C64::zero(); n];
        ilu.solve(&b, &mut x);

        assert!(approx_eq(x[0], C64::new(1.0, 0.0)));
        assert!(approx_eq(x[1], C64::new(2.0, 0.0)));
    }
}
