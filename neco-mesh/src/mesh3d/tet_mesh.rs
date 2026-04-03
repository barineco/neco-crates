//! Tetrahedral mesh with explicit adjacency information and search utilities.

use std::collections::{HashMap, HashSet};

use crate::internal_mesh3d::Mesh3D;
use crate::point3::Point3;

use crate::predicates::{orient3d, p3};

/// Tetrahedral mesh with adjacency links for each face.
#[derive(Debug, Clone)]
pub struct TetMesh {
    pub nodes: Vec<Point3>,
    pub tets: Vec<[usize; 4]>,
    pub neighbors: Vec<[Option<usize>; 4]>,
}

fn face_key(tet: &[usize; 4], fi: usize) -> (usize, usize, usize) {
    let mut v = [0usize; 3];
    let mut idx = 0;
    for (i, &node) in tet.iter().enumerate() {
        if i != fi {
            v[idx] = node;
            idx += 1;
        }
    }
    v.sort();
    (v[0], v[1], v[2])
}

impl TetMesh {
    pub const TOMBSTONE: [usize; 4] = [usize::MAX; 4];

    pub fn is_tombstone(&self, tet_idx: usize) -> bool {
        self.tets[tet_idx] == Self::TOMBSTONE
    }

    pub fn compact_tombstones(&mut self) {
        let tombstones_exist = self.tets.contains(&Self::TOMBSTONE);
        if !tombstones_exist {
            return;
        }

        let old_len = self.tets.len();
        let mut old_to_new = vec![0usize; old_len];
        let mut new_idx = 0;
        for (i, slot) in old_to_new.iter_mut().enumerate().take(old_len) {
            if self.tets[i] != Self::TOMBSTONE {
                *slot = new_idx;
                new_idx += 1;
            }
        }
        let new_len = new_idx;

        let mut new_tets = Vec::with_capacity(new_len);
        let mut new_neighbors = Vec::with_capacity(new_len);
        for i in 0..old_len {
            if self.tets[i] != Self::TOMBSTONE {
                new_tets.push(self.tets[i]);
                let mut nb = self.neighbors[i];
                for idx in nb.iter_mut().flatten() {
                    *idx = old_to_new[*idx];
                }
                new_neighbors.push(nb);
            }
        }

        self.tets = new_tets;
        self.neighbors = new_neighbors;
    }

    pub fn build_adjacency(tets: &[[usize; 4]]) -> Vec<[Option<usize>; 4]> {
        let mut face_to_tet: HashMap<(usize, usize, usize), Vec<(usize, usize)>> = HashMap::new();
        for (ti, tet) in tets.iter().enumerate() {
            for fi in 0..4 {
                let key = face_key(tet, fi);
                face_to_tet.entry(key).or_default().push((ti, fi));
            }
        }
        let mut neighbors = vec![[None; 4]; tets.len()];
        for entries in face_to_tet.values() {
            if entries.len() == 2 {
                let (t0, f0) = entries[0];
                let (t1, f1) = entries[1];
                neighbors[t0][f0] = Some(t1);
                neighbors[t1][f1] = Some(t0);
            }
        }
        neighbors
    }

    pub fn from_mesh3d(mesh: &Mesh3D) -> Self {
        let mut tets = mesh.tetrahedra.clone();
        let nodes = mesh.nodes.clone();

        for tet in &mut tets {
            let a = &nodes[tet[0]];
            let b = &nodes[tet[1]];
            let c = &nodes[tet[2]];
            let d = &nodes[tet[3]];
            let ab = b.sub(a);
            let ac = c.sub(a);
            let ad = d.sub(a);
            let vol = Point3::dot(&ab, &Point3::cross(&ac, &ad));
            if vol < 0.0 {
                tet.swap(0, 1);
            }
        }

        let neighbors = Self::build_adjacency(&tets);
        Self {
            nodes,
            tets,
            neighbors,
        }
    }

    pub fn to_mesh3d(&self) -> Mesh3D {
        Mesh3D {
            nodes: self.nodes.clone(),
            tetrahedra: self.tets.clone(),
        }
    }

    pub fn star(&self, v: usize) -> Vec<usize> {
        self.tets
            .iter()
            .enumerate()
            .filter_map(|(ti, tet)| if tet.contains(&v) { Some(ti) } else { None })
            .collect()
    }

    pub fn edge_ring(&self, a: usize, b: usize) -> Vec<usize> {
        let start = self.tets.iter().enumerate().find_map(|(ti, tet)| {
            if tet.contains(&a) && tet.contains(&b) {
                Some(ti)
            } else {
                None
            }
        });
        let start = match start {
            Some(s) => s,
            None => return Vec::new(),
        };

        let mut ring = vec![start];
        let mut current = start;
        let mut visited = std::collections::HashSet::new();
        visited.insert(start);

        loop {
            let tet = &self.tets[current];
            let mut advanced = false;
            for (li, &node) in tet.iter().enumerate() {
                if node != a && node != b {
                    if let Some(next) = self.neighbors[current][li] {
                        if !visited.contains(&next) {
                            let next_tet = &self.tets[next];
                            if next_tet.contains(&a) && next_tet.contains(&b) {
                                visited.insert(next);
                                ring.push(next);
                                current = next;
                                advanced = true;
                                break;
                            }
                        } else if next == start && ring.len() > 2 {
                            return ring;
                        }
                    }
                }
            }
            if !advanced {
                break;
            }
        }

        current = start;
        let mut prefix = Vec::new();
        loop {
            let tet = &self.tets[current];
            let mut advanced = false;
            for (li, &node) in tet.iter().enumerate() {
                if node != a && node != b {
                    if let Some(next) = self.neighbors[current][li] {
                        if !visited.contains(&next) {
                            let next_tet = &self.tets[next];
                            if next_tet.contains(&a) && next_tet.contains(&b) {
                                visited.insert(next);
                                prefix.push(next);
                                current = next;
                                advanced = true;
                                break;
                            }
                        }
                    }
                }
            }
            if !advanced {
                break;
            }
        }

        if !prefix.is_empty() {
            prefix.reverse();
            prefix.extend(ring);
            return prefix;
        }

        ring
    }

    pub fn tet_signed_volume(&self, tet_idx: usize) -> f64 {
        let tet = &self.tets[tet_idx];
        let a = &self.nodes[tet[0]];
        let b = &self.nodes[tet[1]];
        let c = &self.nodes[tet[2]];
        let d = &self.nodes[tet[3]];
        let ab = b.sub(a);
        let ac = c.sub(a);
        let ad = d.sub(a);
        Point3::dot(&ab, &Point3::cross(&ac, &ad)) / 6.0
    }

    pub fn tet_volume(&self, tet_idx: usize) -> f64 {
        self.tet_signed_volume(tet_idx).abs()
    }

    pub fn locate_point(&self, p: &Point3) -> Option<usize> {
        if self.tets.is_empty() {
            return None;
        }
        let start = self.tets.len() / 2;
        self.locate_point_from(p, start)
    }

    pub fn locate_point_from(&self, p: &Point3, start_tet: usize) -> Option<usize> {
        if self.tets.is_empty() || start_tet >= self.tets.len() {
            return None;
        }

        let pp = p3(p);
        let mut current = start_tet;
        let mut visited = HashSet::new();

        loop {
            if !visited.insert(current) {
                return self.locate_point_linear(p);
            }

            let tet = &self.tets[current];
            if *tet == Self::TOMBSTONE {
                return self.locate_point_linear(p);
            }
            let v = [
                p3(&self.nodes[tet[0]]),
                p3(&self.nodes[tet[1]]),
                p3(&self.nodes[tet[2]]),
                p3(&self.nodes[tet[3]]),
            ];

            //
            let ref_orient = orient3d(v[0], v[1], v[2], v[3]);

            let mut found_outside = false;
            for fi in 0..4 {
                let sub_orient = match fi {
                    0 => orient3d(pp, v[1], v[2], v[3]),
                    1 => orient3d(v[0], pp, v[2], v[3]),
                    2 => orient3d(v[0], v[1], pp, v[3]),
                    3 => orient3d(v[0], v[1], v[2], pp),
                    _ => unreachable!(),
                };

                if sub_orient * ref_orient < 0.0 {
                    match self.neighbors[current][fi] {
                        Some(next) => {
                            current = next;
                            found_outside = true;
                            break;
                        }
                        None => {
                            return None;
                        }
                    }
                }
            }

            if !found_outside {
                return Some(current);
            }
        }
    }

    fn locate_point_linear(&self, p: &Point3) -> Option<usize> {
        let pp = p3(p);
        for (ti, tet) in self.tets.iter().enumerate() {
            if *tet == Self::TOMBSTONE {
                continue;
            }
            let a = p3(&self.nodes[tet[0]]);
            let b = p3(&self.nodes[tet[1]]);
            let c = p3(&self.nodes[tet[2]]);
            let d = p3(&self.nodes[tet[3]]);

            let vol = orient3d(a, b, c, d);
            if vol.abs() < 1e-30 {
                continue;
            }

            let sign = vol.signum();
            let o0 = orient3d(pp, b, c, d) * sign;
            let o1 = orient3d(a, pp, c, d) * sign;
            let o2 = orient3d(a, b, pp, d) * sign;
            let o3 = orient3d(a, b, c, pp) * sign;

            if o0 >= 0.0 && o1 >= 0.0 && o2 >= 0.0 && o3 >= 0.0 {
                return Some(ti);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::internal_mesh3d::generate_box_mesh;

    fn single_tet_mesh() -> Mesh3D {
        Mesh3D {
            nodes: vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
                Point3::new(0.0, 0.0, 1.0),
            ],
            tetrahedra: vec![[0, 1, 2, 3]],
        }
    }

    fn negative_orientation_mesh() -> Mesh3D {
        Mesh3D {
            nodes: vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
                Point3::new(0.0, 0.0, 1.0),
            ],
            tetrahedra: vec![[1, 0, 2, 3]],
        }
    }

    #[test]
    fn from_mesh3d_to_mesh3d_round_trip() {
        let mesh = generate_box_mesh(1.0, 1.0, 1.0, 0.5);
        let tet_mesh = TetMesh::from_mesh3d(&mesh);
        let mesh2 = tet_mesh.to_mesh3d();

        assert_eq!(mesh.nodes.len(), mesh2.nodes.len());
        assert_eq!(mesh.tetrahedra.len(), mesh2.tetrahedra.len());

        for (a, b) in mesh.nodes.iter().zip(mesh2.nodes.iter()) {
            assert!((a.x - b.x).abs() < 1e-15);
            assert!((a.y - b.y).abs() < 1e-15);
            assert!((a.z - b.z).abs() < 1e-15);
        }

        let vol1: f64 = mesh
            .tetrahedra
            .iter()
            .map(|tet| {
                let a = &mesh.nodes[tet[0]];
                let b = &mesh.nodes[tet[1]];
                let c = &mesh.nodes[tet[2]];
                let d = &mesh.nodes[tet[3]];
                let ab = b.sub(a);
                let ac = c.sub(a);
                let ad = d.sub(a);
                (Point3::dot(&ab, &Point3::cross(&ac, &ad)) / 6.0).abs()
            })
            .sum();
        let vol2: f64 = (0..tet_mesh.tets.len())
            .map(|i| tet_mesh.tet_volume(i))
            .sum();
        assert!(
            (vol1 - vol2).abs() < 1e-12,
            "volume mismatch: {} vs {}",
            vol1,
            vol2
        );
    }

    #[test]
    fn adjacency_is_symmetric() {
        let mesh = generate_box_mesh(1.0, 1.0, 1.0, 0.5);
        let tet_mesh = TetMesh::from_mesh3d(&mesh);

        for (ti, nbrs) in tet_mesh.neighbors.iter().enumerate() {
            for (fi, &nbr) in nbrs.iter().enumerate() {
                if let Some(adj) = nbr {
                    let reverse = tet_mesh.neighbors[adj].contains(&Some(ti));
                    assert!(
                        reverse,
                        "adjacency symmetry violated: neighbors[{}][{}] = {}, but neighbors[{}] does not contain {}",
                        ti, fi, adj, adj, ti
                    );
                }
            }
        }
    }

    #[test]
    fn face_index_matches_tetgen_convention() {
        let tet = [0usize, 1, 2, 3];
        assert_eq!(face_key(&tet, 0), (1, 2, 3));
        assert_eq!(face_key(&tet, 1), (0, 2, 3));
        assert_eq!(face_key(&tet, 2), (0, 1, 3));
        assert_eq!(face_key(&tet, 3), (0, 1, 2));
    }

    #[test]
    fn adjacent_tets_share_the_same_face_key() {
        let mesh = generate_box_mesh(1.0, 1.0, 1.0, 0.5);
        let tet_mesh = TetMesh::from_mesh3d(&mesh);

        for (ti, nbrs) in tet_mesh.neighbors.iter().enumerate() {
            for (fi, &nbr) in nbrs.iter().enumerate() {
                if let Some(adj) = nbr {
                    let key_ti = face_key(&tet_mesh.tets[ti], fi);
                    let found = (0..4).any(|fj| face_key(&tet_mesh.tets[adj], fj) == key_ti);
                    assert!(
                        found,
                        "tet {} face {} (key {:?}) is not present in adjacent tet {}",
                        ti, fi, key_ti, adj
                    );
                }
            }
        }
    }

    #[test]
    fn works_with_generate_box_mesh() {
        let mesh = generate_box_mesh(2.0, 1.5, 1.0, 0.5);
        let tet_mesh = TetMesh::from_mesh3d(&mesh);

        for i in 0..tet_mesh.tets.len() {
            let sv = tet_mesh.tet_signed_volume(i);
            assert!(sv > 0.0, "tet {} has non-positive signed volume: {}", i, sv);
        }

        let total: f64 = (0..tet_mesh.tets.len())
            .map(|i| tet_mesh.tet_volume(i))
            .sum();
        assert!(
            (total - 3.0).abs() < 1e-10,
            "total volume mismatch: {} (expected 3.0)",
            total
        );
    }

    #[test]
    fn star_returns_incident_tets() {
        let mesh = generate_box_mesh(1.0, 1.0, 1.0, 1.0);
        let tet_mesh = TetMesh::from_mesh3d(&mesh);

        for v in 0..tet_mesh.nodes.len() {
            let s = tet_mesh.star(v);
            for &ti in &s {
                assert!(
                    tet_mesh.tets[ti].contains(&v),
                    "tet {} in star({}) does not contain vertex {}",
                    v,
                    ti,
                    v
                );
            }
        }

        let corner_star = tet_mesh.star(0);
        assert!(!corner_star.is_empty(), "star(0) should not be empty");
    }

    #[test]
    fn edge_ring_is_open_for_boundary_edges() {
        let mesh = generate_box_mesh(1.0, 1.0, 1.0, 1.0);
        let tet_mesh = TetMesh::from_mesh3d(&mesh);

        // idx(i,j,k) = k*2*2 + j*2 + i
        let v0 = 0; // (0,0,0)
        let v1 = 1; // (1,0,0)

        let ring = tet_mesh.edge_ring(v0, v1);
        assert!(!ring.is_empty(), "edge_ring({}, {}) is empty", v0, v1);

        if ring.len() > 1 {
            let first = ring[0];
            let last = ring[ring.len() - 1];
            for &ti in &ring {
                let tet = &tet_mesh.tets[ti];
                assert!(
                    tet.contains(&v0) && tet.contains(&v1),
                    "tet {} in edge_ring does not contain edge ({}, {})",
                    ti,
                    v0,
                    v1
                );
            }

            let is_cyclic = tet_mesh.neighbors[last].contains(&Some(first));
            assert!(
                !is_cyclic || ring.len() <= 2,
                "boundary edge ({}, {}) unexpectedly formed a cycle (len={})",
                v0,
                v1,
                ring.len()
            );
        }
    }

    #[test]
    fn edge_ring_is_cyclic_for_interior_edges() {
        let mesh = generate_box_mesh(1.0, 1.0, 1.0, 0.5);
        let tet_mesh = TetMesh::from_mesh3d(&mesh);

        // nx=ny=nz=3, idx(i,j,k) = k*9 + j*3 + i
        let _center = 13; // (0.5, 0.5, 0.5)

        // (1.0, 0.5, 0.5) = idx(2,1,1) = 1*9 + 1*3 + 2 = 14

        let mut found_cyclic = false;
        'outer: for ti in 0..tet_mesh.tets.len() {
            let tet = &tet_mesh.tets[ti];
            for i in 0..4 {
                for j in (i + 1)..4 {
                    let a = tet[i];
                    let b = tet[j];
                    let ring = tet_mesh.edge_ring(a, b);
                    if ring.len() >= 3 {
                        let last = ring[ring.len() - 1];
                        let first = ring[0];
                        let cyclic = tet_mesh.neighbors[last].contains(&Some(first))
                            && tet_mesh.tets[first].contains(&a)
                            && tet_mesh.tets[first].contains(&b);
                        if cyclic {
                            found_cyclic = true;
                            for &t in &ring {
                                assert!(
                                    tet_mesh.tets[t].contains(&a) && tet_mesh.tets[t].contains(&b)
                                );
                            }
                            break 'outer;
                        }
                    }
                }
            }
        }
        assert!(found_cyclic, "failed to find a cyclic edge_ring");
    }

    #[test]
    fn fixes_negative_volume_tets_automatically() {
        let mesh = negative_orientation_mesh();

        let a = &mesh.nodes[mesh.tetrahedra[0][0]];
        let b = &mesh.nodes[mesh.tetrahedra[0][1]];
        let c = &mesh.nodes[mesh.tetrahedra[0][2]];
        let d = &mesh.nodes[mesh.tetrahedra[0][3]];
        let ab = b.sub(a);
        let ac = c.sub(a);
        let ad = d.sub(a);
        let vol_before = Point3::dot(&ab, &Point3::cross(&ac, &ad)) / 6.0;
        assert!(
            vol_before < 0.0,
            "test precondition failed: source mesh should have negative orientation"
        );

        let tet_mesh = TetMesh::from_mesh3d(&mesh);
        let sv = tet_mesh.tet_signed_volume(0);
        assert!(
            sv > 0.0,
            "volume is still negative after from_mesh3d: {}",
            sv
        );

        assert!(
            (sv - vol_before.abs()).abs() < 1e-15,
            "absolute volume changed: {} vs {}",
            sv,
            vol_before.abs()
        );
    }

    #[test]
    fn locate_point_for_random_points_inside_mesh() {
        let mesh = generate_box_mesh(2.0, 1.5, 1.0, 0.5);
        let tet_mesh = TetMesh::from_mesh3d(&mesh);

        let mut rng_state: u64 = 12345;
        let mut next_f64 = |lo: f64, hi: f64| -> f64 {
            rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let t = ((rng_state >> 33) as f64) / (u32::MAX as f64);
            lo + t * (hi - lo)
        };

        for i in 0..100 {
            let x = next_f64(-0.99, 0.99);
            let y = next_f64(-0.74, 0.74);
            let z = next_f64(-0.49, 0.49);
            let p = Point3::new(x, y, z);

            let result = tet_mesh.locate_point(&p);
            assert!(
                result.is_some(),
                "point {} ({}, {}, {}) was not found inside the mesh",
                i,
                x,
                y,
                z
            );

            let ti = result.unwrap();
            let tet = &tet_mesh.tets[ti];
            let a = crate::predicates::p3(&tet_mesh.nodes[tet[0]]);
            let b = crate::predicates::p3(&tet_mesh.nodes[tet[1]]);
            let c = crate::predicates::p3(&tet_mesh.nodes[tet[2]]);
            let d = crate::predicates::p3(&tet_mesh.nodes[tet[3]]);
            let pp = crate::predicates::p3(&p);
            let vol = crate::predicates::orient3d(a, b, c, d);
            let sign = vol.signum();
            let o0 = crate::predicates::orient3d(pp, b, c, d) * sign;
            let o1 = crate::predicates::orient3d(a, pp, c, d) * sign;
            let o2 = crate::predicates::orient3d(a, b, pp, d) * sign;
            let o3 = crate::predicates::orient3d(a, b, c, pp) * sign;
            assert!(
                o0 >= 0.0 && o1 >= 0.0 && o2 >= 0.0 && o3 >= 0.0,
                "tet {} returned by locate_point does not contain point ({}, {}, {})",
                ti,
                x,
                y,
                z
            );
        }
    }

    #[test]
    fn locate_point_for_points_outside_mesh() {
        let mesh = generate_box_mesh(1.0, 1.0, 1.0, 0.5);
        let tet_mesh = TetMesh::from_mesh3d(&mesh);

        let outside_points = [
            Point3::new(-1.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, -1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.0, 0.0, -1.0),
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(10.0, 10.0, 10.0),
        ];

        for p in &outside_points {
            let result = tet_mesh.locate_point(p);
            assert!(
                result.is_none(),
                "point ({}, {}, {}) outside the mesh returned Some({:?})",
                p.x,
                p.y,
                p.z,
                result
            );
        }
    }

    #[test]
    fn locate_point_for_mesh_vertices() {
        let mesh = generate_box_mesh(1.0, 1.0, 1.0, 0.5);
        let tet_mesh = TetMesh::from_mesh3d(&mesh);

        for (ni, node) in tet_mesh.nodes.iter().enumerate() {
            let result = tet_mesh.locate_point(node);
            assert!(
                result.is_some(),
                "failed to find a tet containing vertex {} ({}, {}, {})",
                ni,
                node.x,
                node.y,
                node.z
            );

            let ti = result.unwrap();
            let tet = &tet_mesh.tets[ti];
            let a = crate::predicates::p3(&tet_mesh.nodes[tet[0]]);
            let b = crate::predicates::p3(&tet_mesh.nodes[tet[1]]);
            let c = crate::predicates::p3(&tet_mesh.nodes[tet[2]]);
            let d = crate::predicates::p3(&tet_mesh.nodes[tet[3]]);
            let pp = crate::predicates::p3(node);
            let vol = crate::predicates::orient3d(a, b, c, d);
            let sign = vol.signum();
            let o0 = crate::predicates::orient3d(pp, b, c, d) * sign;
            let o1 = crate::predicates::orient3d(a, pp, c, d) * sign;
            let o2 = crate::predicates::orient3d(a, b, pp, d) * sign;
            let o3 = crate::predicates::orient3d(a, b, c, pp) * sign;
            assert!(
                o0 >= 0.0 && o1 >= 0.0 && o2 >= 0.0 && o3 >= 0.0,
                "tet {} returned by locate_point does not contain vertex {}",
                ti,
                ni
            );
        }
    }

    #[test]
    fn locate_point_from_with_explicit_start_tet() {
        let mesh = generate_box_mesh(1.0, 1.0, 1.0, 0.5);
        let tet_mesh = TetMesh::from_mesh3d(&mesh);
        let p = Point3::new(0.25, 0.25, 0.25);

        let results: Vec<Option<usize>> = (0..tet_mesh.tets.len().min(10))
            .map(|start| tet_mesh.locate_point_from(&p, start))
            .collect();

        for (start, result) in results.iter().enumerate() {
            assert!(
                result.is_some(),
                "failed to locate the point when starting from tet {}",
                start
            );
        }

        for &result in &results {
            let ti = result.unwrap();
            let tet = &tet_mesh.tets[ti];
            let a = crate::predicates::p3(&tet_mesh.nodes[tet[0]]);
            let b = crate::predicates::p3(&tet_mesh.nodes[tet[1]]);
            let c = crate::predicates::p3(&tet_mesh.nodes[tet[2]]);
            let d = crate::predicates::p3(&tet_mesh.nodes[tet[3]]);
            let pp = crate::predicates::p3(&p);
            let vol = crate::predicates::orient3d(a, b, c, d);
            let sign = vol.signum();
            let o0 = crate::predicates::orient3d(pp, b, c, d) * sign;
            let o1 = crate::predicates::orient3d(a, pp, c, d) * sign;
            let o2 = crate::predicates::orient3d(a, b, pp, d) * sign;
            let o3 = crate::predicates::orient3d(a, b, c, pp) * sign;
            assert!(
                o0 >= 0.0 && o1 >= 0.0 && o2 >= 0.0 && o3 >= 0.0,
                "tet {} returned by locate_point_from(start={}) does not contain the point",
                ti,
                ti
            );
        }
    }

    #[test]
    fn locate_point_on_empty_mesh() {
        let tet_mesh = TetMesh {
            nodes: Vec::new(),
            tets: Vec::new(),
            neighbors: Vec::new(),
        };
        assert_eq!(tet_mesh.locate_point(&Point3::new(0.0, 0.0, 0.0)), None);
    }

    #[test]
    fn locate_point_on_single_tet() {
        let tet_mesh = TetMesh::from_mesh3d(&single_tet_mesh());

        let inside = Point3::new(0.1, 0.1, 0.1);
        assert_eq!(tet_mesh.locate_point(&inside), Some(0));

        let outside = Point3::new(1.0, 1.0, 1.0);
        assert_eq!(tet_mesh.locate_point(&outside), None);
    }

    #[test]
    fn tet_volume_method() {
        let tet_mesh = TetMesh::from_mesh3d(&single_tet_mesh());
        let sv = tet_mesh.tet_signed_volume(0);
        let v = tet_mesh.tet_volume(0);

        assert!(
            (v - 1.0 / 6.0).abs() < 1e-15,
            "incorrect volume: {} (expected {})",
            v,
            1.0 / 6.0
        );
        assert!(
            (sv.abs() - v).abs() < 1e-15,
            "absolute signed volume does not match volume()"
        );
    }
}
