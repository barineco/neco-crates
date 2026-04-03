//! Bowyer-Watson 3D Incremental Insertion.

use std::collections::{HashSet, VecDeque};

use crate::point3::Point3;

use super::tet_mesh::TetMesh;
use crate::predicates::{insphere, orient3d, p3};

// ────────────────────────────────────────────────────────────────
// ────────────────────────────────────────────────────────────────

fn morton_code(x: f64, y: f64, z: f64) -> u64 {
    let ix = ((x.clamp(0.0, 1.0)) * 1023.0) as u64;
    let iy = ((y.clamp(0.0, 1.0)) * 1023.0) as u64;
    let iz = ((z.clamp(0.0, 1.0)) * 1023.0) as u64;

    fn spread(mut v: u64) -> u64 {
        v &= 0x3FF; // 10 bit
        v = (v | (v << 16)) & 0x030000FF;
        v = (v | (v << 8)) & 0x0300F00F;
        v = (v | (v << 4)) & 0x030C30C3;
        v = (v | (v << 2)) & 0x09249249;
        v
    }

    spread(ix) | (spread(iy) << 1) | (spread(iz) << 2)
}

pub fn brio_sort(points: &[Point3]) -> Vec<usize> {
    let n = points.len();
    if n < 100 {
        return (0..n).collect();
    }

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut min_z = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut max_z = f64::NEG_INFINITY;

    for p in points {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        min_z = min_z.min(p.z);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
        max_z = max_z.max(p.z);
    }

    let dx = (max_x - min_x).max(1e-30);
    let dy = (max_y - min_y).max(1e-30);
    let dz = (max_z - min_z).max(1e-30);

    let mut indices: Vec<usize> = (0..n).collect();
    let codes: Vec<u64> = points
        .iter()
        .map(|p| morton_code((p.x - min_x) / dx, (p.y - min_y) / dy, (p.z - min_z) / dz))
        .collect();

    // ...

    let mut seed: u64 = 0xDEAD_BEEF_CAFE_BABE;
    let lcg_next = |s: &mut u64| -> u64 {
        *s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *s
    };

    for i in (1..n).rev() {
        let j = (lcg_next(&mut seed) as usize) % (i + 1);
        indices.swap(i, j);
    }

    let round_size = (n / 8).max(32);
    let mut rounds: Vec<Vec<usize>> = Vec::new();
    let mut start = 0;
    while start < n {
        let end = (start + round_size).min(n);
        let mut round: Vec<usize> = indices[start..end].to_vec();
        round.sort_by(|&a, &b| codes[a].cmp(&codes[b]));
        rounds.push(round);
        start = end;
    }

    rounds.into_iter().flatten().collect()
}

// ────────────────────────────────────────────────────────────────
// ────────────────────────────────────────────────────────────────

fn face_verts(tet: &[usize; 4], fi: usize) -> [usize; 3] {
    match fi {
        0 => [tet[1], tet[2], tet[3]],
        1 => [tet[0], tet[2], tet[3]],
        2 => [tet[0], tet[1], tet[3]],
        3 => [tet[0], tet[1], tet[2]],
        _ => unreachable!(),
    }
}

pub fn build_delaunay_cavity(mesh: &TetMesh, start_tet: usize, point: &Point3) -> Vec<usize> {
    let pp = p3(point);
    let mut cavity = vec![start_tet];
    let mut visited = HashSet::new();
    visited.insert(start_tet);
    let mut queue = VecDeque::new();
    queue.push_back(start_tet);

    while let Some(ti) = queue.pop_front() {
        for fi in 0..4 {
            if let Some(ni) = mesh.neighbors[ti][fi] {
                if visited.contains(&ni) {
                    continue;
                }
                visited.insert(ni);

                let tet = &mesh.tets[ni];
                let a = p3(&mesh.nodes[tet[0]]);
                let b = p3(&mesh.nodes[tet[1]]);
                let c = p3(&mesh.nodes[tet[2]]);
                let d = p3(&mesh.nodes[tet[3]]);

                let orient = orient3d(a, b, c, d);
                let in_sphere = if orient > 0.0 {
                    insphere(a, b, c, d, pp)
                } else if orient < 0.0 {
                    insphere(b, a, c, d, pp)
                } else {
                    continue;
                };

                if in_sphere > 0.0 {
                    cavity.push(ni);
                    queue.push_back(ni);
                }
            }
        }
    }
    cavity
}

/// Extract oriented boundary faces of the current Delaunay cavity.
pub fn extract_cavity_boundary(
    mesh: &TetMesh,
    cavity: &[usize],
    point: &Point3,
) -> Vec<([usize; 3], Option<usize>)> {
    let cavity_set: HashSet<usize> = cavity.iter().copied().collect();
    let mut boundary = Vec::new();

    for &ti in cavity {
        let tet = mesh.tets[ti];
        for fi in 0..4 {
            let neighbor = mesh.neighbors[ti][fi];
            let is_boundary = match neighbor {
                Some(ni) => !cavity_set.contains(&ni),
                None => true,
            };
            if !is_boundary {
                continue;
            }

            let face = face_verts(&tet, fi);

            let a = &mesh.nodes[face[0]];
            let b = &mesh.nodes[face[1]];
            let c = &mesh.nodes[face[2]];
            let ab = b.sub(a);
            let ac = c.sub(a);
            let ad = point.sub(a);
            let sv = Point3::dot(&ab, &Point3::cross(&ac, &ad));

            if sv > 0.0 {
                boundary.push((face, neighbor));
            } else if sv < 0.0 {
                boundary.push(([face[1], face[0], face[2]], neighbor));
            }
        }
    }
    boundary
}

/// Insert a vertex into the tetrahedral mesh by cavity retriangulation.
pub fn insert_vertex(mesh: &mut TetMesh, point: Point3) -> Result<usize, String> {
    let containing = mesh
        .locate_point(&point)
        .ok_or_else(|| "locate_point: failed to find a containing tetrahedron".to_string())?;

    let cavity = build_delaunay_cavity(mesh, containing, &point);
    if cavity.is_empty() {
        return Err("build_delaunay_cavity: cavity is empty".to_string());
    }

    let boundary = extract_cavity_boundary(mesh, &cavity, &point);
    if boundary.len() < 4 {
        return Err(format!(
            "extract_cavity_boundary: too few boundary faces ({})",
            boundary.len()
        ));
    }

    let vi = mesh.nodes.len();
    mesh.nodes.push(point);

    let new_tets: Vec<[usize; 4]> = boundary
        .iter()
        .map(|(face, _)| [face[0], face[1], face[2], vi])
        .collect();

    for tet in &new_tets {
        let a = &mesh.nodes[tet[0]];
        let b = &mesh.nodes[tet[1]];
        let c = &mesh.nodes[tet[2]];
        let d = &mesh.nodes[tet[3]];
        let ab = b.sub(a);
        let ac = c.sub(a);
        let ad = d.sub(a);
        let sv = Point3::dot(&ab, &Point3::cross(&ac, &ad));
        if sv <= -1e-20 {
            mesh.nodes.pop();
            return Err(
                "insert_vertex: retriangulation produced a non-positive-volume tetrahedron"
                    .to_string(),
            );
        }
    }

    let mut new_tet_indices = Vec::with_capacity(new_tets.len());

    for (i, new_tet) in new_tets.iter().enumerate() {
        if i < cavity.len() {
            let slot = cavity[i];
            mesh.tets[slot] = *new_tet;
            mesh.neighbors[slot] = [None; 4];
            new_tet_indices.push(slot);
        } else {
            let idx = mesh.tets.len();
            mesh.tets.push(*new_tet);
            mesh.neighbors.push([None; 4]);
            new_tet_indices.push(idx);
        }
    }

    for &slot in cavity.iter().skip(new_tets.len()) {
        mesh.tets[slot] = TetMesh::TOMBSTONE;
        mesh.neighbors[slot] = [None; 4];
    }

    use std::collections::HashMap;
    let mut face_to_new: HashMap<(usize, usize, usize), (usize, usize)> = HashMap::new();
    for (ni, &nti) in new_tet_indices.iter().enumerate() {
        let tet = &mesh.tets[nti];
        for fi in 0..4 {
            let fv = face_verts(tet, fi);
            let mut key = [fv[0], fv[1], fv[2]];
            key.sort();
            let key = (key[0], key[1], key[2]);
            if let Some(&(other_ni, other_fi)) = face_to_new.get(&key) {
                let other_nti = new_tet_indices[other_ni];
                mesh.neighbors[nti][fi] = Some(other_nti);
                mesh.neighbors[other_nti][other_fi] = Some(nti);
            } else {
                face_to_new.insert(key, (ni, fi));
            }
        }
    }

    for (i, (face, outside_neighbor)) in boundary.iter().enumerate() {
        let nti = new_tet_indices[i];
        if let Some(outside) = outside_neighbor {
            let mut fv_sorted = [face[0], face[1], face[2]];
            fv_sorted.sort();
            let target_key = (fv_sorted[0], fv_sorted[1], fv_sorted[2]);

            let outside_tet = &mesh.tets[*outside];
            for ofi in 0..4 {
                let ofv = face_verts(outside_tet, ofi);
                let mut ok = [ofv[0], ofv[1], ofv[2]];
                ok.sort();
                let ok = (ok[0], ok[1], ok[2]);
                if ok == target_key {
                    mesh.neighbors[*outside][ofi] = Some(nti);
                    mesh.neighbors[nti][3] = Some(*outside);
                    break;
                }
            }
        }
    }

    mesh.compact_tombstones();

    Ok(vi)
}

pub fn build_delaunay(points: &[Point3]) -> Result<TetMesh, String> {
    if points.len() < 4 {
        return Err("build_delaunay: at least 4 points are required".to_string());
    }

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut min_z = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut max_z = f64::NEG_INFINITY;

    for p in points {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        min_z = min_z.min(p.z);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
        max_z = max_z.max(p.z);
    }

    let cx = (min_x + max_x) / 2.0;
    let cy = (min_y + max_y) / 2.0;
    let cz = (min_z + max_z) / 2.0;
    let dx = max_x - min_x;
    let dy = max_y - min_y;
    let dz = max_z - min_z;
    let size = dx.max(dy).max(dz) * 4.0;
    let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();

    let sv0 = Point3::new(cx + size, cy, cz - size * inv_sqrt2);
    let sv1 = Point3::new(cx - size, cy, cz - size * inv_sqrt2);
    let sv2 = Point3::new(cx, cy + size, cz + size * inv_sqrt2);
    let sv3 = Point3::new(cx, cy - size, cz + size * inv_sqrt2);

    let mut mesh = TetMesh {
        nodes: vec![sv0, sv1, sv2, sv3],
        tets: vec![[0, 1, 2, 3]],
        neighbors: vec![[None; 4]],
    };

    {
        let a = &mesh.nodes[0];
        let b = &mesh.nodes[1];
        let c = &mesh.nodes[2];
        let d = &mesh.nodes[3];
        let ab = b.sub(a);
        let ac = c.sub(a);
        let ad = d.sub(a);
        let vol = Point3::dot(&ab, &Point3::cross(&ac, &ad));
        if vol < 0.0 {
            mesh.tets[0].swap(0, 1);
        }
    }

    let super_vert_count = 4;

    let insertion_order = brio_sort(points);
    for &i in &insertion_order {
        insert_vertex(&mut mesh, points[i])
            .map_err(|e| format!("build_delaunay: failed to insert point {}: {}", i, e))?;
    }

    for (ti, tet) in mesh.tets.iter_mut().enumerate() {
        if tet.iter().any(|&v| v < super_vert_count) {
            *tet = TetMesh::TOMBSTONE;
            mesh.neighbors[ti] = [None; 4];
        }
    }

    let tombstone_set: HashSet<usize> = mesh
        .tets
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            if *t == TetMesh::TOMBSTONE {
                Some(i)
            } else {
                None
            }
        })
        .collect();

    for nb in &mut mesh.neighbors {
        for slot in nb.iter_mut() {
            if let Some(idx) = slot {
                if tombstone_set.contains(idx) {
                    *slot = None;
                }
            }
        }
    }

    // compact
    mesh.compact_tombstones();

    //
    let n_pts = points.len();
    let brio_to_original: Vec<usize> = insertion_order.clone();

    //
    let mut new_nodes = vec![Point3::new(0.0, 0.0, 0.0); n_pts];
    let mut internal_to_original = vec![0usize; n_pts];
    for (k, &orig_idx) in brio_to_original.iter().enumerate() {
        new_nodes[orig_idx] = mesh.nodes[super_vert_count + k];
        internal_to_original[k] = orig_idx;
    }

    for tet in &mut mesh.tets {
        for v in tet.iter_mut() {
            let internal_k = *v - super_vert_count;
            *v = internal_to_original[internal_k];
        }
    }

    mesh.nodes = new_nodes;

    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signed_volume(nodes: &[Point3], tet: &[usize; 4]) -> f64 {
        let a = &nodes[tet[0]];
        let b = &nodes[tet[1]];
        let c = &nodes[tet[2]];
        let d = &nodes[tet[3]];
        let ab = b.sub(a);
        let ac = c.sub(a);
        let ad = d.sub(a);
        Point3::dot(&ab, &Point3::cross(&ac, &ad)) / 6.0
    }

    fn verify_adjacency(mesh: &TetMesh) -> Result<(), String> {
        for (ti, nb) in mesh.neighbors.iter().enumerate() {
            for (fi, &neighbor) in nb.iter().enumerate().take(4) {
                if let Some(ni) = neighbor {
                    if ni >= mesh.tets.len() {
                        return Err(format!(
                            "tet {} face {} references out-of-range neighbor {} (len={})",
                            ti,
                            fi,
                            ni,
                            mesh.tets.len()
                        ));
                    }
                    let found = mesh.neighbors[ni].contains(&Some(ti));
                    if !found {
                        return Err(format!(
                            "tet {} face {} points to tet {}, but the reverse reference is missing",
                            ti, fi, ni
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn delaunay_for_box_vertices() {
        let points = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 1.0),
            Point3::new(0.0, 1.0, 1.0),
            Point3::new(1.0, 1.0, 1.0),
        ];

        let mesh = build_delaunay(&points).expect("build_delaunay should succeed");

        for (i, tet) in mesh.tets.iter().enumerate() {
            let sv = signed_volume(&mesh.nodes, tet);
            assert!(sv > -1e-15, "tet {} has negative volume: {}", i, sv);
        }

        let total_vol: f64 = mesh
            .tets
            .iter()
            .map(|tet| signed_volume(&mesh.nodes, tet).abs())
            .sum();
        assert!(
            (total_vol - 1.0).abs() < 1e-10,
            "total volume {} != 1.0",
            total_vol
        );

        for tet in &mesh.tets {
            for &v in tet {
                assert!(v < points.len(), "super vertex {} found in result", v);
            }
        }

        verify_adjacency(&mesh).expect("adjacency should be consistent");
    }

    #[test]
    fn empty_sphere_property_for_50_random_points() {
        let mut seed: u64 = 12345;
        let mut rand_f64 = || -> f64 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed >> 33) as f64 / (1u64 << 31) as f64
        };

        let points: Vec<Point3> = (0..50)
            .map(|_| Point3::new(rand_f64(), rand_f64(), rand_f64()))
            .collect();

        let mesh = build_delaunay(&points).expect("build_delaunay should succeed");

        for (ti, tet) in mesh.tets.iter().enumerate() {
            let a = p3(&mesh.nodes[tet[0]]);
            let b = p3(&mesh.nodes[tet[1]]);
            let c = p3(&mesh.nodes[tet[2]]);
            let d = p3(&mesh.nodes[tet[3]]);

            let orient = orient3d(a, b, c, d);
            let (a, b) = if orient > 0.0 { (a, b) } else { (b, a) };

            for (vi, node) in mesh.nodes.iter().enumerate() {
                if tet.contains(&vi) {
                    continue;
                }
                let p = p3(node);
                let in_sp = insphere(a, b, c, d, p);
                assert!(
                    in_sp <= 1e-10,
                    "vertex {} lies inside the circumsphere of tet {} (insphere={})",
                    ti,
                    vi,
                    in_sp
                );
            }
        }

        verify_adjacency(&mesh).expect("adjacency should be consistent");
    }

    #[test]
    fn super_tetrahedron_vertices_are_removed() {
        let points = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
        ];

        let mesh = build_delaunay(&points).expect("build_delaunay should succeed");

        for tet in &mesh.tets {
            for &v in tet {
                assert!(v < points.len(), "super vertex {} found", v);
            }
        }
    }

    #[test]
    fn adjacency_stays_consistent_after_insert_vertex() {
        let sv0 = Point3::new(10.0, 0.0, -7.07);
        let sv1 = Point3::new(-10.0, 0.0, -7.07);
        let sv2 = Point3::new(0.0, 10.0, 7.07);
        let sv3 = Point3::new(0.0, -10.0, 7.07);

        let mut mesh = TetMesh {
            nodes: vec![sv0, sv1, sv2, sv3],
            tets: vec![[0, 1, 2, 3]],
            neighbors: vec![[None; 4]],
        };

        {
            let a = &mesh.nodes[0];
            let b = &mesh.nodes[1];
            let c = &mesh.nodes[2];
            let d = &mesh.nodes[3];
            let ab = b.sub(a);
            let ac = c.sub(a);
            let ad = d.sub(a);
            let vol = Point3::dot(&ab, &Point3::cross(&ac, &ad));
            if vol < 0.0 {
                mesh.tets[0].swap(0, 1);
            }
        }

        let vi =
            insert_vertex(&mut mesh, Point3::new(0.0, 0.0, 0.0)).expect("insert should succeed");
        assert_eq!(vi, 4);

        verify_adjacency(&mesh).expect("adjacency should be consistent after insert");

        for (i, tet) in mesh.tets.iter().enumerate() {
            let sv = signed_volume(&mesh.nodes, tet);
            assert!(sv > -1e-15, "tet {} has negative volume: {}", i, sv);
        }
    }

    #[test]
    fn box_volume_matches() {
        let mut points = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 1.0),
            Point3::new(0.0, 1.0, 1.0),
            Point3::new(1.0, 1.0, 1.0),
        ];

        points.push(Point3::new(0.5, 0.5, 0.5));
        points.push(Point3::new(0.25, 0.25, 0.25));
        points.push(Point3::new(0.75, 0.75, 0.75));

        let mesh = build_delaunay(&points).expect("build_delaunay should succeed");

        let total_vol: f64 = mesh
            .tets
            .iter()
            .map(|tet| signed_volume(&mesh.nodes, tet).abs())
            .sum();
        assert!(
            (total_vol - 1.0).abs() < 1e-10,
            "total volume {} != 1.0",
            total_vol
        );
    }

    #[test]
    fn brio_sort_is_identity_for_small_inputs() {
        let points: Vec<Point3> = (0..50).map(|i| Point3::new(i as f64, 0.0, 0.0)).collect();
        let order = brio_sort(&points);
        assert_eq!(order, (0..50).collect::<Vec<_>>());
    }

    #[test]
    fn brio_sort_covers_all_indices_for_large_inputs() {
        let mut seed: u64 = 42;
        let mut rand_f64 = || -> f64 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed >> 33) as f64 / (1u64 << 31) as f64
        };
        let points: Vec<Point3> = (0..200)
            .map(|_| Point3::new(rand_f64(), rand_f64(), rand_f64()))
            .collect();
        let order = brio_sort(&points);
        assert_eq!(order.len(), 200);
        let mut sorted = order.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            200,
            "BRIO ordering contains duplicates or omissions"
        );
    }

    #[test]
    fn brio_sort_is_deterministic() {
        let mut seed: u64 = 123;
        let mut rand_f64 = || -> f64 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed >> 33) as f64 / (1u64 << 31) as f64
        };
        let points: Vec<Point3> = (0..150)
            .map(|_| Point3::new(rand_f64(), rand_f64(), rand_f64()))
            .collect();
        let order1 = brio_sort(&points);
        let order2 = brio_sort(&points);
        assert_eq!(order1, order2, "BRIO ordering is non-deterministic");
    }

    #[test]
    fn brio_build_delaunay_preserves_vertex_order() {
        let points = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 1.0),
            Point3::new(0.0, 1.0, 1.0),
            Point3::new(1.0, 1.0, 1.0),
        ];
        let mesh = build_delaunay(&points).expect("build_delaunay");

        for (i, p) in points.iter().enumerate() {
            let m = &mesh.nodes[i];
            assert!(
                (m.x - p.x).abs() < 1e-15 && (m.y - p.y).abs() < 1e-15 && (m.z - p.z).abs() < 1e-15,
                "vertex {} position mismatch: mesh={:?}, original={:?}",
                i,
                m,
                p
            );
        }
    }
}
