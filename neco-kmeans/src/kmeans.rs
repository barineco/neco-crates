//! k-means++ initialization + Lloyd's iteration.
use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KmeansError {
    InvalidDimension { dim: usize },
    InvalidDataLength { len: usize, dim: usize },
    EmptyData,
    InvalidClusterCount { k: usize, n_points: usize },
    ClusterCountOverflow { k: usize },
}

impl fmt::Display for KmeansError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimension { dim } => write!(f, "dim must be positive, got {dim}"),
            Self::InvalidDataLength { len, dim } => {
                write!(f, "data length {len} must be a multiple of dim {dim}")
            }
            Self::EmptyData => write!(f, "data must not be empty"),
            Self::InvalidClusterCount { k, n_points } => {
                write!(f, "k={k} out of range 1..={n_points}")
            }
            Self::ClusterCountOverflow { k } => {
                write!(f, "k={k} exceeds u32::MAX (assignments use u32)")
            }
        }
    }
}

impl std::error::Error for KmeansError {}

/// Result of k-means clustering.
#[derive(Debug, Clone)]
pub struct KmeansResult {
    /// Cluster assignment for each input point (0-based).
    pub assignments: Vec<u32>,
    /// Centroid of each cluster, flat k × d (row-major).
    pub centroids: Vec<f64>,
    /// Dimensionality of each point / centroid.
    pub dim: usize,
    /// Number of iterations performed.
    pub iterations: usize,
}

impl KmeansResult {
    /// Returns the centroid of cluster `i` as a slice of length `dim`.
    pub fn centroid(&self, i: usize) -> &[f64] {
        &self.centroids[i * self.dim..(i + 1) * self.dim]
    }

    /// Returns the number of clusters.
    pub fn k(&self) -> usize {
        self.centroids.len() / self.dim
    }
}

/// Cluster `data` (flat row-major, n points × `dim` dimensions) into `k`
/// groups using k-means++ initialization and Lloyd's iteration.
pub fn kmeans(
    data: &[f64],
    dim: usize,
    k: usize,
    max_iter: usize,
) -> Result<KmeansResult, KmeansError> {
    if dim == 0 {
        return Err(KmeansError::InvalidDimension { dim });
    }
    if data.len() % dim != 0 {
        return Err(KmeansError::InvalidDataLength {
            len: data.len(),
            dim,
        });
    }
    let n = data.len() / dim;
    if n == 0 {
        return Err(KmeansError::EmptyData);
    }
    if k == 0 || k > n {
        return Err(KmeansError::InvalidClusterCount { k, n_points: n });
    }
    if k > u32::MAX as usize {
        return Err(KmeansError::ClusterCountOverflow { k });
    }

    let mut centroids = kmeanspp_init(data, dim, k);
    let mut assignments = vec![0u32; n];
    let mut accum = vec![0.0f64; k * dim];
    let mut counts = vec![0u32; k];
    let mut iter_count = 0;

    for iteration in 0..max_iter {
        iter_count = iteration + 1;

        // Assignment step
        let changed = assign_step(data, dim, &centroids, &mut assignments);

        if !changed && iteration > 0 {
            break;
        }

        // Update step
        accum.fill(0.0);
        counts.fill(0);

        for i in 0..n {
            let c = assignments[i] as usize;
            counts[c] += 1;
            let pt = &data[i * dim..(i + 1) * dim];
            let acc = &mut accum[c * dim..(c + 1) * dim];
            for j in 0..dim {
                acc[j] += pt[j];
            }
        }

        for c in 0..k {
            if counts[c] > 0 {
                let cnt = counts[c] as f64;
                let cent = &mut centroids[c * dim..(c + 1) * dim];
                for j in 0..dim {
                    cent[j] = accum[c * dim + j] / cnt;
                }
            }
            // counts[c] == 0: keep previous centroid (no write)
        }
    }

    Ok(KmeansResult {
        assignments,
        centroids,
        dim,
        iterations: iter_count,
    })
}

/// k-means++ initialization: select k initial centroids with
/// distance-weighted sampling (deterministic: max-min).
fn kmeanspp_init(data: &[f64], d: usize, k: usize) -> Vec<f64> {
    let n = data.len() / d;
    let mut centroids = Vec::with_capacity(k * d);

    // Pick first centroid: closest to the mean
    let mut mean = vec![0.0f64; d];
    for pt in data.chunks_exact(d) {
        for j in 0..d {
            mean[j] += pt[j];
        }
    }
    let n_f = n as f64;
    for v in &mut mean {
        *v /= n_f;
    }

    let first = (0..n)
        .min_by(|&a, &b| {
            let da = dist_sq(&data[a * d..(a + 1) * d], &mean);
            let db = dist_sq(&data[b * d..(b + 1) * d], &mean);
            da.total_cmp(&db)
        })
        .unwrap();
    centroids.extend_from_slice(&data[first * d..(first + 1) * d]);

    // Distance-weighted selection (deterministic max-min)
    let mut min_dists = vec![f64::INFINITY; n];

    for c_idx in 1..k {
        let last_start = (c_idx - 1) * d;
        let last = &centroids[last_start..last_start + d];
        for i in 0..n {
            let d2 = dist_sq(&data[i * d..(i + 1) * d], last);
            if d2 < min_dists[i] {
                min_dists[i] = d2;
            }
        }

        let next = (0..n)
            .max_by(|&a, &b| min_dists[a].total_cmp(&min_dists[b]))
            .unwrap();
        centroids.extend_from_slice(&data[next * d..(next + 1) * d]);
    }

    centroids
}

/// Assignment step: find nearest centroid for each point.
/// Returns `true` if any assignment changed.
#[cfg(not(feature = "parallel"))]
fn assign_step(data: &[f64], d: usize, centroids: &[f64], assignments: &mut [u32]) -> bool {
    let mut changed = false;
    for (i, a) in assignments.iter_mut().enumerate() {
        let pt = &data[i * d..(i + 1) * d];
        let nearest = nearest_centroid(pt, centroids);
        if *a != nearest {
            *a = nearest;
            changed = true;
        }
    }
    changed
}

#[cfg(feature = "parallel")]
fn assign_step(data: &[f64], d: usize, centroids: &[f64], assignments: &mut [u32]) -> bool {
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    let changed = AtomicBool::new(false);
    assignments.par_iter_mut().enumerate().for_each(|(i, a)| {
        let pt = &data[i * d..(i + 1) * d];
        let nearest = nearest_centroid(pt, centroids);
        if *a != nearest {
            *a = nearest;
            changed.store(true, Ordering::Relaxed);
        }
    });
    changed.load(Ordering::Relaxed)
}

/// Find the index of the nearest centroid to `point`.
fn nearest_centroid(point: &[f64], centroids: &[f64]) -> u32 {
    let d = point.len();
    debug_assert!(centroids.len() % d == 0);
    let mut best = 0u32;
    let mut best_dist = f64::INFINITY;
    for (i, c) in centroids.chunks_exact(d).enumerate() {
        let dist = dist_sq(point, c);
        if dist < best_dist {
            best_dist = dist;
            best = u32::try_from(i).expect("k validated <= u32::MAX");
        }
    }
    best
}

/// Squared Euclidean distance between two equal-length slices.
#[inline]
fn dist_sq(a: &[f64], b: &[f64]) -> f64 {
    debug_assert_eq!(a.len(), b.len());
    let mut sum = 0.0;
    for i in 0..a.len() {
        let d = a[i] - b[i];
        sum += d * d;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_clusters_1d() {
        let data = [0.0, 1.0, 2.0, 10.0, 11.0, 12.0];
        let result = kmeans(&data, 1, 2, 100).expect("valid clustering must succeed");

        assert_eq!(result.assignments.len(), 6);
        assert_eq!(result.assignments[0], result.assignments[1]);
        assert_eq!(result.assignments[1], result.assignments[2]);
        assert_eq!(result.assignments[3], result.assignments[4]);
        assert_eq!(result.assignments[4], result.assignments[5]);
        assert_ne!(result.assignments[0], result.assignments[3]);
    }

    #[test]
    fn three_clusters_2d() {
        #[rustfmt::skip]
        let data = [
            0.0, 0.1,
            0.1, 0.0,
            -0.1, 0.0,
            10.0, 0.1,
            10.1, 0.0,
            9.9, 0.0,
            5.0, 10.0,
            5.1, 10.1,
            4.9, 9.9,
        ];
        let result = kmeans(&data, 2, 3, 100).expect("valid clustering must succeed");

        assert_eq!(result.assignments.len(), 9);
        assert_eq!(result.centroids.len(), 3 * 2);
        assert_eq!(result.k(), 3);
        assert_eq!(result.assignments[0], result.assignments[1]);
        assert_eq!(result.assignments[1], result.assignments[2]);
        assert_eq!(result.assignments[3], result.assignments[4]);
        assert_eq!(result.assignments[4], result.assignments[5]);
        assert_eq!(result.assignments[6], result.assignments[7]);
        assert_eq!(result.assignments[7], result.assignments[8]);
        let a = result.assignments[0];
        let b = result.assignments[3];
        let c = result.assignments[6];
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert_ne!(a, c);
    }

    #[test]
    fn single_cluster() {
        let data = [1.0, 2.0, 1.1, 2.1, 0.9, 1.9];
        let result = kmeans(&data, 2, 1, 100).expect("valid clustering must succeed");

        assert!(result.assignments.iter().all(|&a| a == 0));
        assert_eq!(result.centroids.len(), 2);
        assert_eq!(result.k(), 1);
    }

    #[test]
    fn k_equals_n() {
        let data = [0.0, 10.0, 20.0];
        let result = kmeans(&data, 1, 3, 100).expect("valid clustering must succeed");

        let mut ids: Vec<u32> = result.assignments.clone();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn converges_in_few_iterations() {
        let data = [0.0, 0.0, 100.0, 100.0];
        let result = kmeans(&data, 1, 2, 100).expect("valid clustering must succeed");

        assert!(
            result.iterations <= 5,
            "took {} iterations",
            result.iterations
        );
    }

    #[test]
    fn centroid_accessor() {
        let data = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let result = kmeans(&data, 2, 2, 100).expect("valid clustering must succeed");
        for i in 0..result.k() {
            assert_eq!(result.centroid(i).len(), 2);
        }
    }

    #[test]
    fn invalid_inputs_return_errors() {
        let data = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(
            kmeans(&data, 0, 2, 100).expect_err("zero dimension must be rejected"),
            KmeansError::InvalidDimension { dim: 0 }
        );
        assert_eq!(
            kmeans(&data[..3], 2, 2, 100).expect_err("ragged data must be rejected"),
            KmeansError::InvalidDataLength { len: 3, dim: 2 }
        );
        assert_eq!(
            kmeans(&[], 2, 1, 100).expect_err("empty data must be rejected"),
            KmeansError::EmptyData
        );
        assert_eq!(
            kmeans(&data, 2, 0, 100).expect_err("zero clusters must be rejected"),
            KmeansError::InvalidClusterCount { k: 0, n_points: 2 }
        );
    }
}
