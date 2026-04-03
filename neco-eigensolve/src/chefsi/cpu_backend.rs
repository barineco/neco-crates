use neco_sparse::CsrMat;

use super::filter::ChefsiBackend;

/// CPU fallback backend that stores `K` and lumped `M^-1` in `f32`.
pub struct CpuBackend {
    k_row_offsets: Vec<usize>,
    k_col_indices: Vec<usize>,
    k_values: Vec<f32>,
    m_inv_diag: Vec<f32>,
    n: usize,
}

impl CpuBackend {
    pub fn new(k: &CsrMat<f64>, m_diag: &[f64]) -> Self {
        let n = k.nrows();
        assert_eq!(n, k.ncols(), "K must be square");
        assert_eq!(n, m_diag.len(), "M diagonal length must match K");

        Self {
            k_row_offsets: k.row_offsets().to_vec(),
            k_col_indices: k.col_indices().to_vec(),
            k_values: k.values().iter().map(|&value| value as f32).collect(),
            m_inv_diag: m_diag.iter().map(|&value| (1.0 / value) as f32).collect(),
            n,
        }
    }
}

impl ChefsiBackend for CpuBackend {
    fn spmv_k(&self, x: &[f32], z: &mut [f32], n: usize, m: usize) {
        assert_eq!(n, self.n);
        assert_eq!(x.len(), n * m);
        assert_eq!(z.len(), n * m);

        for col in 0..m {
            let x_col = &x[col * n..(col + 1) * n];
            let z_col = &mut z[col * n..(col + 1) * n];
            for (row, z_value) in z_col.iter_mut().enumerate() {
                let start = self.k_row_offsets[row];
                let end = self.k_row_offsets[row + 1];
                let mut sum = 0.0f32;
                for pos in start..end {
                    sum += self.k_values[pos] * x_col[self.k_col_indices[pos]];
                }
                *z_value = sum;
            }
        }
    }

    fn diag_mul_m_inv(&self, z: &mut [f32], n: usize, m: usize) {
        assert_eq!(n, self.n);
        assert_eq!(z.len(), n * m);
        for col in 0..m {
            let z_col = &mut z[col * n..(col + 1) * n];
            for (value, &inv) in z_col.iter_mut().zip(&self.m_inv_diag) {
                *value *= inv;
            }
        }
    }

    fn axpy(&self, alpha: f32, x: &[f32], y: &mut [f32]) {
        assert_eq!(x.len(), y.len());
        for (yi, &xi) in y.iter_mut().zip(x) {
            *yi += alpha * xi;
        }
    }

    fn scale(&self, alpha: f32, y: &mut [f32]) {
        for value in y {
            *value *= alpha;
        }
    }

    fn copy(&self, src: &[f32], dst: &mut [f32]) {
        assert_eq!(src.len(), dst.len());
        dst.copy_from_slice(src);
    }
}
