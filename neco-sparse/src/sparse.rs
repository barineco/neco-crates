use std::ops::AddAssign;

/// CSR (Compressed Sparse Row) sparse matrix.
#[derive(Debug, Clone)]
pub struct CsrMat<T> {
    nrows: usize,
    ncols: usize,
    row_offsets: Vec<usize>,
    col_indices: Vec<usize>,
    values: Vec<T>,
}

/// Row view of a CSR matrix.
#[derive(Debug)]
pub struct CsrRow<'a, T> {
    col_indices: &'a [usize],
    values: &'a [T],
}

impl<'a, T> CsrRow<'a, T> {
    pub fn col_indices(&self) -> &'a [usize] {
        self.col_indices
    }

    pub fn values(&self) -> &'a [T] {
        self.values
    }

    pub fn nnz(&self) -> usize {
        self.col_indices.len()
    }
}

impl<T> CsrMat<T> {
    pub fn try_from_csr_data(
        nrows: usize,
        ncols: usize,
        row_offsets: Vec<usize>,
        col_indices: Vec<usize>,
        values: Vec<T>,
    ) -> Result<Self, String> {
        if row_offsets.len() != nrows + 1 {
            return Err(format!(
                "row_offsets length {} does not match nrows+1={}",
                row_offsets.len(),
                nrows + 1
            ));
        }
        if col_indices.len() != values.len() {
            return Err(format!(
                "col_indices length {} does not match values length {}",
                col_indices.len(),
                values.len()
            ));
        }
        let nnz = *row_offsets.last().unwrap_or(&0);
        if col_indices.len() != nnz {
            return Err(format!(
                "col_indices length {} does not match last row_offset {}",
                col_indices.len(),
                nnz
            ));
        }
        for window in row_offsets.windows(2) {
            if window[0] > window[1] {
                return Err("row_offsets is not monotonically non-decreasing".into());
            }
        }
        for row in 0..nrows {
            let start = row_offsets[row];
            let end = row_offsets[row + 1];
            let row_cols = &col_indices[start..end];
            if row_cols.windows(2).any(|window| window[0] >= window[1]) {
                return Err(format!(
                    "col_indices in row {} are not strictly increasing",
                    row
                ));
            }
        }
        for &col in &col_indices {
            if col >= ncols {
                return Err(format!(
                    "col_index {} out of range for ncols={}",
                    col, ncols
                ));
            }
        }
        Ok(Self {
            nrows,
            ncols,
            row_offsets,
            col_indices,
            values,
        })
    }

    pub fn nrows(&self) -> usize {
        self.nrows
    }

    pub fn ncols(&self) -> usize {
        self.ncols
    }

    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    pub fn row_offsets(&self) -> &[usize] {
        &self.row_offsets
    }

    pub fn col_indices(&self) -> &[usize] {
        &self.col_indices
    }

    pub fn values(&self) -> &[T] {
        &self.values
    }

    pub fn values_mut(&mut self) -> &mut [T] {
        &mut self.values
    }

    pub fn row(&self, i: usize) -> CsrRow<'_, T> {
        let start = self.row_offsets[i];
        let end = self.row_offsets[i + 1];
        CsrRow {
            col_indices: &self.col_indices[start..end],
            values: &self.values[start..end],
        }
    }

    pub fn row_iter(&self) -> impl Iterator<Item = CsrRow<'_, T>> {
        (0..self.nrows).map(move |i| self.row(i))
    }
}

impl<T: PartialEq + Copy> CsrMat<T> {
    pub fn get(&self, row: usize, col: usize) -> Option<&T> {
        if row >= self.nrows || col >= self.ncols {
            return None;
        }
        let start = self.row_offsets[row];
        let end = self.row_offsets[row + 1];
        let slice = &self.col_indices[start..end];
        match slice.binary_search(&col) {
            Ok(pos) => Some(&self.values[start + pos]),
            Err(_) => None,
        }
    }
}

impl<T: Copy> CsrMat<T> {
    pub fn triplet_iter(&self) -> impl Iterator<Item = (usize, usize, &T)> {
        (0..self.nrows).flat_map(move |row| {
            let start = self.row_offsets[row];
            let end = self.row_offsets[row + 1];
            (start..end).map(move |idx| (row, self.col_indices[idx], &self.values[idx]))
        })
    }
}

impl CsrMat<f64> {
    pub fn identity(n: usize) -> Self {
        let row_offsets: Vec<usize> = (0..=n).collect();
        let col_indices: Vec<usize> = (0..n).collect();
        let values = vec![1.0; n];
        Self {
            nrows: n,
            ncols: n,
            row_offsets,
            col_indices,
            values,
        }
    }

    pub fn zeros(nrows: usize, ncols: usize) -> Self {
        let row_offsets = vec![0; nrows + 1];
        Self {
            nrows,
            ncols,
            row_offsets,
            col_indices: Vec::new(),
            values: Vec::new(),
        }
    }

    pub fn linear_combination(&self, alpha: f64, other: &Self, beta: f64) -> Result<Self, String> {
        if self.nrows != other.nrows || self.ncols != other.ncols {
            return Err(format!(
                "matrix shape mismatch: lhs={}x{}, rhs={}x{}",
                self.nrows, self.ncols, other.nrows, other.ncols
            ));
        }
        if self.row_offsets != other.row_offsets || self.col_indices != other.col_indices {
            return Err("linear_combination requires identical CSR sparsity patterns".into());
        }

        let values = self
            .values
            .iter()
            .zip(other.values.iter())
            .map(|(&lhs, &rhs)| alpha * lhs + beta * rhs)
            .collect();

        Self::try_from_csr_data(
            self.nrows,
            self.ncols,
            self.row_offsets.clone(),
            self.col_indices.clone(),
            values,
        )
    }

    pub fn diagonal(&self) -> Result<Vec<f64>, String> {
        let ndiag = self.nrows.min(self.ncols);
        let mut diagonal = Vec::with_capacity(ndiag);
        for i in 0..ndiag {
            let row = self.row(i);
            let diag_pos = row
                .col_indices()
                .binary_search(&i)
                .map_err(|_| format!("missing diagonal entry at row {i}"))?;
            diagonal.push(row.values()[diag_pos]);
        }
        Ok(diagonal)
    }

    pub fn submatrix(&self, rows: &[usize], cols: &[usize]) -> Result<Self, String> {
        let mut col_positions = vec![usize::MAX; self.ncols];
        for (local_col, &global_col) in cols.iter().enumerate() {
            if global_col >= self.ncols {
                return Err(format!(
                    "column index {} out of range for ncols={}",
                    global_col, self.ncols
                ));
            }
            if col_positions[global_col] != usize::MAX {
                return Err(format!(
                    "duplicate column index {global_col} in submatrix request"
                ));
            }
            col_positions[global_col] = local_col;
        }

        let mut row_offsets = Vec::with_capacity(rows.len() + 1);
        let mut col_indices = Vec::new();
        let mut values = Vec::new();
        row_offsets.push(0);

        let mut seen_rows = vec![false; self.nrows];
        for &global_row in rows {
            if global_row >= self.nrows {
                return Err(format!(
                    "row index {} out of range for nrows={}",
                    global_row, self.nrows
                ));
            }
            if seen_rows[global_row] {
                return Err(format!(
                    "duplicate row index {global_row} in submatrix request"
                ));
            }
            seen_rows[global_row] = true;

            let row = self.row(global_row);
            let mut entries: Vec<(usize, f64)> = row
                .col_indices()
                .iter()
                .zip(row.values().iter())
                .filter_map(|(&global_col, &value)| {
                    let local_col = col_positions[global_col];
                    (local_col != usize::MAX).then_some((local_col, value))
                })
                .collect();
            entries.sort_unstable_by_key(|(local_col, _)| *local_col);

            for (local_col, value) in entries {
                col_indices.push(local_col);
                values.push(value);
            }
            row_offsets.push(col_indices.len());
        }

        Self::try_from_csr_data(rows.len(), cols.len(), row_offsets, col_indices, values)
    }
}

/// COO (Coordinate) sparse matrix.
#[derive(Debug, Clone)]
pub struct CooMat<T> {
    nrows: usize,
    ncols: usize,
    rows: Vec<usize>,
    cols: Vec<usize>,
    vals: Vec<T>,
}

impl<T> CooMat<T> {
    pub fn new(nrows: usize, ncols: usize) -> Self {
        Self {
            nrows,
            ncols,
            rows: Vec::new(),
            cols: Vec::new(),
            vals: Vec::new(),
        }
    }

    pub fn push(&mut self, row: usize, col: usize, val: T) {
        self.rows.push(row);
        self.cols.push(col);
        self.vals.push(val);
    }
}

impl<T: Copy + Default + AddAssign + PartialEq> From<&CooMat<T>> for CsrMat<T> {
    fn from(coo: &CooMat<T>) -> Self {
        let nrows = coo.nrows;
        let ncols = coo.ncols;
        let nnz_raw = coo.rows.len();

        if nnz_raw == 0 {
            return Self {
                nrows,
                ncols,
                row_offsets: vec![0; nrows + 1],
                col_indices: Vec::new(),
                values: Vec::new(),
            };
        }

        // Sort indices by (row, col) without copying triplet data
        let mut order: Vec<usize> = (0..nnz_raw).collect();
        order.sort_unstable_by(|&a, &b| {
            coo.rows[a]
                .cmp(&coo.rows[b])
                .then_with(|| coo.cols[a].cmp(&coo.cols[b]))
        });

        // Merge duplicates and build CSR arrays in a single pass
        let mut row_offsets = Vec::with_capacity(nrows + 1);
        let mut col_indices = Vec::with_capacity(nnz_raw);
        let mut values = Vec::with_capacity(nnz_raw);

        row_offsets.push(0);

        // Fill leading empty rows
        let first_row = coo.rows[order[0]];
        if first_row > 0 {
            row_offsets.extend(std::iter::repeat_n(0, first_row));
        }

        let mut prev_row = first_row;
        let mut prev_col = coo.cols[order[0]];
        let mut acc = T::default();
        acc += coo.vals[order[0]];

        for &idx in &order[1..] {
            let r = coo.rows[idx];
            let c = coo.cols[idx];

            if r == prev_row && c == prev_col {
                acc += coo.vals[idx];
            } else {
                // Flush previous entry
                col_indices.push(prev_col);
                values.push(acc);

                // Fill empty rows between prev_row and r
                for _ in prev_row..r {
                    row_offsets.push(col_indices.len());
                }

                prev_row = r;
                prev_col = c;
                acc = T::default();
                acc += coo.vals[idx];
            }
        }

        // Flush last entry
        col_indices.push(prev_col);
        values.push(acc);

        // Fill remaining rows (including closing the last occupied row)
        while row_offsets.len() <= nrows {
            row_offsets.push(col_indices.len());
        }

        debug_assert_eq!(row_offsets.len(), nrows + 1);

        Self {
            nrows,
            ncols,
            row_offsets,
            col_indices,
            values,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn csr_from_triplets(
        nrows: usize,
        ncols: usize,
        triplets: &[(usize, usize, f64)],
    ) -> CsrMat<f64> {
        let mut coo = CooMat::new(nrows, ncols);
        for &(row, col, value) in triplets {
            coo.push(row, col, value);
        }
        CsrMat::from(&coo)
    }

    #[test]
    fn csr_basic() {
        let mat =
            CsrMat::try_from_csr_data(2, 2, vec![0, 2, 3], vec![0, 1, 1], vec![1.0, 2.0, 3.0])
                .expect("CSR construction");
        assert_eq!(mat.nrows(), 2);
        assert_eq!(mat.ncols(), 2);
        assert_eq!(mat.nnz(), 3);
        assert_eq!(mat.row(0).col_indices(), &[0, 1]);
        assert_eq!(mat.row(0).values(), &[1.0, 2.0]);
        assert_eq!(mat.get(0, 0), Some(&1.0));
        assert_eq!(mat.get(0, 1), Some(&2.0));
        assert_eq!(mat.get(1, 1), Some(&3.0));
        assert_eq!(mat.get(1, 0), None);
    }

    #[test]
    fn csr_rejects_unsorted_row_columns() {
        let err = CsrMat::try_from_csr_data(1, 3, vec![0, 2], vec![2, 1], vec![1.0, 2.0])
            .expect_err("unsorted row must be rejected");
        assert_eq!(err, "col_indices in row 0 are not strictly increasing");

        let mat = CsrMat::try_from_csr_data(1, 3, vec![0, 2], vec![0, 2], vec![1.0, 2.0])
            .expect("sorted CSR construction");
        assert_eq!(mat.get(0, 0), Some(&1.0));
        assert_eq!(mat.get(0, 2), Some(&2.0));
    }

    #[test]
    fn coo_to_csr_accumulates_duplicates() {
        let mut coo = CooMat::new(2, 2);
        coo.push(0, 1, 1.5);
        coo.push(0, 1, 2.5);
        coo.push(1, 0, 4.0);
        let csr = CsrMat::from(&coo);
        assert_eq!(csr.nnz(), 2);
        assert_eq!(csr.get(0, 1), Some(&4.0));
        assert_eq!(csr.get(1, 0), Some(&4.0));
    }

    #[test]
    fn coo_to_csr_empty() {
        let coo: CooMat<f64> = CooMat::new(3, 3);
        let csr = CsrMat::from(&coo);
        assert_eq!(csr.nrows(), 3);
        assert_eq!(csr.ncols(), 3);
        assert_eq!(csr.nnz(), 0);
        for i in 0..3 {
            assert_eq!(csr.row(i).nnz(), 0);
        }
    }

    #[test]
    fn coo_to_csr_single_element() {
        let mut coo = CooMat::new(5, 5);
        coo.push(2, 3, 7.0);
        let csr = CsrMat::from(&coo);
        assert_eq!(csr.nnz(), 1);
        assert_eq!(csr.get(2, 3), Some(&7.0));
        assert_eq!(csr.get(0, 0), None);
    }

    #[test]
    fn coo_to_csr_reverse_column_order() {
        let mut coo = CooMat::new(1, 4);
        coo.push(0, 3, 4.0);
        coo.push(0, 1, 2.0);
        coo.push(0, 0, 1.0);
        coo.push(0, 2, 3.0);
        let csr = CsrMat::from(&coo);
        assert_eq!(csr.row(0).col_indices(), &[0, 1, 2, 3]);
        assert_eq!(csr.row(0).values(), &[1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn coo_to_csr_multiple_duplicates() {
        let mut coo = CooMat::new(2, 2);
        coo.push(0, 0, 1.0);
        coo.push(0, 0, 2.0);
        coo.push(0, 0, 3.0);
        coo.push(0, 0, 4.0);
        coo.push(1, 1, 5.0);
        let csr = CsrMat::from(&coo);
        assert_eq!(csr.nnz(), 2);
        assert_eq!(csr.get(0, 0), Some(&10.0));
        assert_eq!(csr.get(1, 1), Some(&5.0));
    }

    #[test]
    fn coo_to_csr_sorted_columns_per_row() {
        let mut coo = CooMat::new(3, 5);
        coo.push(0, 4, 1.0);
        coo.push(0, 0, 2.0);
        coo.push(1, 3, 3.0);
        coo.push(1, 1, 4.0);
        coo.push(2, 2, 5.0);
        let csr = CsrMat::from(&coo);
        assert_eq!(csr.row(0).col_indices(), &[0, 4]);
        assert_eq!(csr.row(1).col_indices(), &[1, 3]);
        assert_eq!(csr.row(2).col_indices(), &[2]);
    }

    #[test]
    fn coo_to_csr_sparse_rows_with_interior_gaps() {
        let mut coo = CooMat::new(5, 3);
        coo.push(0, 1, 1.0);
        coo.push(4, 2, 2.0);
        let csr = CsrMat::from(&coo);
        assert_eq!(csr.nnz(), 2);
        assert_eq!(csr.get(0, 1), Some(&1.0));
        assert_eq!(csr.get(4, 2), Some(&2.0));
        for i in 1..4 {
            assert_eq!(csr.row(i).nnz(), 0);
        }
    }

    #[test]
    fn coo_to_csr_integer_type() {
        let mut coo: CooMat<i32> = CooMat::new(2, 2);
        coo.push(0, 0, 10);
        coo.push(0, 0, 20);
        coo.push(1, 1, 30);
        let csr = CsrMat::from(&coo);
        assert_eq!(csr.get(0, 0), Some(&30));
        assert_eq!(csr.get(1, 1), Some(&30));
    }

    #[test]
    fn linear_combination_matches_shifted_matrix_pattern() {
        let k = csr_from_triplets(2, 2, &[(0, 0, 4.0), (0, 1, 1.0), (1, 1, 3.0)]);
        let m = csr_from_triplets(2, 2, &[(0, 0, 1.0), (0, 1, 0.5), (1, 1, 2.0)]);

        let shifted = k.linear_combination(1.0, &m, -2.0).unwrap();
        assert_eq!(shifted.row(0).col_indices(), &[0, 1]);
        assert_eq!(shifted.row(0).values(), &[2.0, 0.0]);
        assert_eq!(shifted.row(1).values(), &[-1.0]);
    }

    #[test]
    fn linear_combination_rejects_pattern_mismatch() {
        let lhs = csr_from_triplets(2, 2, &[(0, 0, 1.0), (1, 1, 2.0)]);
        let rhs = csr_from_triplets(2, 2, &[(0, 0, 1.0), (0, 1, 2.0), (1, 1, 3.0)]);

        let err = lhs.linear_combination(1.0, &rhs, -1.0).unwrap_err();
        assert!(err.contains("identical CSR sparsity patterns"), "err={err}");
    }

    #[test]
    fn diagonal_extracts_all_present_entries() {
        let mat = csr_from_triplets(3, 3, &[(0, 0, 2.0), (0, 2, 9.0), (1, 1, 3.0), (2, 2, 5.0)]);
        assert_eq!(mat.diagonal().unwrap(), vec![2.0, 3.0, 5.0]);
    }

    #[test]
    fn diagonal_rejects_missing_entry() {
        let mat = csr_from_triplets(2, 2, &[(0, 1, 1.0), (1, 1, 2.0)]);
        let err = mat.diagonal().unwrap_err();
        assert!(err.contains("missing diagonal entry"), "err={err}");
    }

    #[test]
    fn submatrix_preserves_requested_order_with_sorted_local_columns() {
        let mat = csr_from_triplets(
            3,
            4,
            &[
                (0, 0, 1.0),
                (0, 1, 2.0),
                (0, 3, 3.0),
                (1, 0, 4.0),
                (1, 2, 5.0),
                (2, 1, 6.0),
                (2, 3, 7.0),
            ],
        );

        let sub = mat.submatrix(&[2, 0], &[3, 1]).unwrap();
        assert_eq!(sub.nrows(), 2);
        assert_eq!(sub.ncols(), 2);
        assert_eq!(sub.row(0).col_indices(), &[0, 1]);
        assert_eq!(sub.row(0).values(), &[7.0, 6.0]);
        assert_eq!(sub.row(1).col_indices(), &[0, 1]);
        assert_eq!(sub.row(1).values(), &[3.0, 2.0]);
    }

    #[test]
    fn submatrix_rejects_duplicate_indices() {
        let mat = csr_from_triplets(2, 3, &[(0, 0, 1.0), (0, 1, 2.0), (1, 2, 3.0)]);

        let row_err = mat.submatrix(&[0, 0], &[1]).unwrap_err();
        assert!(row_err.contains("duplicate row index"), "err={row_err}");

        let col_err = mat.submatrix(&[0], &[1, 1]).unwrap_err();
        assert!(col_err.contains("duplicate column index"), "err={col_err}");
    }
}
