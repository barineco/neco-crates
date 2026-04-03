//! Spectral clustering: Laplacian eigenvectors + k-means.

use neco_eigensolve::{lobpcg_configured, JacobiPreconditioner, LobpcgConfig};
use neco_sparse::CsrMat;

/// Result of spectral clustering.
#[derive(Debug, Clone)]
pub struct SpectralResult {
    /// Cluster assignment for each node (0-based).
    pub assignments: Vec<u32>,
    /// Number of clusters.
    pub n_clusters: u32,
    /// Eigenvectors used for embedding (n_nodes x n_clusters), row-major.
    /// Useful for debugging and visualization.
    pub eigenvectors: Vec<Vec<f64>>,
    /// LOBPCG iteration count.
    pub eigen_iterations: usize,
    /// k-means iteration count.
    pub kmeans_iterations: usize,
}

/// Cluster nodes of a weighted adjacency graph.
///
/// # Panics
///
/// Panics if the matrix is not square or `n_clusters` exceeds the node count.
pub fn spectral_cluster(
    adjacency: &CsrMat<f64>,
    n_clusters: usize,
    tol: f64,
    max_eigen_iter: usize,
    max_kmeans_iter: usize,
) -> SpectralResult {
    let n = adjacency.nrows();
    assert_eq!(n, adjacency.ncols(), "adjacency must be square");
    assert!(
        n_clusters <= n,
        "n_clusters={} exceeds node count {}",
        n_clusters,
        n
    );

    if n_clusters <= 1 || n <= 1 {
        return SpectralResult {
            assignments: vec![0; n],
            n_clusters: 1,
            eigenvectors: Vec::new(),
            eigen_iterations: 0,
            kmeans_iterations: 0,
        };
    }

    // L x = lambda D x, DC deflation off (graph Laplacians have informative zero modes).
    let (l_mat, d_mat) = build_laplacian(adjacency);

    let config = LobpcgConfig {
        n_modes: n_clusters,
        tol,
        max_iter: max_eigen_iter,
        deflate_dc: false,
    };
    let precond = JacobiPreconditioner::new(&l_mat);
    let eigen_result = lobpcg_configured(&l_mat, &d_mat, &config, &precond, |_, _| {});

    // Extract eigenvector embedding: each node gets a k-dimensional vector
    let k = eigen_result.eigenvalues.len();
    let mut embedding = vec![vec![0.0; k]; n];
    for (i, row) in embedding.iter_mut().enumerate() {
        for (j, value) in row.iter_mut().enumerate() {
            *value = eigen_result.eigenvectors[(j, i)];
        }
        // Row-normalize for Normalized Cuts (Ng-Jordan-Weiss)
        let norm: f64 = row.iter().map(|v| v * v).sum::<f64>().sqrt();
        if norm > 1e-15 {
            for v in row {
                *v /= norm;
            }
        }
    }

    // k-means on the embedding (flat row-major)
    let dim = k;
    let flat: Vec<f64> = embedding
        .iter()
        .flat_map(|row| row.iter().copied())
        .collect();
    let km_result = neco_kmeans::kmeans(&flat, dim, k, max_kmeans_iter)
        .expect("spectral embedding must always produce valid k-means inputs");

    SpectralResult {
        assignments: km_result.assignments,
        n_clusters: u32::try_from(k).expect("cluster count fits in u32"),
        eigenvectors: embedding,
        eigen_iterations: eigen_result.iterations,
        kmeans_iterations: km_result.iterations,
    }
}

/// Build unnormalized Laplacian L = D - W and degree matrix D.
fn build_laplacian(adjacency: &CsrMat<f64>) -> (CsrMat<f64>, CsrMat<f64>) {
    let n = adjacency.nrows();

    let mut degrees = vec![0.0; n];
    for (i, degree) in degrees.iter_mut().enumerate() {
        *degree = adjacency.row(i).values().iter().sum();
    }

    let mut l_row_offsets = Vec::with_capacity(n + 1);
    let mut l_col_indices = Vec::new();
    let mut l_values = Vec::new();
    l_row_offsets.push(0);

    for (i, &degree) in degrees.iter().enumerate() {
        let row = adjacency.row(i);
        let cols = row.col_indices();
        let vals = row.values();
        let mut inserted_diag = false;

        for (&col, &val) in cols.iter().zip(vals.iter()) {
            if !inserted_diag && col > i {
                l_col_indices.push(i);
                l_values.push(degree);
                inserted_diag = true;
            }
            if col == i {
                l_col_indices.push(i);
                l_values.push(degree - val);
                inserted_diag = true;
            } else if val.abs() > 1e-20 {
                l_col_indices.push(col);
                l_values.push(-val);
            }
        }

        if !inserted_diag {
            l_col_indices.push(i);
            l_values.push(degree);
        }

        l_row_offsets.push(l_col_indices.len());
    }

    let d_row_offsets: Vec<usize> = (0..=n).collect();
    let d_col_indices: Vec<usize> = (0..n).collect();
    let d_values: Vec<f64> = degrees
        .iter()
        .map(|&degree| if degree > 1e-15 { degree } else { 1.0 })
        .collect();

    let l_mat = CsrMat::try_from_csr_data(n, n, l_row_offsets, l_col_indices, l_values)
        .expect("direct Laplacian CSR construction should be valid");
    let d_mat = CsrMat::try_from_csr_data(n, n, d_row_offsets, d_col_indices, d_values)
        .expect("direct degree CSR construction should be valid");

    (l_mat, d_mat)
}

#[cfg(test)]
mod tests {
    use super::*;
    use neco_sparse::CooMat;
    use std::collections::HashMap;

    /// Build a symmetric adjacency matrix from weighted edges.
    fn adjacency_from_edges(n: usize, edges: &[(usize, usize, f64)]) -> CsrMat<f64> {
        let mut coo = CooMat::new(n, n);
        for &(i, j, w) in edges {
            coo.push(i, j, w);
            coo.push(j, i, w);
        }
        CsrMat::from(&coo)
    }

    /// Normalize cluster labels by first occurrence so label permutations compare equal.
    fn normalize_labels(assignments: &[u32]) -> Vec<u32> {
        let mut label_map = HashMap::new();
        let mut next_label = 0u32;
        let mut normalized = Vec::with_capacity(assignments.len());

        for &label in assignments {
            let mapped = if let Some(&mapped) = label_map.get(&label) {
                mapped
            } else {
                let mapped = next_label;
                label_map.insert(label, mapped);
                next_label += 1;
                mapped
            };
            normalized.push(mapped);
        }

        normalized
    }

    /// Restore assignments from a permuted graph back to the original node order.
    fn restore_assignments(assignments: &[u32], permutation: &[usize]) -> Vec<u32> {
        let mut restored = vec![0; assignments.len()];
        for (old_index, &new_index) in permutation.iter().enumerate() {
            restored[old_index] = assignments[new_index];
        }
        restored
    }

    /// Build a symmetric adjacency matrix after permuting node indices.
    fn permuted_adjacency(
        n: usize,
        edges: &[(usize, usize, f64)],
        permutation: &[usize],
    ) -> CsrMat<f64> {
        let permuted_edges: Vec<(usize, usize, f64)> = edges
            .iter()
            .map(|&(i, j, w)| (permutation[i], permutation[j], w))
            .collect();
        adjacency_from_edges(n, &permuted_edges)
    }

    /// Ring graph with `size` nodes per cluster, `n_clusters` clusters.
    /// Strong internal edges, weak inter-cluster edges forming a chain.
    fn clustered_ring(
        n_clusters: usize,
        size: usize,
        strong: f64,
        weak: f64,
    ) -> (usize, Vec<(usize, usize, f64)>) {
        let n = n_clusters * size;
        let mut edges = Vec::new();
        for c in 0..n_clusters {
            let base = c * size;
            // Ring within cluster + random chords for density
            for i in 0..size {
                let j = (i + 1) % size;
                edges.push((base + i, base + j, strong));
                // Chord: connect i to i+2, i+3 for denser connectivity
                if size > 4 {
                    edges.push((base + i, base + (i + 2) % size, strong * 0.5));
                    edges.push((base + i, base + (i + 3) % size, strong * 0.3));
                }
            }
        }
        // Weak inter-cluster bridges
        for c in 0..n_clusters - 1 {
            edges.push((c * size + size - 1, (c + 1) * size, weak));
        }
        (n, edges)
    }

    /// Two disconnected rings with no bridge between the components.
    fn disconnected_double_ring(size: usize, strong: f64) -> (usize, Vec<(usize, usize, f64)>) {
        let n = size * 2;
        let mut edges = Vec::new();
        for base in [0, size] {
            for i in 0..size {
                let j = (i + 1) % size;
                edges.push((base + i, base + j, strong));
                if size > 4 {
                    edges.push((base + i, base + (i + 2) % size, strong * 0.5));
                }
            }
        }
        (n, edges)
    }

    #[test]
    fn two_clusters_ring() {
        let (n, edges) = clustered_ring(2, 50, 5.0, 0.05);
        let adj = adjacency_from_edges(n, &edges);
        let result = spectral_cluster(&adj, 2, 1e-6, 500, 100);

        assert_eq!(result.assignments.len(), n);
        // Majority vote per cluster
        let c0_mode = result.assignments[0];
        let c1_mode = result.assignments[50];
        assert_ne!(c0_mode, c1_mode, "two clusters should differ");
        let c0_correct = (0..50)
            .filter(|&i| result.assignments[i] == c0_mode)
            .count();
        let c1_correct = (50..100)
            .filter(|&i| result.assignments[i] == c1_mode)
            .count();
        assert!(c0_correct >= 45, "cluster A: {}/50 correct", c0_correct);
        assert!(c1_correct >= 45, "cluster B: {}/50 correct", c1_correct);
    }

    #[test]
    fn three_clusters_ring() {
        let (n, edges) = clustered_ring(3, 40, 5.0, 0.05);
        let adj = adjacency_from_edges(n, &edges);
        let result = spectral_cluster(&adj, 3, 1e-6, 500, 100);

        assert_eq!(result.assignments.len(), n);
        // Each group should have a dominant cluster ID
        let modes: Vec<u32> = (0..3)
            .map(|g| {
                let base = g * 40;
                let mut counts = [0u32; 3];
                for i in base..base + 40 {
                    let a = result.assignments[i] as usize;
                    if a < 3 {
                        counts[a] += 1;
                    }
                }
                counts
                    .iter()
                    .enumerate()
                    .max_by_key(|&(_, &c)| c)
                    .expect("each cluster should have a dominant label")
                    .0
                    .try_into()
                    .expect("cluster index fits in u32")
            })
            .collect();
        // Three distinct cluster IDs
        assert_ne!(modes[0], modes[1]);
        assert_ne!(modes[1], modes[2]);
        assert_ne!(modes[0], modes[2]);
    }

    #[test]
    fn single_cluster() {
        let n = 50;
        let mut edges = Vec::new();
        for i in 0..n {
            edges.push((i, (i + 1) % n, 1.0));
            edges.push((i, (i + 2) % n, 0.5));
        }
        let adj = adjacency_from_edges(n, &edges);
        let result = spectral_cluster(&adj, 1, 1e-8, 100, 100);
        assert!(result.assignments.iter().all(|&a| a == 0));
    }

    #[test]
    fn permutation_invariance_under_node_relabeling() {
        let (n, edges) = clustered_ring(2, 32, 4.0, 0.03);
        let permutation: Vec<usize> = (0..n).rev().collect();
        let adj = adjacency_from_edges(n, &edges);
        let permuted_adj = permuted_adjacency(n, &edges, &permutation);

        let original = spectral_cluster(&adj, 2, 1e-6, 500, 100);
        let permuted = spectral_cluster(&permuted_adj, 2, 1e-6, 500, 100);
        let restored = restore_assignments(&permuted.assignments, &permutation);

        assert_eq!(original.assignments.len(), n);
        assert_eq!(original.n_clusters, 2);
        assert_eq!(permuted.assignments.len(), n);
        assert_eq!(permuted.n_clusters, 2);
        assert_eq!(
            normalize_labels(&original.assignments),
            normalize_labels(&restored),
            "cluster partition should be invariant under node relabeling"
        );
    }

    #[test]
    fn positive_weight_scaling_preserves_partition() {
        let (n, edges) = clustered_ring(3, 24, 3.5, 0.04);
        let scaled_edges: Vec<(usize, usize, f64)> =
            edges.iter().map(|&(i, j, w)| (i, j, w * 7.25)).collect();
        let adj = adjacency_from_edges(n, &edges);
        let scaled_adj = adjacency_from_edges(n, &scaled_edges);

        let original = spectral_cluster(&adj, 3, 1e-6, 500, 100);
        let scaled = spectral_cluster(&scaled_adj, 3, 1e-6, 500, 100);

        assert_eq!(original.assignments.len(), n);
        assert_eq!(scaled.assignments.len(), n);
        assert_eq!(original.n_clusters, 3);
        assert_eq!(scaled.n_clusters, 3);
        assert_eq!(
            normalize_labels(&original.assignments),
            normalize_labels(&scaled.assignments),
            "positive global scaling should not change the clustering"
        );
    }

    #[test]
    fn disconnected_components_stay_separated() {
        let (n, edges) = disconnected_double_ring(18, 4.0);
        let adj = adjacency_from_edges(n, &edges);
        let result = spectral_cluster(&adj, 2, 1e-6, 500, 100);

        assert_eq!(result.assignments.len(), n);
        assert_eq!(result.n_clusters, 2);

        let left_label = result.assignments[0];
        let right_label = result.assignments[18];
        assert_ne!(
            left_label, right_label,
            "components should receive different labels"
        );
        assert!(
            result.assignments[..18]
                .iter()
                .all(|&label| label == left_label),
            "left component should be internally uniform"
        );
        assert!(
            result.assignments[18..]
                .iter()
                .all(|&label| label == right_label),
            "right component should be internally uniform"
        );
    }

    #[test]
    fn isolated_node_does_not_break_assignment_shape() {
        let n = 7;
        let edges = [
            (0, 1, 2.0),
            (1, 2, 2.0),
            (0, 2, 1.0),
            (3, 4, 2.0),
            (4, 5, 2.0),
            (3, 5, 1.0),
        ];
        let adj = adjacency_from_edges(n, &edges);
        let result = spectral_cluster(&adj, 3, 1e-6, 500, 100);

        assert_eq!(result.assignments.len(), n);
        assert_eq!(result.n_clusters, 3);
        assert_eq!(result.eigenvectors.len(), n);
        assert!(
            result
                .assignments
                .iter()
                .all(|&label| label < result.n_clusters),
            "all assignments must stay within the reported cluster range"
        );
    }

    #[test]
    fn laplacian_diagonal_equals_degree() {
        let adj = adjacency_from_edges(3, &[(0, 1, 1.0), (1, 2, 1.0), (0, 2, 1.0)]);
        let (l, _d) = build_laplacian(&adj);

        for i in 0..3 {
            let val = l
                .get(i, i)
                .copied()
                .expect("Laplacian diagonal should be present");
            assert!(
                (val - 2.0).abs() < 1e-10,
                "L[{},{}] = {}, expected 2.0",
                i,
                i,
                val
            );
        }
    }
}
