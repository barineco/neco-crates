//! Mesh invariant validation.

use crate::tessellate::TriMesh;
use crate::vec3;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct MeshValidation {
    /// Every edge is shared by exactly 2 triangles.
    pub is_watertight: bool,
    /// Adjacent triangles traverse shared edges in opposite directions.
    pub is_consistently_oriented: bool,
    pub has_no_degenerate_faces: bool,
    pub signed_volume: f64,
    /// Euler characteristic (V - E + F).
    pub euler_number: i64,
    pub n_connected_components: usize,
}

impl TriMesh {
    /// Validate mesh invariants.
    pub fn validate(&self) -> MeshValidation {
        let half_edges = build_half_edge_map(&self.triangles);
        let edge_counts = build_edge_counts(&self.triangles);

        let is_watertight = check_watertight(&edge_counts);
        let is_consistently_oriented = check_consistent_orientation(&half_edges);
        let has_no_degenerate_faces = check_no_degenerate(&self.vertices, &self.triangles);
        let signed_volume = compute_signed_volume(&self.vertices, &self.triangles);
        let euler_number =
            compute_euler_number(self.vertices.len(), edge_counts.len(), self.triangles.len());
        let n_connected_components =
            compute_connected_components(self.vertices.len(), &self.triangles);

        MeshValidation {
            is_watertight,
            is_consistently_oriented,
            has_no_degenerate_faces,
            signed_volume,
            euler_number,
            n_connected_components,
        }
    }
}

/// Half-edge map: (a, b) -> occurrence count.
fn build_half_edge_map(triangles: &[[usize; 3]]) -> HashMap<(usize, usize), usize> {
    let mut map = HashMap::new();
    for tri in triangles {
        for i in 0..3 {
            let a = tri[i];
            let b = tri[(i + 1) % 3];
            *map.entry((a, b)).or_insert(0) += 1;
        }
    }
    map
}

/// Edge count: (min, max) -> occurrence count.
fn build_edge_counts(triangles: &[[usize; 3]]) -> HashMap<(usize, usize), usize> {
    let mut map = HashMap::new();
    for tri in triangles {
        for i in 0..3 {
            let a = tri[i];
            let b = tri[(i + 1) % 3];
            let key = if a < b { (a, b) } else { (b, a) };
            *map.entry(key).or_insert(0) += 1;
        }
    }
    map
}

/// Watertight: every edge shared by exactly 2 triangles.
fn check_watertight(edge_counts: &HashMap<(usize, usize), usize>) -> bool {
    edge_counts.values().all(|&count| count == 2)
}

/// Consistent orientation: each half-edge (a, b) has a reverse (b, a).
fn check_consistent_orientation(half_edges: &HashMap<(usize, usize), usize>) -> bool {
    for &(a, b) in half_edges.keys() {
        if !half_edges.contains_key(&(b, a)) {
            return false;
        }
    }
    // Each half-edge must appear exactly once.
    half_edges.values().all(|&count| count == 1)
}

/// Degenerate triangle check: area > 1e-20.
fn check_no_degenerate(vertices: &[[f64; 3]], triangles: &[[usize; 3]]) -> bool {
    for tri in triangles {
        let v0 = vertices[tri[0]];
        let v1 = vertices[tri[1]];
        let v2 = vertices[tri[2]];
        let area = vec3::length(vec3::cross(vec3::sub(v1, v0), vec3::sub(v2, v0))) * 0.5;
        if area <= 1e-20 {
            return false;
        }
    }
    true
}

/// Signed volume via divergence theorem.
fn compute_signed_volume(vertices: &[[f64; 3]], triangles: &[[usize; 3]]) -> f64 {
    let mut volume = 0.0;
    for tri in triangles {
        let v0 = vertices[tri[0]];
        let v1 = vertices[tri[1]];
        let v2 = vertices[tri[2]];
        volume += vec3::dot(v0, vec3::cross(v1, v2));
    }
    volume / 6.0
}

/// Euler number: V - E + F.
fn compute_euler_number(n_vertices: usize, n_edges: usize, n_faces: usize) -> i64 {
    n_vertices as i64 - n_edges as i64 + n_faces as i64
}

/// Connected components via union-find.
fn compute_connected_components(n_vertices: usize, triangles: &[[usize; 3]]) -> usize {
    if n_vertices == 0 {
        return 0;
    }

    let mut parent: Vec<usize> = (0..n_vertices).collect();
    let mut rank = vec![0u32; n_vertices];

    fn find(parent: &mut [usize], x: usize) -> usize {
        if parent[x] != x {
            parent[x] = find(parent, parent[x]);
        }
        parent[x]
    }

    fn union(parent: &mut [usize], rank: &mut [u32], a: usize, b: usize) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra == rb {
            return;
        }
        match rank[ra].cmp(&rank[rb]) {
            std::cmp::Ordering::Less => parent[ra] = rb,
            std::cmp::Ordering::Greater => parent[rb] = ra,
            std::cmp::Ordering::Equal => {
                parent[rb] = ra;
                rank[ra] += 1;
            }
        }
    }

    // Union vertices referenced by triangles.
    let mut used = vec![false; n_vertices];
    for tri in triangles {
        used[tri[0]] = true;
        used[tri[1]] = true;
        used[tri[2]] = true;
        union(&mut parent, &mut rank, tri[0], tri[1]);
        union(&mut parent, &mut rank, tri[0], tri[2]);
    }

    // Count distinct roots among used vertices.
    let mut roots = std::collections::HashSet::new();
    for (i, is_used) in used.iter().enumerate().take(n_vertices) {
        if *is_used {
            roots.insert(find(&mut parent, i));
        }
    }
    roots.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_triangle_not_watertight() {
        let mesh = TriMesh {
            vertices: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            triangles: vec![[0, 1, 2]],
        };
        let v = mesh.validate();
        assert!(!v.is_watertight);
        assert_eq!(v.n_connected_components, 1);
    }

    #[test]
    fn tetrahedron_watertight() {
        // Regular tetrahedron
        let mesh = TriMesh {
            vertices: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.5, 0.866, 0.0],
                [0.5, 0.289, 0.816],
            ],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            triangles: vec![
                [0, 2, 1], // bottom (outward)
                [0, 1, 3], // front
                [1, 2, 3], // right
                [2, 0, 3], // left
            ],
        };
        let v = mesh.validate();
        assert!(v.is_watertight, "tetrahedron not watertight");
        assert!(
            v.is_consistently_oriented,
            "tetrahedron orientation inconsistent"
        );
        assert!(
            v.has_no_degenerate_faces,
            "tetrahedron has degenerate faces"
        );
        assert_eq!(v.euler_number, 2);
        assert_eq!(v.n_connected_components, 1);
        assert!(v.signed_volume.abs() > 1e-10, "tetrahedron volume is zero");
    }

    #[test]
    fn empty_mesh() {
        let mesh = TriMesh::new();
        let v = mesh.validate();
        assert!(v.is_watertight); // vacuously true: no edges
        assert!(v.is_consistently_oriented);
        assert!(v.has_no_degenerate_faces);
        assert!((v.signed_volume).abs() < 1e-20);
        assert_eq!(v.n_connected_components, 0);
    }
}
