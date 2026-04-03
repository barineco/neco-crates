//! 2D Constrained Delaunay Triangulation (CDT).
//!
//! Bowyer-Watson incremental insertion + edge-flip constraint recovery.
//! Uses crate-local adaptive predicates for exact orientation and in-circle evaluation.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use crate::predicates::{incircle, orient2d};

/// Sentinel for no adjacent triangle.
const NONE: usize = usize::MAX;

/// 2D Constrained Delaunay Triangulation.
#[derive(Debug, Clone)]
pub struct Cdt {
    /// Vertex coordinates (first 3 are the super-triangle).
    vertices: Vec<[f64; 2]>,
    /// Triangles (CCW vertex-index triples).
    triangles: Vec<[usize; 3]>,
    /// Adjacent triangles. `adjacency[t][i]` = triangle adjacent to edge (v[(i+1)%3], v[(i+2)%3]).
    adjacency: Vec<[usize; 3]>,
    /// Constraint edge set (normalized: (min, max)).
    constraints: HashSet<(usize, usize)>,
    /// Number of super-triangle vertices (always 3).
    n_super: usize,
    /// Start hint for walk-based point location.
    last_triangle: usize,
}

/// Errors produced by constrained edge recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CdtError {
    /// Constraint edge recovery did not converge within the iteration budget.
    ConstraintRecoveryDidNotConverge {
        /// User-vertex indices of the failed edge.
        edge: (usize, usize),
        /// Maximum number of iterations attempted.
        iterations: usize,
    },
}

impl fmt::Display for CdtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConstraintRecoveryDidNotConverge { edge, iterations } => write!(
                f,
                "constraint edge ({}, {}) recovery did not converge after {} iterations",
                edge.0, edge.1, iterations
            ),
        }
    }
}

impl Error for CdtError {}

/// Normalized edge key.
#[inline]
fn edge_key(a: usize, b: usize) -> (usize, usize) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

impl Cdt {
    /// Initialize an empty CDT with a super-triangle.
    ///
    /// Generates a super-triangle large enough to enclose `bounds` = (min_x, min_y, max_x, max_y).
    pub fn new(bounds: (f64, f64, f64, f64)) -> Self {
        let (min_x, min_y, max_x, max_y) = bounds;
        let dx = max_x - min_x;
        let dy = max_y - min_y;
        let d = dx.max(dy).max(1e-10);
        let cx = (min_x + max_x) * 0.5;
        let cy = (min_y + max_y) * 0.5;

        // Super-triangle: sufficiently large equilateral triangle
        let margin = 10.0 * d;
        let v0 = [cx - margin, cy - margin];
        let v1 = [cx + margin, cy - margin];
        let v2 = [cx, cy + margin];

        Cdt {
            vertices: vec![v0, v1, v2],
            triangles: vec![[0, 1, 2]],
            adjacency: vec![[NONE; 3]],
            constraints: HashSet::new(),
            n_super: 3,
            last_triangle: 0,
        }
    }

    /// Insert a vertex and return its index.
    ///
    /// Bowyer-Watson incremental insertion.
    pub fn insert(&mut self, x: f64, y: f64) -> usize {
        let vi = self.vertices.len();
        self.vertices.push([x, y]);

        // Find the triangle containing the point
        let ti = match self.locate(x, y) {
            Some(t) => t,
            None => {
                // Outside all triangles (beyond super-triangle) -- should not happen, fallback
                // Use the nearest triangle
                self.nearest_triangle(x, y)
            }
        };

        // Bowyer-Watson: collect triangles whose circumcircle contains the point
        let bad = self.find_bad_triangles(vi, ti);

        // Find boundary edges of bad triangles
        let boundary = self.find_boundary(&bad);

        // Remove bad triangles and fill with new ones
        self.re_triangulate(vi, &bad, &boundary);

        vi
    }

    /// Add constraint edges.
    ///
    /// Inserts each point and registers constraint edges between consecutive points.
    /// If `closed` is true, also connects the last point to the first.
    pub fn add_constraint_edges(
        &mut self,
        points: &[[f64; 2]],
        closed: bool,
    ) -> Result<(), CdtError> {
        if points.is_empty() {
            return Ok(());
        }

        // Reuse exact duplicate vertices so closed polylines and sampled curves
        // do not create overlapping vertices that make constraint recovery fail.
        let indices: Vec<usize> = points
            .iter()
            .map(|p| self.find_or_insert_vertex(p[0], p[1]))
            .collect();

        // Register and recover constraint edges
        let n = indices.len();
        for i in 0..n {
            let next = if i + 1 < n {
                i + 1
            } else if closed {
                0
            } else {
                break;
            };
            let a = indices[i];
            let b = indices[next];
            if a == b {
                continue;
            }
            let chain = self.constraint_chain(a, b);
            for edge in chain.windows(2) {
                let start = edge[0];
                let end = edge[1];
                if start != end {
                    self.enforce_constraint(start, end)?;
                }
            }
        }

        Ok(())
    }

    /// Return the number of user vertices (excluding super-triangle).
    pub fn n_user_vertices(&self) -> usize {
        self.vertices.len() - self.n_super
    }

    /// Convert a vertex index to a user index (without super-triangle offset).
    #[inline]
    fn user_index(&self, vi: usize) -> usize {
        debug_assert!(vi >= self.n_super);
        vi - self.n_super
    }

    /// Return triangles that do not include super-triangle vertices.
    /// Vertex indices in the returned triangles are user indices (0-based).
    pub fn triangles(&self) -> Vec<[usize; 3]> {
        let mut result = Vec::new();
        for tri in &self.triangles {
            // Exclude triangles containing super-triangle vertices
            if tri[0] < self.n_super || tri[1] < self.n_super || tri[2] < self.n_super {
                continue;
            }
            result.push([
                self.user_index(tri[0]),
                self.user_index(tri[1]),
                self.user_index(tri[2]),
            ]);
        }
        result
    }

    /// Return user vertex coordinates (excluding super-triangle).
    pub fn user_vertices(&self) -> &[[f64; 2]] {
        &self.vertices[self.n_super..]
    }

    // ── Internal methods ──

    /// Find the triangle containing point (x, y) by adjacency walk.
    fn locate(&self, x: f64, y: f64) -> Option<usize> {
        if self.triangles.is_empty() {
            return None;
        }

        let p = [x, y];
        let mut ti = self
            .last_triangle
            .min(self.triangles.len().saturating_sub(1));

        for _ in 0..self.triangles.len() {
            let tri = self.triangles[ti];
            let a = self.vertices[tri[0]];
            let b = self.vertices[tri[1]];
            let c = self.vertices[tri[2]];

            let ab = orient2d(a, b, p);
            let bc = orient2d(b, c, p);
            let ca = orient2d(c, a, p);

            if ab >= 0.0 && bc >= 0.0 && ca >= 0.0 {
                return Some(ti);
            }

            let edge = if ab < bc && ab < ca {
                0
            } else if bc < ca {
                1
            } else {
                2
            };

            let adj = self.adjacency[ti][edge];
            if adj == NONE {
                return self.locate_linear(x, y);
            }
            ti = adj;
        }

        self.locate_linear(x, y)
    }

    /// Linear fallback for point location.
    fn locate_linear(&self, x: f64, y: f64) -> Option<usize> {
        let p = [x, y];
        for (ti, tri) in self.triangles.iter().enumerate() {
            let a = self.vertices[tri[0]];
            let b = self.vertices[tri[1]];
            let c = self.vertices[tri[2]];
            if orient2d(a, b, p) >= 0.0 && orient2d(b, c, p) >= 0.0 && orient2d(c, a, p) >= 0.0 {
                return Some(ti);
            }
        }
        None
    }

    /// Nearest triangle (fallback).
    fn nearest_triangle(&self, x: f64, y: f64) -> usize {
        let mut best = 0;
        let mut best_dist = f64::INFINITY;
        for (ti, tri) in self.triangles.iter().enumerate() {
            let cx =
                (self.vertices[tri[0]][0] + self.vertices[tri[1]][0] + self.vertices[tri[2]][0])
                    / 3.0;
            let cy =
                (self.vertices[tri[0]][1] + self.vertices[tri[1]][1] + self.vertices[tri[2]][1])
                    / 3.0;
            let d = (cx - x) * (cx - x) + (cy - y) * (cy - y);
            if d < best_dist {
                best_dist = d;
                best = ti;
            }
        }
        best
    }

    fn find_or_insert_vertex(&mut self, x: f64, y: f64) -> usize {
        if let Some((offset, _)) = self.vertices[self.n_super..]
            .iter()
            .enumerate()
            .find(|(_, v)| v[0] == x && v[1] == y)
        {
            return self.n_super + offset;
        }
        self.insert(x, y)
    }

    /// Bowyer-Watson: BFS to collect connected triangles whose circumcircle contains vertex vi.
    fn find_bad_triangles(&self, vi: usize, start: usize) -> Vec<usize> {
        let p = self.vertices[vi];
        let mut bad = Vec::new();
        let mut stack = vec![start];
        let mut visited = vec![false; self.triangles.len()];

        while let Some(ti) = stack.pop() {
            if visited[ti] {
                continue;
            }
            visited[ti] = true;
            let tri = self.triangles[ti];
            let a = self.vertices[tri[0]];
            let b = self.vertices[tri[1]];
            let c = self.vertices[tri[2]];

            let o = orient2d(a, b, c);
            let in_circle = if o > 0.0 {
                incircle(a, b, c, p) > 0.0
            } else if o < 0.0 {
                incircle(b, a, c, p) > 0.0
            } else {
                true
            };

            if in_circle {
                bad.push(ti);
                for &adj in &self.adjacency[ti] {
                    if adj != NONE {
                        stack.push(adj);
                    }
                }
            }
        }
        bad
    }

    /// Find boundary edges of the bad triangle set.
    fn find_boundary(&self, bad: &[usize]) -> Vec<(usize, usize, usize)> {
        let mut is_bad = vec![false; self.triangles.len()];
        for &ti in bad {
            is_bad[ti] = true;
        }
        let mut boundary = Vec::new();

        for &ti in bad {
            let tri = self.triangles[ti];
            for i in 0..3 {
                let adj = self.adjacency[ti][i];
                let va = tri[(i + 1) % 3];
                let vb = tri[(i + 2) % 3];
                if adj == NONE || !is_bad[adj] {
                    boundary.push((va, vb, adj));
                }
            }
        }
        boundary
    }

    /// Remove bad triangles and create new triangles from vertex vi to boundary edges.
    fn re_triangulate(&mut self, vi: usize, bad: &[usize], boundary: &[(usize, usize, usize)]) {
        let mut bad_sorted: Vec<usize> = bad.to_vec();
        bad_sorted.sort_unstable();

        let n_new = boundary.len();

        let mut removed: HashSet<usize> = bad_sorted.iter().copied().collect();

        let mut new_indices = Vec::with_capacity(n_new);

        let mut reuse: Vec<usize> = bad_sorted.clone();
        for _ in 0..n_new {
            if let Some(slot) = reuse.pop() {
                new_indices.push(slot);
                removed.remove(&slot);
            } else {
                let idx = self.triangles.len();
                self.triangles.push([0; 3]);
                self.adjacency.push([NONE; 3]);
                new_indices.push(idx);
            }
        }

        let mut remaining_bad: Vec<usize> = removed.into_iter().collect();
        remaining_bad.sort_unstable();

        use std::collections::HashMap;
        let mut vertex_to_new: HashMap<usize, usize> = HashMap::new();
        for (k, &(va, _vb, _)) in boundary.iter().enumerate() {
            vertex_to_new.insert(va, k);
        }

        for (k, &(va, vb, adj_outside)) in boundary.iter().enumerate() {
            let new_ti = new_indices[k];
            self.triangles[new_ti] = [vi, va, vb];

            self.adjacency[new_ti][0] = adj_outside;

            self.adjacency[new_ti][1] = if let Some(&other_k) = vertex_to_new.get(&vb) {
                new_indices[other_k]
            } else {
                NONE
            };

            let mut adj2 = NONE;
            for (j, &(_jva, jvb, _)) in boundary.iter().enumerate() {
                if jvb == va {
                    adj2 = new_indices[j];
                    break;
                }
            }
            self.adjacency[new_ti][2] = adj2;

            if adj_outside != NONE {
                for i in 0..3 {
                    if bad_sorted.contains(&self.adjacency[adj_outside][i])
                        || self.adjacency[adj_outside][i] == NONE
                    {
                        let ot = self.triangles[adj_outside];
                        let ea = ot[(i + 1) % 3];
                        let eb = ot[(i + 2) % 3];
                        if (ea == va && eb == vb) || (ea == vb && eb == va) {
                            self.adjacency[adj_outside][i] = new_ti;
                            break;
                        }
                    }
                }
            }
        }

        remaining_bad.sort_unstable_by(|a, b| b.cmp(a));
        for &slot in &remaining_bad {
            let last = self.triangles.len() - 1;
            if slot < last {
                self.triangles[slot] = self.triangles[last];
                self.adjacency[slot] = self.adjacency[last];
                for i in 0..3 {
                    let adj = self.adjacency[slot][i];
                    if adj != NONE && adj < self.triangles.len() {
                        for j in 0..3 {
                            if self.adjacency[adj][j] == last {
                                self.adjacency[adj][j] = slot;
                            }
                        }
                    }
                }
                for ni in new_indices.iter_mut() {
                    if *ni == last {
                        *ni = slot;
                    }
                }
            }
            self.triangles.pop();
            self.adjacency.pop();
        }

        self.last_triangle = new_indices.first().copied().unwrap_or(0);
    }

    /// Enforce constraint edge (a, b).
    fn enforce_constraint(&mut self, a: usize, b: usize) -> Result<(), CdtError> {
        let key = edge_key(a, b);
        self.constraints.insert(key);

        if self.find_edge_triangle(a, b).is_some() {
            return Ok(());
        }

        let max_iter = self.triangles.len() * 4;
        for _ in 0..max_iter {
            if self.find_edge_triangle(a, b).is_some() {
                return Ok(());
            }

            if !self.flip_one_crossing_edge(a, b) {
                if self.find_edge_triangle(a, b).is_some() {
                    return Ok(());
                }
                return Err(CdtError::ConstraintRecoveryDidNotConverge {
                    edge: (self.user_index(a), self.user_index(b)),
                    iterations: max_iter,
                });
            }
        }

        Err(CdtError::ConstraintRecoveryDidNotConverge {
            edge: (self.user_index(a), self.user_index(b)),
            iterations: max_iter,
        })
    }

    fn constraint_chain(&self, a: usize, b: usize) -> Vec<usize> {
        let pa = self.vertices[a];
        let pb = self.vertices[b];
        let ab = [pb[0] - pa[0], pb[1] - pa[1]];
        let ab_len_sq = ab[0] * ab[0] + ab[1] * ab[1];
        if ab_len_sq <= 1e-24 {
            return vec![a, b];
        }

        let tol = 1e-12 * ab_len_sq.sqrt().max(1.0);
        let mut interior = Vec::new();
        for vi in self.n_super..self.vertices.len() {
            if vi == a || vi == b {
                continue;
            }
            let p = self.vertices[vi];
            let ap = [p[0] - pa[0], p[1] - pa[1]];
            let cross = orient2d(pa, pb, p).abs();
            if cross > tol {
                continue;
            }
            let dot = ap[0] * ab[0] + ap[1] * ab[1];
            if dot <= tol || dot >= ab_len_sq - tol {
                continue;
            }
            interior.push((dot / ab_len_sq, vi));
        }

        interior.sort_unstable_by(|(ta, _), (tb, _)| ta.total_cmp(tb));
        let mut chain = Vec::with_capacity(interior.len() + 2);
        chain.push(a);
        chain.extend(interior.into_iter().map(|(_, vi)| vi));
        chain.push(b);
        chain
    }

    fn find_edge_triangle(&self, a: usize, b: usize) -> Option<(usize, usize)> {
        for (ti, tri) in self.triangles.iter().enumerate() {
            for i in 0..3 {
                let va = tri[(i + 1) % 3];
                let vb = tri[(i + 2) % 3];
                if (va == a && vb == b) || (va == b && vb == a) {
                    return Some((ti, i));
                }
            }
        }
        None
    }

    fn flip_one_crossing_edge(&mut self, a: usize, b: usize) -> bool {
        let pa = self.vertices[a];
        let pb = self.vertices[b];

        let n_tri = self.triangles.len();
        for ti in 0..n_tri {
            let tri = self.triangles[ti];
            for i in 0..3 {
                let va = tri[(i + 1) % 3];
                let vb = tri[(i + 2) % 3];

                if va == a || va == b || vb == a || vb == b {
                    continue;
                }
                if self.is_constraint(va, vb) {
                    continue;
                }

                let pva = self.vertices[va];
                let pvb = self.vertices[vb];

                if segments_intersect(pa, pb, pva, pvb) {
                    let adj = self.adjacency[ti][i];
                    if adj != NONE {
                        self.flip_edge(ti, adj, i);
                        return true;
                    }
                }
            }
        }
        false
    }

    #[inline]
    fn is_constraint(&self, a: usize, b: usize) -> bool {
        self.constraints.contains(&edge_key(a, b))
    }

    fn flip_edge(&mut self, t1: usize, t2: usize, edge_in_t1: usize) {
        let tri1 = self.triangles[t1];
        let tri2 = self.triangles[t2];

        let e = edge_in_t1;
        let shared_a = tri1[(e + 1) % 3];
        let shared_b = tri1[(e + 2) % 3];
        let apex1 = tri1[e];

        let edge_in_t2 = self.find_local_edge(t2, shared_a, shared_b);
        if edge_in_t2 == NONE {
            return;
        }
        let apex2 = tri2[edge_in_t2];

        self.triangles[t1] = [apex1, apex2, shared_b];
        self.triangles[t2] = [apex2, apex1, shared_a];

        let old_adj1 = self.adjacency[t1];
        let old_adj2 = self.adjacency[t2];

        let t1_adj_sb_apex1 = old_adj1[(e + 1) % 3];
        let t1_adj_apex1_sa = old_adj1[(e + 2) % 3];

        let t2_adj_sb_apex2 = {
            let mut found = NONE;
            for i in 0..3 {
                let va = tri2[(i + 1) % 3];
                let vb = tri2[(i + 2) % 3];
                if (va == shared_b && vb == apex2) || (va == apex2 && vb == shared_b) {
                    found = old_adj2[i];
                    break;
                }
            }
            found
        };

        let t2_adj_apex2_sa = {
            let mut found = NONE;
            for i in 0..3 {
                let va = tri2[(i + 1) % 3];
                let vb = tri2[(i + 2) % 3];
                if (va == shared_a && vb == apex2) || (va == apex2 && vb == shared_a) {
                    found = old_adj2[i];
                    break;
                }
            }
            found
        };

        self.adjacency[t1] = [t2_adj_sb_apex2, t1_adj_sb_apex1, t2];
        self.adjacency[t2] = [t1_adj_apex1_sa, t2_adj_apex2_sa, t1];

        if t2_adj_sb_apex2 != NONE {
            self.update_adjacency_ref(t2_adj_sb_apex2, t2, t1);
        }
        if t1_adj_apex1_sa != NONE {
            self.update_adjacency_ref(t1_adj_apex1_sa, t1, t2);
        }
    }

    fn update_adjacency_ref(&mut self, tri_idx: usize, old_ref: usize, new_ref: usize) {
        for i in 0..3 {
            if self.adjacency[tri_idx][i] == old_ref {
                self.adjacency[tri_idx][i] = new_ref;
                return;
            }
        }
    }

    fn find_local_edge(&self, t: usize, a: usize, b: usize) -> usize {
        let tri = self.triangles[t];
        for i in 0..3 {
            let va = tri[(i + 1) % 3];
            let vb = tri[(i + 2) % 3];
            if (va == a && vb == b) || (va == b && vb == a) {
                return i;
            }
        }
        NONE
    }
}

/// Test whether segments (p1,p2) and (p3,p4) intersect in their interiors (shared endpoints excluded).
fn segments_intersect(p1: [f64; 2], p2: [f64; 2], p3: [f64; 2], p4: [f64; 2]) -> bool {
    let d1 = orient2d(p3, p4, p1);
    let d2 = orient2d(p3, p4, p2);
    let d3 = orient2d(p1, p2, p3);
    let d4 = orient2d(p1, p2, p4);

    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orient3d;

    fn triangle_area(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
        ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs() * 0.5
    }

    fn total_triangle_area(cdt: &Cdt) -> f64 {
        let tris = cdt.triangles();
        let verts = cdt.user_vertices();
        tris.iter()
            .map(|t| triangle_area(verts[t[0]], verts[t[1]], verts[t[2]]))
            .sum()
    }

    fn polygon_area(points: &[[f64; 2]]) -> f64 {
        let mut sum = 0.0;
        for i in 0..points.len() {
            let j = (i + 1) % points.len();
            sum += points[i][0] * points[j][1] - points[j][0] * points[i][1];
        }
        sum.abs() * 0.5
    }

    fn collect_edges(tris: &[[usize; 3]]) -> HashSet<(usize, usize)> {
        let mut edges = HashSet::new();
        for t in tris {
            for i in 0..3 {
                edges.insert(edge_key(t[i], t[(i + 1) % 3]));
            }
        }
        edges
    }

    #[test]
    fn insert_single_point() {
        let mut cdt = Cdt::new((0.0, 0.0, 1.0, 1.0));
        cdt.insert(0.5, 0.5);
        let tris = cdt.triangles();
        assert!(tris.is_empty());
    }

    #[test]
    fn insert_triangle() {
        let mut cdt = Cdt::new((0.0, 0.0, 1.0, 1.0));
        cdt.insert(0.0, 0.0);
        cdt.insert(1.0, 0.0);
        cdt.insert(0.5, 1.0);
        let tris = cdt.triangles();
        assert_eq!(tris.len(), 1);
    }

    #[test]
    fn square_triangulation() {
        let mut cdt = Cdt::new((0.0, 0.0, 1.0, 1.0));
        cdt.insert(0.0, 0.0);
        cdt.insert(1.0, 0.0);
        cdt.insert(1.0, 1.0);
        cdt.insert(0.0, 1.0);
        let tris = cdt.triangles();
        assert_eq!(tris.len(), 2);

        let verts = cdt.user_vertices();
        let total_area: f64 = tris
            .iter()
            .map(|t| {
                let a = verts[t[0]];
                let b = verts[t[1]];
                let c = verts[t[2]];
                ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs() * 0.5
            })
            .sum();
        assert!((total_area - 1.0).abs() < 1e-10);
    }

    #[test]
    fn constraint_edges_preserved() {
        let pts = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let mut cdt = Cdt::new((-0.5, -0.5, 1.5, 1.5));
        cdt.add_constraint_edges(&pts, true).unwrap();

        let tris = cdt.triangles();
        let mut edges: HashSet<(usize, usize)> = HashSet::new();
        for t in &tris {
            for i in 0..3 {
                edges.insert(edge_key(t[i], t[(i + 1) % 3]));
            }
        }

        let n = pts.len();
        for i in 0..n {
            let j = (i + 1) % n;
            assert!(
                edges.contains(&edge_key(i, j)),
                "constraint edge ({}, {}) not found in triangulation",
                i,
                j
            );
        }
    }

    #[test]
    fn delaunay_property() {
        let points = [
            [0.1, 0.2],
            [0.8, 0.1],
            [0.9, 0.9],
            [0.2, 0.8],
            [0.5, 0.5],
            [0.3, 0.4],
            [0.7, 0.6],
        ];
        let mut cdt = Cdt::new((-0.5, -0.5, 1.5, 1.5));
        for p in &points {
            cdt.insert(p[0], p[1]);
        }

        let tris = cdt.triangles();
        let verts = cdt.user_vertices();

        for tri in &tris {
            let a = verts[tri[0]];
            let b = verts[tri[1]];
            let c = verts[tri[2]];
            let (a, b, c) = if orient2d(a, b, c) >= 0.0 {
                (a, b, c)
            } else {
                (a, c, b)
            };

            for (vi, v) in verts.iter().enumerate() {
                if vi == tri[0] || vi == tri[1] || vi == tri[2] {
                    continue;
                }
                let ic = incircle(a, b, c, *v);
                assert!(
                    ic <= 1e-10,
                    "Delaunay violation: triangle {:?}, vertex {} (incircle={})",
                    tri,
                    vi,
                    ic
                );
            }
        }
    }

    #[test]
    fn closed_constraint_polygon_is_order_invariant() {
        let points = [[0.0, 0.0], [2.0, 0.0], [2.5, 1.0], [1.0, 2.0], [-0.5, 1.0]];

        let mut forward = Cdt::new((-1.0, -1.0, 3.0, 3.0));
        forward.add_constraint_edges(&points, true).unwrap();

        let mut reversed_points = points;
        reversed_points.reverse();
        let mut reversed = Cdt::new((-1.0, -1.0, 3.0, 3.0));
        reversed
            .add_constraint_edges(&reversed_points, true)
            .unwrap();

        let expected_area = polygon_area(&points);
        let forward_area = total_triangle_area(&forward);
        let reversed_area = total_triangle_area(&reversed);
        let forward_edges = collect_edges(&forward.triangles());
        let reversed_edges = collect_edges(&reversed.triangles());

        assert_eq!(forward.n_user_vertices(), points.len());
        assert_eq!(reversed.n_user_vertices(), points.len());
        assert!(!forward.triangles().is_empty());
        assert!(!reversed.triangles().is_empty());
        assert_eq!(forward.triangles().len(), reversed.triangles().len());
        for i in 0..points.len() {
            let j = (i + 1) % points.len();
            let edge = edge_key(i, j);
            assert!(forward_edges.contains(&edge));
            assert!(reversed_edges.contains(&edge));
        }
        assert!((forward_area - expected_area).abs() < 1e-10);
        assert!((reversed_area - expected_area).abs() < 1e-10);
    }

    #[test]
    fn nearly_degenerate_constraint_chain_remains_valid() {
        let points = [
            [0.0, 0.0],
            [1.0e-12, 1.0e-12],
            [2.0, 0.0],
            [2.0, 1.0],
            [1.0, 1.0 + 1.0e-12],
            [0.0, 1.0],
        ];

        let mut cdt = Cdt::new((-0.5, -0.5, 2.5, 1.5));
        cdt.add_constraint_edges(&points, true).unwrap();

        let tris = cdt.triangles();
        let total_area = total_triangle_area(&cdt);

        assert_eq!(cdt.n_user_vertices(), points.len());
        assert!(!tris.is_empty());
        assert!((total_area - polygon_area(&points)).abs() < 1e-10);

        let edges = collect_edges(&tris);
        for i in 0..points.len() {
            let j = (i + 1) % points.len();
            assert!(
                edges.contains(&edge_key(i, j)),
                "constraint edge ({}, {}) not found in triangulation",
                i,
                j
            );
        }
    }

    #[test]
    fn orient_sign_contract_is_fixed() {
        assert!(orient2d([0.0, 0.0], [1.0, 0.0], [0.0, 1.0]) > 0.0);
        assert!(orient2d([0.0, 0.0], [0.0, 1.0], [1.0, 0.0]) < 0.0);
        assert_eq!(orient2d([0.0, 0.0], [1.0, 0.0], [2.0, 0.0]), 0.0);

        assert!(
            orient3d(
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, -1.0]
            ) > 0.0
        );
        assert!(
            orient3d(
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0]
            ) < 0.0
        );
        assert_eq!(
            orient3d(
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.5, 0.5, 0.0]
            ),
            0.0
        );
    }

    #[test]
    fn constraint_chain_splits_at_existing_collinear_vertices() {
        let mut cdt = Cdt::new((-0.5, -0.5, 2.5, 1.5));
        let a = cdt.insert(0.0, 0.0);
        let b = cdt.insert(2.0, 0.0);
        let mid = cdt.insert(1.0, 0.0);
        cdt.insert(0.5, 1.0);
        cdt.insert(1.5, 1.0);

        let chain = cdt.constraint_chain(a, b);
        assert_eq!(chain, vec![a, mid, b]);

        for edge in chain.windows(2) {
            cdt.enforce_constraint(edge[0], edge[1]).unwrap();
        }

        let edges = collect_edges(&cdt.triangles());
        assert!(edges.contains(&edge_key(0, 2)));
        assert!(edges.contains(&edge_key(1, 2)));
    }
}
