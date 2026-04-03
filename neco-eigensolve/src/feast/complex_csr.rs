use neco_sparse::CsrMat;

use crate::c64::C64;

use super::internal_lu::{diagonal_positions, ShiftedCsrData};

/// Complex CSR matrix.
pub struct ComplexCsr {
    pub(crate) n: usize,
    pub(crate) row_offsets: Vec<usize>,
    pub(crate) col_indices: Vec<usize>,
    pub(crate) values: Vec<C64>,
}

impl ComplexCsr {
    /// Build (z*M - K) from real K, M and complex shift z.
    pub fn from_shift(k: &CsrMat<f64>, m: &CsrMat<f64>, z: C64) -> Self {
        let shifted = ShiftedCsrData::from_shift(k, m, z);
        Self {
            n: shifted.n,
            row_offsets: shifted.row_offsets,
            col_indices: shifted.col_indices,
            values: shifted.values,
        }
    }

    /// y = A * x
    pub(crate) fn matvec(&self, x: &[C64], y: &mut [C64]) {
        for (i, yi) in y.iter_mut().enumerate().take(self.n) {
            let start = self.row_offsets[i];
            let end = self.row_offsets[i + 1];
            let mut sum = C64::zero();
            for pos in start..end {
                sum += self.values[pos] * x[self.col_indices[pos]];
            }
            *yi = sum;
        }
    }

    pub(crate) fn n(&self) -> usize {
        self.n
    }

    pub(crate) fn diagonal_positions(&self) -> Result<Vec<usize>, String> {
        diagonal_positions(self.n, &self.row_offsets, &self.col_indices)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-14;

    fn approx_eq(a: C64, b: C64) -> bool {
        (a.re - b.re).abs() < EPS && (a.im - b.im).abs() < EPS
    }

    /// Verify (z*M - K) for diagonal K, M with z = 1+2i.
    #[test]
    fn from_shift_and_matvec_diagonal() {
        // K = [[1, 0], [0, 2]]
        let k = CsrMat::try_from_csr_data(2, 2, vec![0, 1, 2], vec![0, 1], vec![1.0, 2.0]).unwrap();
        // M = [[3, 0], [0, 4]]
        let m = CsrMat::try_from_csr_data(2, 2, vec![0, 1, 2], vec![0, 1], vec![3.0, 4.0]).unwrap();

        let z = C64::new(1.0, 2.0);
        let a = ComplexCsr::from_shift(&k, &m, z);

        assert_eq!(a.n(), 2);

        // Expected: z*M - K
        // (0,0): (1+2i)*3 - 1 = (3+6i) - 1 = (2, 6)
        // (1,1): (1+2i)*4 - 2 = (4+8i) - 2 = (2, 8)
        let x = vec![C64::new(1.0, 0.0), C64::new(0.0, 1.0)];
        let mut y = vec![C64::zero(); 2];
        a.matvec(&x, &mut y);

        // y[0] = (2+6i) * (1+0i) = (2, 6)
        assert!(approx_eq(y[0], C64::new(2.0, 6.0)));
        // y[1] = (2+8i) * (0+1i) = (2*0 - 8*1) + (2*1 + 8*0)i = (-8, 2)
        assert!(approx_eq(y[1], C64::new(-8.0, 2.0)));
    }

    /// K and M have different sparsity patterns (BTreeMap merge path).
    #[test]
    fn from_shift_different_pattern() {
        // K = [[1, 2], [0, 3]]
        let k = CsrMat::try_from_csr_data(2, 2, vec![0, 2, 3], vec![0, 1, 1], vec![1.0, 2.0, 3.0])
            .unwrap();
        // M = [[4, 0], [5, 6]]
        let m = CsrMat::try_from_csr_data(2, 2, vec![0, 1, 3], vec![0, 0, 1], vec![4.0, 5.0, 6.0])
            .unwrap();

        let z = C64::new(1.0, 1.0);
        let a = ComplexCsr::from_shift(&k, &m, z);

        assert_eq!(a.n(), 2);

        // Expected z*M - K with z = (1+i):
        // Row 0: col 0: z*4 - 1 = (4+4i) - 1 = (3, 4)
        //        col 1: z*0 - 2 = (-2, 0)
        // Row 1: col 0: z*5 - 0 = (5+5i) = (5, 5)
        //        col 1: z*6 - 3 = (6+6i) - 3 = (3, 6)

        let x = vec![C64::new(1.0, 0.0), C64::new(1.0, 0.0)];
        let mut y = vec![C64::zero(); 2];
        a.matvec(&x, &mut y);

        // y[0] = (3+4i)*1 + (-2+0i)*1 = (1, 4)
        assert!(approx_eq(y[0], C64::new(1.0, 4.0)));
        // y[1] = (5+5i)*1 + (3+6i)*1 = (8, 11)
        assert!(approx_eq(y[1], C64::new(8.0, 11.0)));
    }

    #[test]
    fn diagonal_positions_reject_missing_diagonal() {
        let k = CsrMat::try_from_csr_data(2, 2, vec![0, 1, 2], vec![1, 0], vec![2.0, 3.0]).unwrap();
        let m = CsrMat::zeros(2, 2);
        let a = ComplexCsr::from_shift(&k, &m, C64::new(1.0, 0.0));

        let err = a.diagonal_positions().unwrap_err();
        assert!(
            err.contains("no diagonal entry"),
            "missing diagonal error should mention diagonal entry: {err}"
        );
    }
}
