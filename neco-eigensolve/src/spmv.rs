use neco_sparse::CsrMat;

#[cfg(feature = "parallel")]
const PARALLEL_THRESHOLD: usize = 10_000;

/// y = A * x (sequential, raw-slice)
#[inline(always)]
pub(crate) fn spmv_into(y: &mut [f64], a: &CsrMat<f64>, x: &[f64]) {
    debug_assert_eq!(y.len(), a.nrows());
    debug_assert!(x.len() >= a.ncols());
    let offsets = a.row_offsets();
    let cols = a.col_indices();
    let vals = a.values();
    for row_idx in 0..a.nrows() {
        let start = offsets[row_idx];
        let end = offsets[row_idx + 1];
        let mut sum = 0.0;
        for pos in start..end {
            sum += vals[pos] * x[cols[pos]];
        }
        y[row_idx] = sum;
    }
}

/// y = A * x (row-parallel)
#[cfg(feature = "parallel")]
#[inline]
fn spmv_into_parallel(y: &mut [f64], a: &CsrMat<f64>, x: &[f64]) {
    use rayon::prelude::*;
    let offsets = a.row_offsets();
    let cols = a.col_indices();
    let vals = a.values();
    y.par_iter_mut().enumerate().for_each(|(row_idx, y_i)| {
        let start = offsets[row_idx];
        let end = offsets[row_idx + 1];
        let mut sum = 0.0;
        for pos in start..end {
            sum += vals[pos] * x[cols[pos]];
        }
        *y_i = sum;
    });
}

/// Single-vector dispatch: parallel when nnz >= threshold, sequential otherwise.
#[inline]
pub(crate) fn spmv_dispatch(y: &mut [f64], a: &CsrMat<f64>, x: &[f64]) {
    #[cfg(feature = "parallel")]
    {
        if a.nnz() >= PARALLEL_THRESHOLD {
            spmv_into_parallel(y, a, x);
            return;
        }
    }
    spmv_into(y, a, x);
}

/// Block dispatch: y = A * X where X is n×ncols column-major.
#[inline]
pub(crate) fn block_spmv_dispatch(
    y: &mut [f64],
    a: &CsrMat<f64>,
    x: &[f64],
    n: usize,
    ncols: usize,
) {
    debug_assert!(y.len() >= n * ncols);
    debug_assert!(x.len() >= n * ncols);
    for j in 0..ncols {
        let offset = j * n;
        spmv_dispatch(&mut y[offset..offset + n], a, &x[offset..offset + n]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neco_sparse::CsrMat;

    fn diagonal_csr(diag: &[f64]) -> CsrMat<f64> {
        let n = diag.len();
        let offsets: Vec<usize> = (0..=n).collect();
        let indices: Vec<usize> = (0..n).collect();
        CsrMat::try_from_csr_data(n, n, offsets, indices, diag.to_vec()).unwrap()
    }

    #[test]
    fn spmv_diagonal() {
        let a = diagonal_csr(&[2.0, 3.0, 5.0]);
        let x = [1.0, 2.0, 3.0];
        let mut y = [0.0; 3];
        spmv_into(&mut y, &a, &x);
        assert_eq!(y, [2.0, 6.0, 15.0]);
    }

    #[test]
    fn spmv_tridiagonal() {
        let offsets = vec![0, 2, 5, 7];
        let cols = vec![0, 1, 0, 1, 2, 1, 2];
        let vals = vec![2.0, -1.0, -1.0, 2.0, -1.0, -1.0, 2.0];
        let a = CsrMat::try_from_csr_data(3, 3, offsets, cols, vals).unwrap();
        let x = [1.0, 1.0, 1.0];
        let mut y = [0.0; 3];
        spmv_into(&mut y, &a, &x);
        assert_eq!(y, [1.0, 0.0, 1.0]);
    }

    #[test]
    fn block_spmv_two_columns() {
        let a = diagonal_csr(&[2.0, 3.0, 5.0]);
        // 2 columns, column-major: col0 = [1,2,3], col1 = [4,5,6]
        let x = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let mut y = [0.0; 6];
        block_spmv_dispatch(&mut y, &a, &x, 3, 2);
        assert_eq!(y, [2.0, 6.0, 15.0, 8.0, 15.0, 30.0]);
    }
}
