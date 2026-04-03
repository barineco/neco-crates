use std::collections::{BTreeMap, BTreeSet};

use neco_sparse::CsrMat;

use crate::c64::C64;

use super::backend::PreparedLinearSolver;

#[derive(Debug, Clone)]
pub(crate) struct ShiftedCsrData {
    pub(crate) n: usize,
    pub(crate) row_offsets: Vec<usize>,
    pub(crate) col_indices: Vec<usize>,
    pub(crate) values: Vec<C64>,
}

impl ShiftedCsrData {
    pub(crate) fn from_shift(k: &CsrMat<f64>, m: &CsrMat<f64>, z: C64) -> Self {
        let n = k.nrows();
        let k_offsets = k.row_offsets();
        let m_offsets = m.row_offsets();
        let pattern_match = k_offsets == m_offsets && k.col_indices() == m.col_indices();

        if pattern_match {
            let values: Vec<C64> = k
                .values()
                .iter()
                .zip(m.values().iter())
                .map(|(&kv, &mv)| z * C64::new(mv, 0.0) - C64::new(kv, 0.0))
                .collect();
            Self {
                n,
                row_offsets: k_offsets.to_vec(),
                col_indices: k.col_indices().to_vec(),
                values,
            }
        } else {
            let mut rows: Vec<BTreeMap<usize, C64>> = vec![BTreeMap::new(); n];
            for (row_idx, row) in rows.iter_mut().enumerate() {
                let k_row = k.row(row_idx);
                for (&col, &val) in k_row.col_indices().iter().zip(k_row.values()) {
                    *row.entry(col).or_insert(C64::zero()) -= C64::new(val, 0.0);
                }
                let m_row = m.row(row_idx);
                for (&col, &val) in m_row.col_indices().iter().zip(m_row.values()) {
                    *row.entry(col).or_insert(C64::zero()) += z * C64::new(val, 0.0);
                }
            }

            let mut row_offsets = vec![0usize];
            let mut col_indices = Vec::new();
            let mut values = Vec::new();
            for row in &rows {
                for (&col, &val) in row {
                    col_indices.push(col);
                    values.push(val);
                }
                row_offsets.push(col_indices.len());
            }

            Self {
                n,
                row_offsets,
                col_indices,
                values,
            }
        }
    }

    pub(crate) fn diagonal_positions(&self) -> Result<Vec<usize>, String> {
        diagonal_positions(self.n, &self.row_offsets, &self.col_indices)
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct SymbolicLuPattern {
    pub(crate) n: usize,
    pub(crate) row_offsets: Vec<usize>,
    pub(crate) col_indices: Vec<usize>,
    pub(crate) diag_pos: Vec<usize>,
}

impl SymbolicLuPattern {
    #[allow(dead_code)]
    pub(crate) fn from_shifted(shifted: &ShiftedCsrData) -> Result<Self, String> {
        shifted.diagonal_positions()?;

        let mut rows: Vec<BTreeSet<usize>> = (0..shifted.n)
            .map(|row_idx| {
                let start = shifted.row_offsets[row_idx];
                let end = shifted.row_offsets[row_idx + 1];
                shifted.col_indices[start..end].iter().copied().collect()
            })
            .collect();

        for row_idx in 0..shifted.n {
            let mut pending: BTreeSet<usize> = rows[row_idx]
                .iter()
                .copied()
                .filter(|&col| col < row_idx)
                .collect();

            while let Some(pivot_col) = pending.pop_first() {
                let upper_cols: Vec<usize> =
                    rows[pivot_col].range((pivot_col + 1)..).copied().collect();
                for upper_col in upper_cols {
                    if rows[row_idx].insert(upper_col) && upper_col < row_idx {
                        pending.insert(upper_col);
                    }
                }
            }
        }

        let mut row_offsets = vec![0usize];
        let mut col_indices = Vec::new();
        for row in &rows {
            for &col in row {
                col_indices.push(col);
            }
            row_offsets.push(col_indices.len());
        }
        let diag_pos = diagonal_positions(shifted.n, &row_offsets, &col_indices)?;

        Ok(Self {
            n: shifted.n,
            row_offsets,
            col_indices,
            diag_pos,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct NumericLu {
    row_offsets: Vec<usize>,
    col_indices: Vec<usize>,
    lu_values: Vec<C64>,
    diag_pos: Vec<usize>,
    row_perm: Vec<usize>,
}

#[derive(Debug, Clone)]
struct PivotCandidate {
    row_idx: usize,
    row: BTreeMap<usize, C64>,
    diag_norm: f64,
    trailing_offdiag_norm: f64,
    active_row_l1_norm: f64,
}

impl NumericLu {
    pub(crate) fn factorize(
        shifted: &ShiftedCsrData,
        symbolic: &SymbolicLuPattern,
    ) -> Result<Self, String> {
        const LIMITED_PIVOT_WINDOW: usize = 8;

        let mut rows = build_numeric_rows(shifted, symbolic)?;
        let mut row_perm: Vec<usize> = (0..shifted.n).collect();

        for row_idx in 0..shifted.n {
            let mut best_candidate: Option<PivotCandidate> = None;
            let pivot_window_end = pivot_window_end(shifted.n, row_idx, LIMITED_PIVOT_WINDOW);
            for candidate_idx in row_idx..pivot_window_end {
                let candidate_row = factorize_candidate_row(&rows, candidate_idx, row_idx)?;
                if let Some(candidate) =
                    evaluate_pivot_candidate(candidate_idx, candidate_row, row_idx)
                {
                    if best_candidate
                        .as_ref()
                        .is_none_or(|best| pivot_candidate_is_better(&candidate, best))
                    {
                        best_candidate = Some(candidate);
                    }
                }
            }

            let Some(best_candidate) = best_candidate else {
                let diag = rows[row_idx]
                    .get(&row_idx)
                    .copied()
                    .unwrap_or_else(C64::zero);
                return Err(format!(
                    "internal LU encountered a near-zero diagonal at row {} within pivot window {}: {:?}",
                    row_idx, LIMITED_PIVOT_WINDOW, diag
                ));
            };

            if best_candidate.row_idx != row_idx {
                rows.swap(row_idx, best_candidate.row_idx);
                row_perm.swap(row_idx, best_candidate.row_idx);
            }
            rows[row_idx] = best_candidate.row;
        }

        let mut row_offsets = vec![0usize];
        let mut col_indices = Vec::new();
        let mut lu_values = Vec::new();
        for (row_idx, row) in rows.iter().enumerate() {
            for (&col, &val) in row {
                if col == row_idx || val.norm() > 1e-40 {
                    col_indices.push(col);
                    lu_values.push(val);
                }
            }
            row_offsets.push(col_indices.len());
        }
        let diag_pos = diagonal_positions(shifted.n, &row_offsets, &col_indices)?;

        Ok(Self {
            row_offsets,
            col_indices,
            lu_values,
            diag_pos,
            row_perm,
        })
    }

    pub(crate) fn solve(&self, rhs: &[C64], out: &mut [C64]) -> Result<(), String> {
        if rhs.len() != self.diag_pos.len() || out.len() != self.diag_pos.len() {
            return Err(format!(
                "rhs/out length mismatch: rhs={}, out={}, n={}",
                rhs.len(),
                out.len(),
                self.diag_pos.len()
            ));
        }

        let n = self.diag_pos.len();
        let mut z = vec![C64::zero(); n];
        for i in 0..n {
            z[i] = rhs[self.row_perm[i]];
            let row_start = self.row_offsets[i];
            let diag = self.diag_pos[i];
            let mut sum = z[i];
            for pos in row_start..diag {
                let k = self.col_indices[pos];
                sum -= self.lu_values[pos] * z[k];
            }
            z[i] = sum;
        }

        out.copy_from_slice(&z);
        for i in (0..n).rev() {
            let diag = self.diag_pos[i];
            let row_end = self.row_offsets[i + 1];
            let mut sum = out[i];
            for pos in (diag + 1)..row_end {
                let j = self.col_indices[pos];
                sum -= self.lu_values[pos] * out[j];
            }
            let d = self.lu_values[diag];
            if d.norm() <= 1e-30 || !d.norm().is_finite() {
                return Err(format!(
                    "internal LU encountered a non-invertible diagonal at row {}: {:?}",
                    i, d
                ));
            }
            out[i] = sum / d;
        }
        Ok(())
    }
}

pub(crate) struct InternalLuShiftedSolver {
    lu: NumericLu,
}

impl InternalLuShiftedSolver {
    pub(crate) fn new(k_mat: &CsrMat<f64>, m_mat: &CsrMat<f64>, z: C64) -> Result<Self, String> {
        let shifted = ShiftedCsrData::from_shift(k_mat, m_mat, z);
        let symbolic = SymbolicLuPattern::from_shifted(&shifted)?;
        let lu = NumericLu::factorize(&shifted, &symbolic)?;
        Ok(Self { lu })
    }
}

impl PreparedLinearSolver for InternalLuShiftedSolver {
    fn solve_block(&self, rhs: &[f64], n: usize, m0: usize, _tol: f64) -> Result<Vec<C64>, String> {
        if rhs.len() != n * m0 {
            return Err(format!(
                "rhs length {} does not match n * m0 = {}",
                rhs.len(),
                n * m0
            ));
        }

        let mut result = Vec::with_capacity(n * m0);
        let mut rhs_c = vec![C64::zero(); n];
        let mut x = vec![C64::zero(); n];
        for rhs_idx in 0..m0 {
            let col_start = rhs_idx * n;
            for i in 0..n {
                rhs_c[i] = C64::new(rhs[col_start + i], 0.0);
            }
            self.lu.solve(&rhs_c, &mut x)?;
            result.extend_from_slice(&x);
        }
        Ok(result)
    }
}

fn evaluate_pivot_candidate(
    candidate_idx: usize,
    candidate_row: BTreeMap<usize, C64>,
    row_idx: usize,
) -> Option<PivotCandidate> {
    const PIVOT_TOL: f64 = 1e-30;

    let diag_norm = candidate_row
        .get(&row_idx)
        .copied()
        .unwrap_or_else(C64::zero)
        .norm();
    if !diag_norm.is_finite() || diag_norm <= PIVOT_TOL {
        return None;
    }

    let trailing_offdiag_norm = candidate_row
        .range((row_idx + 1)..)
        .map(|(_, value)| value.norm())
        .fold(0.0f64, f64::max);
    let active_row_l1_norm = active_row_l1_norm(&candidate_row, row_idx);

    Some(PivotCandidate {
        row_idx: candidate_idx,
        row: candidate_row,
        diag_norm,
        trailing_offdiag_norm,
        active_row_l1_norm,
    })
}

fn pivot_candidate_is_better(candidate: &PivotCandidate, best: &PivotCandidate) -> bool {
    let candidate_threshold_ok = pivot_passes_thresholds(candidate);
    let best_threshold_ok = pivot_passes_thresholds(best);
    if candidate_threshold_ok != best_threshold_ok {
        return candidate_threshold_ok;
    }

    let candidate_row_ratio = pivot_row_quality_ratio(candidate);
    let best_row_ratio = pivot_row_quality_ratio(best);
    if (candidate_row_ratio - best_row_ratio).abs() > 1e-12 {
        return candidate_row_ratio > best_row_ratio;
    }

    let candidate_trailing_ratio = pivot_trailing_quality_ratio(candidate);
    let best_trailing_ratio = pivot_trailing_quality_ratio(best);
    if (candidate_trailing_ratio - best_trailing_ratio).abs() > 1e-12 {
        return candidate_trailing_ratio > best_trailing_ratio;
    }

    if (candidate.diag_norm - best.diag_norm).abs() > 1e-30 {
        return candidate.diag_norm > best.diag_norm;
    }

    candidate.row_idx < best.row_idx
}

fn pivot_passes_thresholds(candidate: &PivotCandidate) -> bool {
    const PIVOT_TRAILING_THRESHOLD: f64 = 0.1;
    const PIVOT_ROW_NORM_THRESHOLD: f64 = 0.05;

    candidate.diag_norm >= PIVOT_TRAILING_THRESHOLD * candidate.trailing_offdiag_norm
        && candidate.diag_norm >= PIVOT_ROW_NORM_THRESHOLD * candidate.active_row_l1_norm
}

fn pivot_trailing_quality_ratio(candidate: &PivotCandidate) -> f64 {
    if candidate.trailing_offdiag_norm <= 1e-40 {
        f64::INFINITY
    } else {
        candidate.diag_norm / candidate.trailing_offdiag_norm
    }
}

fn pivot_row_quality_ratio(candidate: &PivotCandidate) -> f64 {
    if candidate.active_row_l1_norm <= 1e-40 {
        f64::INFINITY
    } else {
        candidate.diag_norm / candidate.active_row_l1_norm
    }
}

fn active_row_l1_norm(row: &BTreeMap<usize, C64>, row_idx: usize) -> f64 {
    row.range(row_idx..).map(|(_, value)| value.norm()).sum()
}

fn pivot_window_end(n: usize, row_idx: usize, window: usize) -> usize {
    row_idx.saturating_add(window).min(n)
}

pub(crate) fn diagonal_positions(
    n: usize,
    row_offsets: &[usize],
    col_indices: &[usize],
) -> Result<Vec<usize>, String> {
    let mut diag_pos = vec![0usize; n];
    for (row_idx, diag_slot) in diag_pos.iter_mut().enumerate() {
        let start = row_offsets[row_idx];
        let end = row_offsets[row_idx + 1];
        let mut found = false;
        for (offset, &col) in col_indices[start..end].iter().enumerate() {
            if col == row_idx {
                *diag_slot = start + offset;
                found = true;
                break;
            }
        }
        if !found {
            return Err(format!(
                "shifted matrix row {} has no diagonal entry",
                row_idx
            ));
        }
    }
    Ok(diag_pos)
}

fn build_numeric_rows(
    shifted: &ShiftedCsrData,
    symbolic: &SymbolicLuPattern,
) -> Result<Vec<BTreeMap<usize, C64>>, String> {
    let mut rows: Vec<BTreeMap<usize, C64>> = Vec::with_capacity(shifted.n);
    for row_idx in 0..shifted.n {
        let mut row = BTreeMap::new();
        let sym_start = symbolic.row_offsets[row_idx];
        let sym_end = symbolic.row_offsets[row_idx + 1];
        for &col in &symbolic.col_indices[sym_start..sym_end] {
            row.insert(col, C64::zero());
        }

        let shifted_start = shifted.row_offsets[row_idx];
        let shifted_end = shifted.row_offsets[row_idx + 1];
        for pos in shifted_start..shifted_end {
            let col = shifted.col_indices[pos];
            if let Some(slot) = row.get_mut(&col) {
                *slot = shifted.values[pos];
            } else {
                return Err(format!(
                    "symbolic pattern is missing shifted entry at row {}, col {}",
                    row_idx, col
                ));
            }
        }

        rows.push(row);
    }
    Ok(rows)
}

fn factorize_candidate_row(
    rows: &[BTreeMap<usize, C64>],
    candidate_idx: usize,
    row_idx: usize,
) -> Result<BTreeMap<usize, C64>, String> {
    const PIVOT_TOL: f64 = 1e-30;

    let mut row = rows[candidate_idx].clone();
    let mut pending: BTreeSet<usize> = row.keys().copied().filter(|&col| col < row_idx).collect();

    while let Some(pivot_row) = pending.pop_first() {
        let value = row.get(&pivot_row).copied().unwrap_or_else(C64::zero);
        if value.norm() <= 1e-40 {
            continue;
        }

        let pivot_diag = rows[pivot_row]
            .get(&pivot_row)
            .copied()
            .unwrap_or_else(C64::zero);
        if pivot_diag.norm() <= PIVOT_TOL || !pivot_diag.norm().is_finite() {
            return Err(format!(
                "internal LU encountered a near-zero pivot at row {}: {:?}",
                pivot_row, pivot_diag
            ));
        }

        let factor = value / pivot_diag;
        row.insert(pivot_row, factor);

        for (&col, &pivot_val) in rows[pivot_row].range((pivot_row + 1)..) {
            let updated = row.get(&col).copied().unwrap_or_else(C64::zero) - factor * pivot_val;
            row.insert(col, updated);
            if col < row_idx {
                pending.insert(col);
            }
        }
    }

    Ok(row)
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
    fn shifted_csr_data_merges_mismatched_patterns() {
        let k = CsrMat::try_from_csr_data(2, 2, vec![0, 2, 3], vec![0, 1, 1], vec![1.0, 2.0, 3.0])
            .unwrap();
        let m = CsrMat::try_from_csr_data(2, 2, vec![0, 1, 3], vec![0, 0, 1], vec![4.0, 5.0, 6.0])
            .unwrap();

        let shifted = ShiftedCsrData::from_shift(&k, &m, C64::new(1.0, 1.0));
        assert_eq!(shifted.row_offsets, vec![0, 2, 4]);
        assert_eq!(shifted.col_indices, vec![0, 1, 0, 1]);
        assert_eq!(shifted.values[0], C64::new(3.0, 4.0));
        assert_eq!(shifted.values[1], C64::new(-2.0, 0.0));
        assert_eq!(shifted.values[2], C64::new(5.0, 5.0));
        assert_eq!(shifted.values[3], C64::new(3.0, 6.0));
    }

    #[test]
    fn shifted_csr_data_finds_diagonal_positions() {
        let k = diagonal_csr(3, &[2.0, 3.0, 5.0]);
        let m = diagonal_csr(3, &[1.0, 1.0, 1.0]);

        let shifted = ShiftedCsrData::from_shift(&k, &m, C64::new(1.5, 0.25));
        assert_eq!(shifted.diagonal_positions().unwrap(), vec![0, 1, 2]);
    }

    #[test]
    fn symbolic_lu_pattern_preserves_diagonal_pattern() {
        let k = diagonal_csr(3, &[2.0, 3.0, 5.0]);
        let m = diagonal_csr(3, &[1.0, 1.0, 1.0]);

        let shifted = ShiftedCsrData::from_shift(&k, &m, C64::new(1.5, 0.25));
        let symbolic = SymbolicLuPattern::from_shifted(&shifted).unwrap();
        assert_eq!(symbolic.n, 3);
        assert_eq!(symbolic.row_offsets, vec![0, 1, 2, 3]);
        assert_eq!(symbolic.col_indices, vec![0, 1, 2]);
        assert_eq!(symbolic.diag_pos, vec![0, 1, 2]);
    }

    #[test]
    fn symbolic_lu_pattern_adds_natural_order_fill() {
        let shifted = ShiftedCsrData {
            n: 3,
            row_offsets: vec![0, 2, 4, 6],
            col_indices: vec![0, 2, 0, 1, 1, 2],
            values: vec![C64::new(1.0, 0.0); 6],
        };

        let symbolic = SymbolicLuPattern::from_shifted(&shifted).unwrap();
        assert_eq!(symbolic.row_offsets, vec![0, 2, 5, 7]);
        assert_eq!(symbolic.col_indices, vec![0, 2, 0, 1, 2, 1, 2]);
        assert_eq!(symbolic.diag_pos, vec![0, 3, 6]);
    }

    #[test]
    fn numeric_lu_solves_diagonal_system_exactly() {
        let k = diagonal_csr(3, &[1.0, 2.0, 3.0]);
        let m = diagonal_csr(3, &[1.0, 1.0, 1.0]);
        let shifted = ShiftedCsrData::from_shift(&k, &m, C64::new(2.5, 1.0));
        let symbolic = SymbolicLuPattern::from_shifted(&shifted).unwrap();
        let lu = NumericLu::factorize(&shifted, &symbolic).unwrap();

        let rhs = vec![C64::new(1.0, 0.0), C64::new(1.0, 0.0), C64::new(1.0, 0.0)];
        let mut out = vec![C64::zero(); 3];
        lu.solve(&rhs, &mut out).unwrap();

        let expected = [
            C64::new(1.0, 0.0) / C64::new(1.5, 1.0),
            C64::new(1.0, 0.0) / C64::new(0.5, 1.0),
            C64::new(1.0, 0.0) / C64::new(-0.5, 1.0),
        ];
        for i in 0..3 {
            let diff = out[i] - expected[i];
            let err = (diff.re * diff.re + diff.im * diff.im).sqrt();
            assert!(err < 1e-12, "x[{i}] err={err}");
        }
    }

    #[test]
    fn numeric_lu_rejects_near_zero_diagonal() {
        let k = diagonal_csr(2, &[1.0, 2.0]);
        let m = diagonal_csr(2, &[1.0, 1.0]);
        let shifted = ShiftedCsrData::from_shift(&k, &m, C64::new(1.0, 0.0));
        let symbolic = SymbolicLuPattern::from_shifted(&shifted).unwrap();

        let err = NumericLu::factorize(&shifted, &symbolic).unwrap_err();
        assert!(
            err.contains("near-zero diagonal"),
            "near-zero diagonal error should be preserved: {err}"
        );
    }

    #[test]
    fn internal_lu_shifted_solver_rejects_missing_diagonal_pattern() {
        let k = CsrMat::try_from_csr_data(2, 2, vec![0, 1, 2], vec![1, 0], vec![2.0, 3.0]).unwrap();
        let m = CsrMat::zeros(2, 2);

        let err = match InternalLuShiftedSolver::new(&k, &m, C64::new(0.0, 0.0)) {
            Ok(_) => panic!("internal LU construction should fail without diagonal entries"),
            Err(err) => err,
        };
        assert!(
            err.contains("no diagonal entry"),
            "missing diagonal should be reported: {err}"
        );
    }

    #[test]
    fn internal_lu_shifted_solver_handles_multiple_rhs() {
        let k = diagonal_csr(2, &[1.0, 2.0]);
        let m = diagonal_csr(2, &[1.0, 1.0]);
        let solver = InternalLuShiftedSolver::new(&k, &m, C64::new(3.0, 0.5)).unwrap();

        let rhs = vec![1.0, 3.0, 2.0, 4.0];
        let result = solver.solve_block(&rhs, 2, 2, 1e-10).unwrap();
        let d0 = C64::new(3.0, 0.5) - C64::new(1.0, 0.0);
        let d1 = C64::new(3.0, 0.5) - C64::new(2.0, 0.0);
        let expected = [
            C64::new(1.0, 0.0) / d0,
            C64::new(3.0, 0.0) / d1,
            C64::new(2.0, 0.0) / d0,
            C64::new(4.0, 0.0) / d1,
        ];
        for i in 0..expected.len() {
            let diff = result[i] - expected[i];
            let err = (diff.re * diff.re + diff.im * diff.im).sqrt();
            assert!(err < 1e-12, "entry {i}: err={err}");
        }
    }

    #[test]
    fn internal_lu_shifted_solver_uses_limited_row_swap_for_zero_pivot_case() {
        let k = CsrMat::try_from_csr_data(2, 2, vec![0, 1, 3], vec![1, 0, 1], vec![1.0, 1.0, 1.0])
            .unwrap();
        let m = diagonal_csr(2, &[1.0, 1.0]);
        let solver = InternalLuShiftedSolver::new(&k, &m, C64::new(0.0, 0.0)).unwrap();

        let rhs = vec![1.0, 2.0];
        let result = solver.solve_block(&rhs, 2, 1, 1e-10).unwrap();
        let expected = [C64::new(-1.0, 0.0), C64::new(-1.0, 0.0)];
        for i in 0..expected.len() {
            let diff = result[i] - expected[i];
            let err = (diff.re * diff.re + diff.im * diff.im).sqrt();
            assert!(err < 1e-12, "entry {i}: err={err}");
        }
    }

    #[test]
    fn pivot_selection_prefers_row_with_stronger_diagonal_dominance() {
        let weak = evaluate_pivot_candidate(
            0,
            BTreeMap::from([
                (0usize, C64::new(10.0, 0.0)),
                (1usize, C64::new(1.0e8, 0.0)),
            ]),
            0,
        )
        .unwrap();
        let strong = evaluate_pivot_candidate(
            1,
            BTreeMap::from([(0usize, C64::new(9.0, 0.0)), (1usize, C64::new(1.0, 0.0))]),
            0,
        )
        .unwrap();

        assert!(
            pivot_candidate_is_better(&strong, &weak),
            "threshold pivoting should prefer the row with stronger diagonal dominance"
        );
    }

    #[test]
    fn pivot_selection_penalizes_large_active_row_norm() {
        let wide = evaluate_pivot_candidate(
            0,
            BTreeMap::from([
                (0usize, C64::new(8.0, 0.0)),
                (1usize, C64::new(20.0, 0.0)),
                (2usize, C64::new(20.0, 0.0)),
            ]),
            0,
        )
        .unwrap();
        let compact = evaluate_pivot_candidate(
            1,
            BTreeMap::from([(0usize, C64::new(7.0, 0.0)), (1usize, C64::new(1.0, 0.0))]),
            0,
        )
        .unwrap();

        assert!(
            pivot_candidate_is_better(&compact, &wide),
            "pivoting should prefer the row whose diagonal is stronger relative to the active row norm"
        );
    }

    #[test]
    fn pivot_window_end_clamps_to_matrix_size() {
        assert_eq!(pivot_window_end(16, 0, 8), 8);
        assert_eq!(pivot_window_end(16, 7, 8), 15);
        assert_eq!(pivot_window_end(16, 8, 8), 16);
        assert_eq!(pivot_window_end(16, 14, 8), 16);
    }
}
