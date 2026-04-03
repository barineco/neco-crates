use std::ops::{Index, IndexMut};

#[derive(Debug, Clone, PartialEq)]
pub struct DenseMatrix {
    nrows: usize,
    ncols: usize,
    data: Vec<f64>,
}

impl DenseMatrix {
    pub fn zeros(nrows: usize, ncols: usize) -> Self {
        Self {
            nrows,
            ncols,
            data: vec![0.0; nrows * ncols],
        }
    }

    pub fn identity(nrows: usize, ncols: usize) -> Self {
        let mut out = Self::zeros(nrows, ncols);
        let diag = nrows.min(ncols);
        for i in 0..diag {
            out[(i, i)] = 1.0;
        }
        out
    }

    pub fn from_diagonal_element(nrows: usize, ncols: usize, value: f64) -> Self {
        let mut out = Self::zeros(nrows, ncols);
        let diag = nrows.min(ncols);
        for i in 0..diag {
            out[(i, i)] = value;
        }
        out
    }

    pub fn from_column_slice(nrows: usize, ncols: usize, data: &[f64]) -> Self {
        assert_eq!(data.len(), nrows * ncols);
        Self {
            nrows,
            ncols,
            data: data.to_vec(),
        }
    }

    pub fn from_row_slice(nrows: usize, ncols: usize, data: &[f64]) -> Self {
        assert_eq!(data.len(), nrows * ncols);
        let mut out = Self::zeros(nrows, ncols);
        for row in 0..nrows {
            for col in 0..ncols {
                out[(row, col)] = data[row * ncols + col];
            }
        }
        out
    }

    pub fn from_fn(nrows: usize, ncols: usize, mut f: impl FnMut(usize, usize) -> f64) -> Self {
        let mut out = Self::zeros(nrows, ncols);
        for col in 0..ncols {
            for row in 0..nrows {
                out[(row, col)] = f(row, col);
            }
        }
        out
    }

    pub fn nrows(&self) -> usize {
        self.nrows
    }

    pub fn ncols(&self) -> usize {
        self.ncols
    }

    pub fn as_slice(&self) -> &[f64] {
        &self.data
    }

    pub(crate) fn as_mut_slice(&mut self) -> &mut [f64] {
        &mut self.data
    }

    pub fn into_vec(self) -> Vec<f64> {
        self.data
    }

    pub(crate) fn column(&self, col: usize) -> &[f64] {
        let start = col * self.nrows;
        &self.data[start..start + self.nrows]
    }

    pub(crate) fn column_mut(&mut self, col: usize) -> &mut [f64] {
        let start = col * self.nrows;
        &mut self.data[start..start + self.nrows]
    }

    pub(crate) fn set_column(&mut self, col: usize, values: &[f64]) {
        assert_eq!(values.len(), self.nrows);
        self.column_mut(col).copy_from_slice(values);
    }

    pub(crate) fn get(&self, row: usize, col: usize) -> f64 {
        self[(row, col)]
    }

    pub(crate) fn set(&mut self, row: usize, col: usize, value: f64) {
        self[(row, col)] = value;
    }

    pub(crate) fn copy_columns_from(
        &mut self,
        dst_start: usize,
        src: &Self,
        src_start: usize,
        count: usize,
    ) {
        assert_eq!(self.nrows, src.nrows);
        for offset in 0..count {
            self.column_mut(dst_start + offset)
                .copy_from_slice(src.column(src_start + offset));
        }
    }

    pub(crate) fn transpose(&self) -> Self {
        let mut out = Self::zeros(self.ncols, self.nrows);
        for row in 0..self.nrows {
            for col in 0..self.ncols {
                out[(col, row)] = self[(row, col)];
            }
        }
        out
    }

    pub(crate) fn mul(&self, rhs: &Self) -> Self {
        assert_eq!(self.ncols, rhs.nrows);
        let mut out = Self::zeros(self.nrows, rhs.ncols);
        for out_col in 0..rhs.ncols {
            for k in 0..self.ncols {
                let rhs_value = rhs[(k, out_col)];
                if rhs_value.abs() <= 1e-30 {
                    continue;
                }
                for row in 0..self.nrows {
                    out[(row, out_col)] += self[(row, k)] * rhs_value;
                }
            }
        }
        out
    }

    pub(crate) fn transpose_mul(&self, rhs: &Self) -> Self {
        assert_eq!(self.nrows, rhs.nrows);
        let mut out = Self::zeros(self.ncols, rhs.ncols);
        for out_col in 0..rhs.ncols {
            for left_col in 0..self.ncols {
                out[(left_col, out_col)] = self
                    .column(left_col)
                    .iter()
                    .zip(rhs.column(out_col))
                    .map(|(a, b)| a * b)
                    .sum();
            }
        }
        out
    }

    pub(crate) fn mul_vector(&self, rhs: &[f64]) -> Vec<f64> {
        assert_eq!(self.ncols, rhs.len());
        let mut out = vec![0.0; self.nrows];
        for col in 0..self.ncols {
            let rhs_value = rhs[col];
            if rhs_value.abs() <= 1e-30 {
                continue;
            }
            for row in 0..self.nrows {
                out[row] += self[(row, col)] * rhs_value;
            }
        }
        out
    }

    pub(crate) fn select_columns(&self, indices: &[usize]) -> Self {
        let mut out = Self::zeros(self.nrows, indices.len());
        for (dst_col, &src_col) in indices.iter().enumerate() {
            out.column_mut(dst_col)
                .copy_from_slice(self.column(src_col));
        }
        out
    }

    pub(crate) fn to_row_major(&self) -> Vec<f64> {
        let mut out = vec![0.0; self.nrows * self.ncols];
        for row in 0..self.nrows {
            for col in 0..self.ncols {
                out[row * self.ncols + col] = self[(row, col)];
            }
        }
        out
    }

    pub(crate) fn from_row_major(nrows: usize, ncols: usize, data: &[f64]) -> Self {
        Self::from_row_slice(nrows, ncols, data)
    }
}

impl Index<(usize, usize)> for DenseMatrix {
    type Output = f64;

    fn index(&self, index: (usize, usize)) -> &Self::Output {
        &self.data[index.1 * self.nrows + index.0]
    }
}

impl IndexMut<(usize, usize)> for DenseMatrix {
    fn index_mut(&mut self, index: (usize, usize)) -> &mut Self::Output {
        &mut self.data[index.1 * self.nrows + index.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dense_matrix_roundtrips_column_major() {
        let mat = DenseMatrix::from_column_slice(3, 2, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert_eq!(mat.nrows(), 3);
        assert_eq!(mat.ncols(), 2);
        assert_eq!(mat[(2, 1)], 6.0);
        assert_eq!(mat.as_slice(), &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn transpose_mul_matches_manual_result() {
        let a = DenseMatrix::from_row_slice(3, 2, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let b = DenseMatrix::from_row_slice(3, 2, &[7.0, 8.0, 9.0, 10.0, 11.0, 12.0]);
        let gram = a.transpose_mul(&b);
        assert_eq!(
            gram,
            DenseMatrix::from_row_slice(2, 2, &[89.0, 98.0, 116.0, 128.0])
        );
    }
}
