use std::collections::HashSet;

use neco_sparse::CsrMat;

use crate::dense::symmetric_dense_kernel;
use crate::DenseMatrix;

/// Result of a Craig-Bampton reduction.
#[derive(Debug)]
pub struct CraigBamptonReduction {
    /// Reduced stiffness matrix (n_modes + n_boundary) × (n_modes + n_boundary).
    pub k_reduced: DenseMatrix,
    /// Reduced mass matrix.
    pub m_reduced: DenseMatrix,
    /// Number of retained interior modes.
    pub n_interior_modes: usize,
    /// Number of boundary DOFs.
    pub n_boundary_dofs: usize,
    /// Transformation matrix T (full_dof × reduced_dof).
    pub transform: DenseMatrix,
    /// Interior DOF indices in the original system.
    pub interior_dofs: Vec<usize>,
    /// Boundary DOF indices in the original system.
    pub boundary_dofs: Vec<usize>,
}

fn check_dense_capacity(rows: usize, cols: usize, label: &str) -> Result<(), String> {
    let max_elements = usize::MAX / 8;
    if rows.checked_mul(cols).is_none_or(|n| n > max_elements) {
        return Err(format!(
            "CMS: {} matrix ({}x{}) exceeds the dense allocation limit",
            label, rows, cols
        ));
    }
    Ok(())
}

fn csr_times_dense(csr: &CsrMat<f64>, dense: &DenseMatrix) -> DenseMatrix {
    let nrows = csr.nrows();
    let ncols = dense.ncols();
    let mut out = DenseMatrix::zeros(nrows, ncols);
    for row_idx in 0..nrows {
        let row = csr.row(row_idx);
        for (&col, &val) in row.col_indices().iter().zip(row.values()) {
            for dense_col in 0..ncols {
                let updated = out.get(row_idx, dense_col) + val * dense.get(col, dense_col);
                out.set(row_idx, dense_col, updated);
            }
        }
    }
    out
}

fn solve_generalized_modes_dense(
    k_ii: &DenseMatrix,
    m_ii: &DenseMatrix,
    n_interior_modes: usize,
) -> Result<DenseMatrix, String> {
    let eigen = symmetric_dense_kernel()
        .generalized_symmetric_eigen(&k_ii.to_row_major(), &m_ii.to_row_major(), k_ii.nrows())
        .ok_or("failed to solve interior generalized eigenproblem")?;
    let eigenvectors = DenseMatrix::from_row_major(k_ii.nrows(), k_ii.ncols(), &eigen.eigenvectors);

    let mut eig_pairs: Vec<(f64, usize)> = eigen
        .eigenvalues
        .iter()
        .enumerate()
        .map(|(i, &lam)| (lam, i))
        .collect();
    eig_pairs.sort_by(|a, b| a.0.total_cmp(&b.0));

    let non_dc: Vec<(f64, usize)> = eig_pairs
        .into_iter()
        .filter(|&(lam, _)| lam > 1e-6)
        .collect();
    let take = non_dc.len().min(n_interior_modes);

    let ni = k_ii.nrows();
    let mut phi_ii = DenseMatrix::zeros(ni, take);
    for (mode_idx, &(_lam, eig_idx)) in non_dc[..take].iter().enumerate() {
        let psi = eigenvectors.column(eig_idx);
        let m_psi = m_ii.mul_vector(psi);
        let norm = psi
            .iter()
            .zip(&m_psi)
            .map(|(a, b)| a * b)
            .sum::<f64>()
            .sqrt();
        if norm > 1e-15 {
            let normalized: Vec<f64> = psi.iter().map(|value| value / norm).collect();
            phi_ii.set_column(mode_idx, &normalized);
        }
    }
    Ok(phi_ii)
}

fn solve_constraint_modes_dense(
    k_ii: &DenseMatrix,
    k_ib: &DenseMatrix,
) -> Result<DenseMatrix, String> {
    let rhs: Vec<f64> = k_ib
        .to_row_major()
        .into_iter()
        .map(|value| -value)
        .collect();
    let solved = symmetric_dense_kernel()
        .lu_solve_multiple(&k_ii.to_row_major(), &rhs, k_ii.nrows(), k_ib.ncols())
        .ok_or("failed to solve for constraint modes".to_string())?;
    Ok(DenseMatrix::from_row_major(
        k_ii.nrows(),
        k_ib.ncols(),
        &solved,
    ))
}

#[allow(clippy::type_complexity)]
fn extract_submatrices(
    mat: &CsrMat<f64>,
    interior: &[usize],
    boundary: &[usize],
) -> Result<(DenseMatrix, DenseMatrix, DenseMatrix), String> {
    let ni = interior.len();
    let nb = boundary.len();

    check_dense_capacity(ni, ni, "M_ii")?;
    check_dense_capacity(ni, nb, "M_ib")?;
    check_dense_capacity(nb, nb, "M_bb")?;

    let n = mat.nrows();
    let mut interior_map = vec![usize::MAX; n];
    let mut boundary_map = vec![usize::MAX; n];
    for (new, &old) in interior.iter().enumerate() {
        interior_map[old] = new;
    }
    for (new, &old) in boundary.iter().enumerate() {
        boundary_map[old] = new;
    }

    let mut mii = DenseMatrix::zeros(ni, ni);
    let mut mib = DenseMatrix::zeros(ni, nb);
    let mut mbb = DenseMatrix::zeros(nb, nb);

    for &old_row in interior {
        let new_row = interior_map[old_row];
        let row = mat.row(old_row);
        for (&col, &val) in row.col_indices().iter().zip(row.values()) {
            if interior_map[col] != usize::MAX {
                mii.set(new_row, interior_map[col], val);
            } else if boundary_map[col] != usize::MAX {
                mib.set(new_row, boundary_map[col], val);
            }
        }
    }

    for &old_row in boundary {
        let new_row = boundary_map[old_row];
        let row = mat.row(old_row);
        for (&col, &val) in row.col_indices().iter().zip(row.values()) {
            if boundary_map[col] != usize::MAX {
                mbb.set(new_row, boundary_map[col], val);
            }
        }
    }

    Ok((mii, mib, mbb))
}

/// Perform Craig-Bampton reduction on a sparse generalized eigenproblem.
pub fn craig_bampton_reduce(
    k: &CsrMat<f64>,
    m: &CsrMat<f64>,
    boundary_dofs: &[usize],
    n_interior_modes: usize,
) -> Result<CraigBamptonReduction, String> {
    let n = k.nrows();
    let boundary_set: HashSet<usize> = boundary_dofs.iter().copied().collect();

    let interior_dofs: Vec<usize> = (0..n).filter(|d| !boundary_set.contains(d)).collect();
    let ni = interior_dofs.len();
    let nb = boundary_dofs.len();

    if n_interior_modes > ni {
        return Err(format!(
            "requested interior mode count {} exceeds interior DOF count {}",
            n_interior_modes, ni
        ));
    }

    let (k_ii, k_ib, _k_bb) = extract_submatrices(k, &interior_dofs, boundary_dofs)?;
    let (m_ii, _m_ib, _m_bb) = extract_submatrices(m, &interior_dofs, boundary_dofs)?;

    let phi_ii = solve_generalized_modes_dense(&k_ii, &m_ii, n_interior_modes)?;
    let take = phi_ii.ncols();
    let constraint_modes = solve_constraint_modes_dense(&k_ii, &k_ib)?;

    let n_red = take + nb;
    let mut transform = DenseMatrix::zeros(n, n_red);

    for (local_i, &global_i) in interior_dofs.iter().enumerate() {
        for mode in 0..take {
            transform[(global_i, mode)] = phi_ii[(local_i, mode)];
        }
        for bj in 0..nb {
            transform[(global_i, take + bj)] = constraint_modes[(local_i, bj)];
        }
    }

    for (local_b, &global_b) in boundary_dofs.iter().enumerate() {
        transform[(global_b, take + local_b)] = 1.0;
    }

    let kt = csr_times_dense(k, &transform);
    let k_reduced = transform.transpose_mul(&kt);
    let mt = csr_times_dense(m, &transform);
    let m_reduced = transform.transpose_mul(&mt);

    Ok(CraigBamptonReduction {
        k_reduced,
        m_reduced,
        n_interior_modes: take,
        n_boundary_dofs: nb,
        transform,
        interior_dofs,
        boundary_dofs: boundary_dofs.to_vec(),
    })
}

/// Couple two Craig-Bampton reduced systems by eliminating duplicated interface DOFs.
pub fn couple_cb_systems(
    cb_a: &CraigBamptonReduction,
    cb_b: &CraigBamptonReduction,
    interface_pairs: &[(usize, usize)],
) -> (DenseMatrix, DenseMatrix) {
    let na = cb_a.k_reduced.nrows();
    let nb = cb_b.k_reduced.nrows();
    let n_total = na + nb;

    let mut k_block = DenseMatrix::zeros(n_total, n_total);
    let mut m_block = DenseMatrix::zeros(n_total, n_total);
    for row in 0..na {
        for col in 0..na {
            k_block[(row, col)] = cb_a.k_reduced[(row, col)];
            m_block[(row, col)] = cb_a.m_reduced[(row, col)];
        }
    }
    for row in 0..nb {
        for col in 0..nb {
            k_block[(na + row, na + col)] = cb_b.k_reduced[(row, col)];
            m_block[(na + row, na + col)] = cb_b.m_reduced[(row, col)];
        }
    }

    let n_interface = interface_pairs.len();
    let n_coupled = n_total - n_interface;
    let eliminated: HashSet<usize> = interface_pairs
        .iter()
        .map(|&(_, b_idx)| na + cb_b.n_interior_modes + b_idx)
        .collect();
    let kept_dofs: Vec<usize> = (0..n_total).filter(|d| !eliminated.contains(d)).collect();

    let mut l_mat = DenseMatrix::zeros(n_total, n_coupled);
    let mut old_to_new = vec![usize::MAX; n_total];
    for (new_idx, &old_idx) in kept_dofs.iter().enumerate() {
        old_to_new[old_idx] = new_idx;
        l_mat[(old_idx, new_idx)] = 1.0;
    }

    for &(a_idx, b_idx) in interface_pairs {
        let a_global = cb_a.n_interior_modes + a_idx;
        let b_global = na + cb_b.n_interior_modes + b_idx;
        let a_new = old_to_new[a_global];
        l_mat[(b_global, a_new)] = 1.0;
    }

    let k_coupled = l_mat.transpose_mul(&k_block).mul(&l_mat);
    let m_coupled = l_mat.transpose_mul(&m_block).mul(&l_mat);
    (k_coupled, m_coupled)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn csr_from_dense(rows: &[&[f64]]) -> CsrMat<f64> {
        let nrows = rows.len();
        let ncols = rows.first().map_or(0, |row| row.len());
        let mut row_offsets = Vec::with_capacity(nrows + 1);
        let mut col_indices = Vec::new();
        let mut values = Vec::new();
        row_offsets.push(0);
        for row in rows {
            for (col, &value) in row.iter().enumerate() {
                if value != 0.0 {
                    col_indices.push(col);
                    values.push(value);
                }
            }
            row_offsets.push(col_indices.len());
        }
        CsrMat::try_from_csr_data(nrows, ncols, row_offsets, col_indices, values).unwrap()
    }

    fn dense_eigenvalues(k: &DenseMatrix, m: &DenseMatrix) -> Vec<f64> {
        let eigen = symmetric_dense_kernel()
            .generalized_symmetric_eigen(&k.to_row_major(), &m.to_row_major(), k.nrows())
            .expect("generalized eig");
        let mut vals: Vec<f64> = eigen
            .eigenvalues
            .iter()
            .copied()
            .filter(|lam| *lam > 1e-6)
            .collect();
        vals.sort_by(f64::total_cmp);
        vals
    }

    #[test]
    fn cb_diagonal_system_preserves_modes() {
        let k = csr_from_dense(&[
            &[4.0, 0.0, 0.0, 0.0],
            &[0.0, 9.0, 0.0, 0.0],
            &[0.0, 0.0, 16.0, 0.0],
            &[0.0, 0.0, 0.0, 25.0],
        ]);
        let m = csr_from_dense(&[
            &[1.0, 0.0, 0.0, 0.0],
            &[0.0, 1.0, 0.0, 0.0],
            &[0.0, 0.0, 1.0, 0.0],
            &[0.0, 0.0, 0.0, 1.0],
        ]);

        let cb = craig_bampton_reduce(&k, &m, &[2, 3], 2).expect("CB reduction should succeed");
        assert_eq!(cb.n_interior_modes, 2);
        assert_eq!(cb.n_boundary_dofs, 2);

        let eigs = dense_eigenvalues(&cb.k_reduced, &cb.m_reduced);
        assert_eq!(eigs.len(), 4);
        let expected = [4.0, 9.0, 16.0, 25.0];
        for i in 0..expected.len() {
            assert!((eigs[i] - expected[i]).abs() < 1e-10, "i={i}: {}", eigs[i]);
        }
    }

    #[test]
    fn cb_reduction_rejects_too_many_interior_modes() {
        let k = csr_from_dense(&[&[2.0, 0.0], &[0.0, 3.0]]);
        let m = csr_from_dense(&[&[1.0, 0.0], &[0.0, 1.0]]);
        let err = craig_bampton_reduce(&k, &m, &[1], 2).unwrap_err();
        assert!(err.contains("exceeds"), "unexpected error: {err}");
    }

    #[test]
    fn coupled_system_eliminates_duplicate_interface_dof() {
        let cb_a = CraigBamptonReduction {
            k_reduced: DenseMatrix::from_diagonal_element(2, 2, 2.0),
            m_reduced: DenseMatrix::identity(2, 2),
            n_interior_modes: 1,
            n_boundary_dofs: 1,
            transform: DenseMatrix::identity(2, 2),
            interior_dofs: vec![0],
            boundary_dofs: vec![1],
        };
        let cb_b = CraigBamptonReduction {
            k_reduced: DenseMatrix::from_diagonal_element(2, 2, 3.0),
            m_reduced: DenseMatrix::identity(2, 2),
            n_interior_modes: 1,
            n_boundary_dofs: 1,
            transform: DenseMatrix::identity(2, 2),
            interior_dofs: vec![0],
            boundary_dofs: vec![1],
        };

        let (k_coupled, m_coupled) = couple_cb_systems(&cb_a, &cb_b, &[(0, 0)]);
        assert_eq!(k_coupled.nrows(), 3);
        assert_eq!(m_coupled.nrows(), 3);
        assert!(
            (k_coupled[(1, 1)] - 5.0).abs() < 1e-10,
            "shared interface stiffness should add"
        );
        assert!(
            (m_coupled[(1, 1)] - 2.0).abs() < 1e-10,
            "shared interface mass should add"
        );
    }
}
