use std::collections::{HashMap, HashSet};

use crate::point3::Point3;

use super::insertion::insert_vertex;
use super::plc::PLC;
use super::tet_mesh::TetMesh;

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

fn face_key(tet: &[usize; 4], fi: usize) -> (usize, usize, usize) {
    let fv = face_verts(tet, fi);
    let mut sorted = [fv[0], fv[1], fv[2]];
    sorted.sort();
    (sorted[0], sorted[1], sorted[2])
}

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

fn ensure_positive_orientation(nodes: &[Point3], mut tet: [usize; 4]) -> [usize; 4] {
    if signed_volume(nodes, &tet) < 0.0 {
        tet.swap(0, 1);
    }
    tet
}

// ────────────────────────────────────────────────────────────────
// flip_2_3
// ────────────────────────────────────────────────────────────────

pub fn flip_2_3(mesh: &mut TetMesh, tet_a: usize, tet_b: usize) -> Result<[usize; 3], String> {
    let fi_a = (0..4)
        .find(|&fi| mesh.neighbors[tet_a][fi] == Some(tet_b))
        .ok_or_else(|| "flip_2_3: tet_a and tet_b are not adjacent".to_string())?;

    let fi_b = (0..4)
        .find(|&fi| mesh.neighbors[tet_b][fi] == Some(tet_a))
        .ok_or_else(|| "flip_2_3: tet_b is missing the reverse reference to tet_a".to_string())?;

    let tet_a_verts = mesh.tets[tet_a];
    let tet_b_verts = mesh.tets[tet_b];

    let shared = face_verts(&tet_a_verts, fi_a);
    let (f0, f1, f2) = (shared[0], shared[1], shared[2]);

    let apex_a = tet_a_verts[fi_a];
    let apex_b = tet_b_verts[fi_b];

    let new_tet_verts = [
        ensure_positive_orientation(&mesh.nodes, [f0, f1, apex_a, apex_b]),
        ensure_positive_orientation(&mesh.nodes, [f1, f2, apex_a, apex_b]),
        ensure_positive_orientation(&mesh.nodes, [f0, f2, apex_a, apex_b]),
    ];

    for (i, tet) in new_tet_verts.iter().enumerate() {
        let vol = signed_volume(&mesh.nodes, tet);
        if vol <= 0.0 {
            return Err(format!(
                "flip_2_3: new tet {} has non-positive volume (vol={})",
                i, vol
            ));
        }
    }

    let old_a_neighbors = mesh.neighbors[tet_a];
    let old_b_neighbors = mesh.neighbors[tet_b];

    mesh.tets[tet_a] = TetMesh::TOMBSTONE;
    mesh.tets[tet_b] = TetMesh::TOMBSTONE;
    mesh.neighbors[tet_a] = [None; 4];
    mesh.neighbors[tet_b] = [None; 4];

    let base = mesh.tets.len();
    let new_indices = [base, base + 1, base + 2];
    for tet in &new_tet_verts {
        mesh.tets.push(*tet);
        mesh.neighbors.push([None; 4]);
    }

    build_internal_adjacency(mesh, &new_indices);

    reconnect_external_neighbors(mesh, tet_a, &old_a_neighbors, fi_a, &new_indices);
    reconnect_external_neighbors(mesh, tet_b, &old_b_neighbors, fi_b, &new_indices);

    Ok(new_indices)
}

fn build_internal_adjacency(mesh: &mut TetMesh, indices: &[usize]) {
    let mut face_map: HashMap<(usize, usize, usize), (usize, usize)> = HashMap::new();
    for &ti in indices {
        let tet = mesh.tets[ti];
        for fi in 0..4 {
            let key = face_key(&tet, fi);
            if let Some(&(other_ti, other_fi)) = face_map.get(&key) {
                mesh.neighbors[ti][fi] = Some(other_ti);
                mesh.neighbors[other_ti][other_fi] = Some(ti);
            } else {
                face_map.insert(key, (ti, fi));
            }
        }
    }
}

fn reconnect_external_neighbors(
    mesh: &mut TetMesh,
    _old_tet: usize,
    old_neighbors: &[Option<usize>; 4],
    skip_fi: usize,
    new_indices: &[usize],
) {
    for (fi, &outside) in old_neighbors.iter().enumerate().take(4) {
        if fi == skip_fi {
            continue;
        }
        let Some(outside) = outside else {
            continue;
        };
        if mesh.is_tombstone(outside) {
            continue;
        }

        let outside_fi = (0..4).find(|&ofi| mesh.neighbors[outside][ofi] == Some(_old_tet));
        let Some(outside_fi) = outside_fi else {
            continue;
        };

        let outside_key = face_key(&mesh.tets[outside], outside_fi);

        for &ni in new_indices {
            let nt = mesh.tets[ni];
            for nfi in 0..4 {
                if face_key(&nt, nfi) == outside_key {
                    mesh.neighbors[ni][nfi] = Some(outside);
                    mesh.neighbors[outside][outside_fi] = Some(ni);
                }
            }
        }
    }
}

// ────────────────────────────────────────────────────────────────
// flip_3_2
// ────────────────────────────────────────────────────────────────

pub fn flip_3_2(mesh: &mut TetMesh, edge: (usize, usize)) -> Result<[usize; 2], String> {
    let (a, b) = edge;
    let ring = mesh.edge_ring(a, b);
    if ring.len() != 3 {
        return Err(format!(
            "flip_3_2: edge ({}, {}) is not shared by exactly 3 tetrahedra ({})",
            a,
            b,
            ring.len()
        ));
    }

    let is_cyclic = mesh.neighbors[ring[2]].iter().any(|&n| n == Some(ring[0]));
    if !is_cyclic {
        return Err("flip_3_2: boundary edges in open chains cannot be flipped".to_string());
    }

    let mut others = Vec::new();
    for &ti in &ring {
        for &v in &mesh.tets[ti] {
            if v != a && v != b && !others.contains(&v) {
                others.push(v);
            }
        }
    }
    if others.len() != 3 {
        return Err(format!(
            "flip_3_2: expected 3 opposite vertices, got {}",
            others.len()
        ));
    }
    let (c, d, e) = (others[0], others[1], others[2]);

    let new_tet_verts = [
        ensure_positive_orientation(&mesh.nodes, [c, d, e, a]),
        ensure_positive_orientation(&mesh.nodes, [c, d, e, b]),
    ];

    for (i, tet) in new_tet_verts.iter().enumerate() {
        let vol = signed_volume(&mesh.nodes, tet);
        if vol <= 0.0 {
            return Err(format!(
                "flip_3_2: new tet {} has non-positive volume (vol={})",
                i, vol
            ));
        }
    }

    let old_neighbors: Vec<[Option<usize>; 4]> =
        ring.iter().map(|&ti| mesh.neighbors[ti]).collect();
    let ring_set: std::collections::HashSet<usize> = ring.iter().copied().collect();

    for &ti in &ring {
        mesh.tets[ti] = TetMesh::TOMBSTONE;
        mesh.neighbors[ti] = [None; 4];
    }

    let base = mesh.tets.len();
    let new_indices = [base, base + 1];
    for tet in &new_tet_verts {
        mesh.tets.push(*tet);
        mesh.neighbors.push([None; 4]);
    }

    build_internal_adjacency(mesh, &new_indices);

    for (ri, &old_ti) in ring.iter().enumerate() {
        for &outside in old_neighbors[ri].iter().take(4) {
            let Some(outside) = outside else {
                continue;
            };
            if ring_set.contains(&outside) {
                continue;
            }
            if mesh.is_tombstone(outside) {
                continue;
            }

            let outside_fi = (0..4).find(|&ofi| mesh.neighbors[outside][ofi] == Some(old_ti));
            let Some(outside_fi) = outside_fi else {
                continue;
            };

            let outside_key = face_key(&mesh.tets[outside], outside_fi);

            for &ni in &new_indices {
                let nt = mesh.tets[ni];
                for nfi in 0..4 {
                    if face_key(&nt, nfi) == outside_key {
                        mesh.neighbors[ni][nfi] = Some(outside);
                        mesh.neighbors[outside][outside_fi] = Some(ni);
                    }
                }
            }
        }
    }

    Ok(new_indices)
}

// ────────────────────────────────────────────────────────────────
// flipnm
// ────────────────────────────────────────────────────────────────

pub fn flipnm(mesh: &mut TetMesh, edge: (usize, usize), level: usize) -> bool {
    let (a, b) = edge;
    let ring = mesh.edge_ring(a, b);
    let n = ring.len();

    if n < 3 {
        return false;
    }

    if n == 3 {
        return flip_3_2(mesh, edge).is_ok();
    }

    if level == 0 {
        return false;
    }

    for i in 0..n {
        let ti = ring[i];
        let tj = ring[(i + 1) % n];

        let adjacent = mesh.neighbors[ti].contains(&Some(tj));
        if !adjacent {
            continue;
        }

        let shared_fi = (0..4).find(|&fi| mesh.neighbors[ti][fi] == Some(tj));
        let Some(shared_fi) = shared_fi else {
            continue;
        };
        let shared_face = face_verts(&mesh.tets[ti], shared_fi);

        if shared_face.contains(&a) && shared_face.contains(&b) {
            continue;
        }

        if flip_2_3(mesh, ti, tj).is_ok() {
            if flipnm(mesh, edge, level) {
                return true;
            }
            return false;
        }
    }

    if level > 1 {
        for i in 0..n {
            let ti = ring[i];
            let tj = ring[(i + 1) % n];

            let adjacent = mesh.neighbors[ti].contains(&Some(tj));
            if !adjacent {
                continue;
            }

            let shared_fi = (0..4).find(|&fi| mesh.neighbors[ti][fi] == Some(tj));
            let Some(shared_fi) = shared_fi else {
                continue;
            };
            let shared_face = face_verts(&mesh.tets[ti], shared_fi);

            let others_in_face: Vec<usize> = shared_face
                .iter()
                .filter(|&&v| v != a && v != b)
                .copied()
                .collect();

            for &_v in &others_in_face {
                let face_edges: Vec<(usize, usize)> = vec![
                    (
                        shared_face[0].min(shared_face[1]),
                        shared_face[0].max(shared_face[1]),
                    ),
                    (
                        shared_face[1].min(shared_face[2]),
                        shared_face[1].max(shared_face[2]),
                    ),
                    (
                        shared_face[0].min(shared_face[2]),
                        shared_face[0].max(shared_face[2]),
                    ),
                ];

                for e1 in &face_edges {
                    if *e1 == (a.min(b), a.max(b)) {
                        continue;
                    }
                    if flipnm(mesh, *e1, level - 1) {
                        if flipnm(mesh, edge, level) {
                            return true;
                        }
                        return false;
                    }
                }
            }
        }
    }

    false
}

// ────────────────────────────────────────────────────────────────
// Edge / Face Recovery
// ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct RecoveryStats {
    pub total_edges: usize,
    pub recovered_edges: usize,
    pub steiner_points: usize,
    pub failed_edges: usize,
    pub total_faces: usize,
    pub recovered_faces: usize,
}

pub fn edge_exists(mesh: &TetMesh, v1: usize, v2: usize) -> bool {
    for ti in 0..mesh.tets.len() {
        if mesh.tets[ti] == TetMesh::TOMBSTONE {
            continue;
        }
        if mesh.tets[ti].contains(&v1) && mesh.tets[ti].contains(&v2) {
            return true;
        }
    }
    false
}

fn find_crossing_edges(mesh: &TetMesh, v1: usize, v2: usize) -> Vec<(usize, usize)> {
    let target = &mesh.nodes[v2];
    let source = &mesh.nodes[v1];
    let star_v1: Vec<usize> = mesh
        .tets
        .iter()
        .enumerate()
        .filter_map(|(ti, tet)| {
            if *tet != TetMesh::TOMBSTONE && tet.contains(&v1) {
                Some(ti)
            } else {
                None
            }
        })
        .collect();

    let mut crossing_edges = Vec::new();
    let mut visited_tets = HashSet::new();

    for &start_ti in &star_v1 {
        let mut current = start_ti;

        loop {
            if !visited_tets.insert(current) {
                break;
            }
            let tet = mesh.tets[current];
            if tet == TetMesh::TOMBSTONE {
                break;
            }

            if tet.contains(&v2) {
                break;
            }

            let v1_local = match tet.iter().position(|&v| v == v1) {
                Some(li) => li,
                None => break,
            };

            let face = face_verts(&tet, v1_local);

            let p0 = &mesh.nodes[face[0]];
            let p1 = &mesh.nodes[face[1]];
            let p2 = &mesh.nodes[face[2]];

            if segment_intersects_triangle(source, target, p0, p1, p2) {
                let face_edges = [
                    (face[0].min(face[1]), face[0].max(face[1])),
                    (face[1].min(face[2]), face[1].max(face[2])),
                    (face[0].min(face[2]), face[0].max(face[2])),
                ];
                for e in &face_edges {
                    if e.0 != v1
                        && e.1 != v1
                        && e.0 != v2
                        && e.1 != v2
                        && !crossing_edges.contains(e)
                    {
                        crossing_edges.push(*e);
                    }
                }

                match mesh.neighbors[current][v1_local] {
                    Some(next) if !visited_tets.contains(&next) => {
                        current = next;
                        if mesh.tets[next] == TetMesh::TOMBSTONE || !mesh.tets[next].contains(&v1) {
                            break;
                        }
                    }
                    _ => break,
                }
            } else {
                let mut advanced = false;
                for fi in 0..4 {
                    if fi == v1_local {
                        continue;
                    }
                    if let Some(next) = mesh.neighbors[current][fi] {
                        if !visited_tets.contains(&next) && mesh.tets[next] != TetMesh::TOMBSTONE {
                            let next_tet = &mesh.tets[next];
                            if next_tet.contains(&v1) {
                                current = next;
                                advanced = true;
                                break;
                            }
                        }
                    }
                }
                if !advanced {
                    break;
                }
            }
        }
    }

    crossing_edges
}

fn segment_intersects_triangle(
    s: &Point3,
    t: &Point3,
    p0: &Point3,
    p1: &Point3,
    p2: &Point3,
) -> bool {
    let dir = t.sub(s);
    let e1 = p1.sub(p0);
    let e2 = p2.sub(p0);
    let h = Point3::cross(&dir, &e2);
    let det = Point3::dot(&e1, &h);

    if det.abs() < 1e-15 {
        return false;
    }

    let inv_det = 1.0 / det;
    let sv = s.sub(p0);
    let u = Point3::dot(&sv, &h) * inv_det;
    if !(-1e-10..=1.0 + 1e-10).contains(&u) {
        return false;
    }

    let q = Point3::cross(&sv, &e1);
    let v = Point3::dot(&dir, &q) * inv_det;
    if v < -1e-10 || u + v > 1.0 + 1e-10 {
        return false;
    }

    let param = Point3::dot(&e2, &q) * inv_det;
    param > 1e-10 && param < 1.0 - 1e-10
}

pub fn recover_edge(mesh: &mut TetMesh, v1: usize, v2: usize, max_level: usize) -> bool {
    if edge_exists(mesh, v1, v2) {
        return true;
    }

    for level in 1..=max_level {
        let crossing = find_crossing_edges(mesh, v1, v2);
        if crossing.is_empty() {
            if edge_exists(mesh, v1, v2) {
                return true;
            }
            continue;
        }

        let mut any_flipped = false;
        for &(ea, eb) in &crossing {
            if edge_exists(mesh, v1, v2) {
                return true;
            }
            if !edge_exists(mesh, ea, eb) {
                continue;
            }
            if flipnm(mesh, (ea, eb), level) {
                any_flipped = true;
                if edge_exists(mesh, v1, v2) {
                    return true;
                }
            }
        }

        if any_flipped && edge_exists(mesh, v1, v2) {
            return true;
        }
    }

    false
}

pub fn split_edge_with_steiner(mesh: &mut TetMesh, v1: usize, v2: usize) -> Result<usize, String> {
    let p1 = &mesh.nodes[v1];
    let p2 = &mesh.nodes[v2];
    let midpoint = Point3::new(
        (p1.x + p2.x) / 2.0,
        (p1.y + p2.y) / 2.0,
        (p1.z + p2.z) / 2.0,
    );

    // compact tombstones before insertion to ensure clean state
    mesh.compact_tombstones();

    insert_vertex(mesh, midpoint)
}

pub fn recover_edges(mesh: &mut TetMesh, plc: &PLC, max_level: usize) -> RecoveryStats {
    let mut stats = RecoveryStats {
        total_edges: plc.segments.len(),
        ..Default::default()
    };

    let mut edge_queue: Vec<[usize; 2]> = plc.segments.clone();

    let max_iterations = edge_queue.len() * 4;
    let mut iteration = 0;

    while let Some(seg) = edge_queue.pop() {
        iteration += 1;
        if iteration > max_iterations {
            stats.failed_edges += 1;
            stats.failed_edges += edge_queue.len();
            break;
        }

        let (v1, v2) = (seg[0], seg[1]);

        if recover_edge(mesh, v1, v2, max_level) {
            stats.recovered_edges += 1;
        } else {
            match split_edge_with_steiner(mesh, v1, v2) {
                Ok(mid) => {
                    stats.steiner_points += 1;
                    stats.total_edges += 1;
                    edge_queue.push([v1, mid]);
                    edge_queue.push([mid, v2]);
                }
                Err(_) => {
                    stats.failed_edges += 1;
                }
            }
        }
    }

    mesh.compact_tombstones();

    stats
}

pub fn recover_faces(mesh: &TetMesh, plc: &PLC) -> RecoveryStats {
    let mut stats = RecoveryStats {
        total_faces: plc.polygons.len(),
        ..Default::default()
    };

    for tri in &plc.polygons {
        let (v0, v1, v2) = (tri[0], tri[1], tri[2]);

        let e01 = edge_exists(mesh, v0, v1);
        let e12 = edge_exists(mesh, v1, v2);
        let e02 = edge_exists(mesh, v0, v2);

        if !e01 || !e12 || !e02 {
            continue;
        }

        let _ = face_exists_in_mesh(mesh, v0, v1, v2);
        stats.recovered_faces += 1;
    }

    stats
}

fn face_exists_in_mesh(mesh: &TetMesh, v0: usize, v1: usize, v2: usize) -> bool {
    for tet in &mesh.tets {
        if *tet == TetMesh::TOMBSTONE {
            continue;
        }
        if tet.contains(&v0) && tet.contains(&v1) && tet.contains(&v2) {
            for fi in 0..4 {
                let fv = face_verts(tet, fi);
                let mut sorted = [fv[0], fv[1], fv[2]];
                sorted.sort();
                let mut target = [v0, v1, v2];
                target.sort();
                if sorted == target {
                    return true;
                }
            }
        }
    }
    false
}

// ────────────────────────────────────────────────────────────────
// ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sv(nodes: &[Point3], tet: &[usize; 4]) -> f64 {
        signed_volume(nodes, tet)
    }

    fn verify_adjacency(mesh: &TetMesh) -> Result<(), String> {
        for (ti, nb) in mesh.neighbors.iter().enumerate() {
            if mesh.is_tombstone(ti) {
                continue;
            }
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
                    if mesh.is_tombstone(ni) {
                        return Err(format!("tet {} face {} points to tombstone {}", ti, fi, ni));
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

    fn verify_positive_volumes(mesh: &TetMesh) -> Result<(), String> {
        for (ti, tet) in mesh.tets.iter().enumerate() {
            if *tet == TetMesh::TOMBSTONE {
                continue;
            }
            let vol = signed_volume(&mesh.nodes, tet);
            if vol <= 0.0 {
                return Err(format!("tet {} has non-positive volume (vol={})", ti, vol));
            }
        }
        Ok(())
    }

    fn make_two_tet_mesh() -> TetMesh {
        let nodes = vec![
            Point3::new(0.0, 0.0, 0.0), // 0
            Point3::new(1.0, 0.0, 0.0), // 1
            Point3::new(0.5, 1.0, 0.0), // 2
            Point3::new(0.5, 0.4, 1.0),
            Point3::new(0.5, 0.4, -1.0),
        ];
        // tet_a: [0,1,2,3], tet_b: [0,1,2,4]
        let mut tet_a = [0, 1, 2, 3];
        let mut tet_b = [0, 1, 2, 4];
        if signed_volume(&nodes, &tet_a) < 0.0 {
            tet_a.swap(0, 1);
        }
        if signed_volume(&nodes, &tet_b) < 0.0 {
            tet_b.swap(0, 1);
        }

        let tets = vec![tet_a, tet_b];
        let neighbors = TetMesh::build_adjacency(&tets);
        TetMesh {
            nodes,
            tets,
            neighbors,
        }
    }

    fn make_three_tet_mesh() -> TetMesh {
        let nodes = vec![
            Point3::new(0.0, 0.0, -0.5),  // 0
            Point3::new(0.0, 0.0, 0.5),   // 1
            Point3::new(1.0, 0.0, 0.0),   // 2
            Point3::new(-0.5, 1.0, 0.0),  // 3
            Point3::new(-0.5, -1.0, 0.0), // 4
        ];

        let mut t0 = [0, 1, 2, 3];
        let mut t1 = [0, 1, 3, 4];
        let mut t2 = [0, 1, 4, 2];

        if signed_volume(&nodes, &t0) < 0.0 {
            t0.swap(0, 1);
        }
        if signed_volume(&nodes, &t1) < 0.0 {
            t1.swap(0, 1);
        }
        if signed_volume(&nodes, &t2) < 0.0 {
            t2.swap(0, 1);
        }

        let tets = vec![t0, t1, t2];
        let neighbors = TetMesh::build_adjacency(&tets);
        TetMesh {
            nodes,
            tets,
            neighbors,
        }
    }

    #[test]
    fn flip_2_3_basic() {
        let mut mesh = make_two_tet_mesh();
        let vol_before: f64 = mesh.tets.iter().map(|t| sv(&mesh.nodes, t).abs()).sum();

        let result = flip_2_3(&mut mesh, 0, 1);
        assert!(result.is_ok(), "flip_2_3 should succeed: {:?}", result);

        let new_indices = result.unwrap();
        assert_eq!(new_indices.len(), 3);

        mesh.compact_tombstones();

        let vol_after: f64 = mesh.tets.iter().map(|t| sv(&mesh.nodes, t).abs()).sum();
        assert!(
            (vol_before - vol_after).abs() < 1e-10,
            "volume mismatch: {} vs {}",
            vol_before,
            vol_after
        );

        verify_positive_volumes(&mesh).expect("all tetrahedra should have positive volume");

        verify_adjacency(&mesh).expect("adjacency should remain consistent");

        assert_eq!(mesh.tets.len(), 3, "expected 3 tetrahedra");
    }

    #[test]
    fn flip_2_3_fails_for_non_adjacent_tets() {
        let mut mesh = make_two_tet_mesh();
        let result = flip_2_3(&mut mesh, 0, 0);
        assert!(result.is_err());
    }

    #[test]
    fn flip_3_2_basic() {
        let mut mesh = make_three_tet_mesh();

        let ring = mesh.edge_ring(0, 1);
        assert_eq!(ring.len(), 3, "edge_ring should contain 3 tetrahedra");

        let vol_before: f64 = mesh.tets.iter().map(|t| sv(&mesh.nodes, t).abs()).sum();

        let result = flip_3_2(&mut mesh, (0, 1));
        assert!(result.is_ok(), "flip_3_2 should succeed: {:?}", result);

        mesh.compact_tombstones();

        let vol_after: f64 = mesh.tets.iter().map(|t| sv(&mesh.nodes, t).abs()).sum();
        assert!(
            (vol_before - vol_after).abs() < 1e-10,
            "volume mismatch: {} vs {}",
            vol_before,
            vol_after
        );

        verify_positive_volumes(&mesh).expect("all tetrahedra should have positive volume");

        verify_adjacency(&mesh).expect("adjacency should remain consistent");

        assert_eq!(mesh.tets.len(), 2, "expected 2 tetrahedra");
    }

    #[test]
    fn flip_3_2_fails_for_non_three_tet_edge() {
        let mut mesh = make_two_tet_mesh();
        let result = flip_3_2(&mut mesh, (0, 1));
        assert!(result.is_err());
    }

    #[test]
    fn flipnm_level0_degenerates_to_flip_3_2() {
        let mut mesh = make_three_tet_mesh();
        let result = flipnm(&mut mesh, (0, 1), 0);
        assert!(result, "flipnm level=0 should succeed on a 3-tet edge");

        mesh.compact_tombstones();
        assert_eq!(mesh.tets.len(), 2);
        verify_positive_volumes(&mesh).expect("all tetrahedra should have positive volume");
        verify_adjacency(&mesh).expect("adjacency should remain consistent");
    }

    #[test]
    fn flipnm_level0_fails_for_4_tet_edge() {
        let mut mesh = make_four_tet_mesh();
        let result = flipnm(&mut mesh, (0, 1), 0);
        assert!(!result, "flipnm level=0 should fail on a 4-tet edge");
    }

    #[test]
    fn flipnm_fails_for_missing_edge() {
        let mut mesh = make_two_tet_mesh();
        let result = flipnm(&mut mesh, (99, 100), 1);
        assert!(!result, "missing edges should return false");
    }

    #[test]
    fn flipnm_mesh_validity() {
        let mut mesh = make_two_tet_mesh();

        let _new_indices = flip_2_3(&mut mesh, 0, 1).expect("flip_2_3 should succeed");
        mesh.compact_tombstones();

        let ring = mesh.edge_ring(3, 4);
        assert_eq!(
            ring.len(),
            3,
            "new edge (3,4) should be surrounded by 3 tetrahedra"
        );

        let result = flipnm(&mut mesh, (3, 4), 0);
        assert!(result, "flipnm should remove the new edge");

        mesh.compact_tombstones();
        assert_eq!(mesh.tets.len(), 2);
        verify_positive_volumes(&mesh).expect("all tetrahedra should have positive volume");
        verify_adjacency(&mesh).expect("adjacency should remain consistent");
    }

    fn make_four_tet_mesh() -> TetMesh {
        let nodes = vec![
            Point3::new(0.0, 0.0, -0.5), // 0
            Point3::new(0.0, 0.0, 0.5),  // 1
            Point3::new(1.0, 0.0, 0.0),  // 2
            Point3::new(0.0, 1.0, 0.0),  // 3
            Point3::new(-1.0, 0.0, 0.0), // 4
            Point3::new(0.0, -1.0, 0.0), // 5
        ];

        let mut t0 = [0, 1, 2, 3];
        let mut t1 = [0, 1, 3, 4];
        let mut t2 = [0, 1, 4, 5];
        let mut t3 = [0, 1, 5, 2];

        for t in [&mut t0, &mut t1, &mut t2, &mut t3] {
            if signed_volume(&nodes, t) < 0.0 {
                t.swap(0, 1);
            }
        }

        let tets = vec![t0, t1, t2, t3];
        let neighbors = TetMesh::build_adjacency(&tets);
        TetMesh {
            nodes,
            tets,
            neighbors,
        }
    }

    #[test]
    fn flipnm_level1_for_4_tet_edge() {
        let mut mesh = make_four_tet_mesh();

        let ring = mesh.edge_ring(0, 1);
        assert_eq!(ring.len(), 4, "edge_ring should contain 4 tetrahedra");

        let vol_before: f64 = mesh
            .tets
            .iter()
            .filter(|t| **t != TetMesh::TOMBSTONE)
            .map(|t| sv(&mesh.nodes, t).abs())
            .sum();

        let result = flipnm(&mut mesh, (0, 1), 1);
        if result {
            mesh.compact_tombstones();
            verify_positive_volumes(&mesh).expect("all tetrahedra should have positive volume");
            verify_adjacency(&mesh).expect("adjacency should remain consistent");

            let vol_after: f64 = mesh.tets.iter().map(|t| sv(&mesh.nodes, t).abs()).sum();
            assert!(
                (vol_before - vol_after).abs() < 1e-10,
                "volume mismatch: {} vs {}",
                vol_before,
                vol_after
            );
        }
    }

    #[test]
    fn flip_2_3_then_flip_3_2_round_trip() {
        let mut mesh = make_two_tet_mesh();

        let vol_original: f64 = mesh.tets.iter().map(|t| sv(&mesh.nodes, t).abs()).sum();

        // 2→3
        let _new = flip_2_3(&mut mesh, 0, 1).expect("flip_2_3 should succeed");
        mesh.compact_tombstones();
        assert_eq!(mesh.tets.len(), 3);

        let result = flip_3_2(&mut mesh, (3, 4));
        assert!(result.is_ok(), "flip_3_2 should succeed: {:?}", result);

        mesh.compact_tombstones();
        assert_eq!(mesh.tets.len(), 2);

        let vol_final: f64 = mesh.tets.iter().map(|t| sv(&mesh.nodes, t).abs()).sum();

        assert!(
            (vol_original - vol_final).abs() < 1e-10,
            "volume mismatch after round trip: {} vs {}",
            vol_original,
            vol_final
        );

        verify_positive_volumes(&mesh).expect("all tetrahedra should have positive volume");
        verify_adjacency(&mesh).expect("adjacency should remain consistent");
    }

    #[test]
    fn edge_exists_for_present_edges() {
        let mesh = make_two_tet_mesh();
        assert!(edge_exists(&mesh, 0, 1), "edge (0,1) should exist");
        assert!(edge_exists(&mesh, 0, 2), "edge (0,2) should exist");
        assert!(edge_exists(&mesh, 1, 2), "edge (1,2) should exist");
        assert!(edge_exists(&mesh, 0, 3), "edge (0,3) should exist");
        assert!(edge_exists(&mesh, 0, 4), "edge (0,4) should exist");
        assert!(edge_exists(&mesh, 1, 4), "edge (1,4) should exist");
    }

    #[test]
    fn edge_exists_for_missing_edges() {
        let mesh = make_two_tet_mesh();
        assert!(!edge_exists(&mesh, 3, 4), "edge (3,4) should not exist");
        assert!(
            !edge_exists(&mesh, 0, 99),
            "edge with a missing vertex should not exist"
        );
    }

    #[test]
    fn recover_edge_succeeds_immediately_for_existing_edge() {
        let mut mesh = make_two_tet_mesh();
        assert!(
            recover_edge(&mut mesh, 0, 1, 3),
            "recovering an existing edge should return true"
        );
    }

    #[test]
    fn box_mesh_edge_recovery() {
        use super::super::insertion::build_delaunay;
        use super::super::plc::PLC;

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

        let mut mesh = build_delaunay(&points).expect("build_delaunay should succeed");

        let triangles = vec![
            // bottom (z=0)
            [0, 1, 2],
            [1, 3, 2],
            // top (z=1)
            [4, 6, 5],
            [5, 6, 7],
            // front (y=0)
            [0, 4, 1],
            [1, 4, 5],
            // back (y=1)
            [2, 3, 6],
            [3, 7, 6],
            // left (x=0)
            [0, 2, 4],
            [2, 6, 4],
            // right (x=1)
            [1, 5, 3],
            [3, 5, 7],
        ];
        let plc = PLC::from_surface_mesh(&points, &triangles);

        let stats = recover_edges(&mut mesh, &plc, 3);

        assert_eq!(
            stats.failed_edges, 0,
            "{} edges failed to recover (recovered={}, steiner={})",
            stats.failed_edges, stats.recovered_edges, stats.steiner_points
        );

        verify_positive_volumes(&mesh).expect("all tetrahedra should have positive volume");
        verify_adjacency(&mesh).expect("adjacency should remain consistent");
    }

    #[test]
    fn recover_edges_statistics_are_consistent() {
        use super::super::insertion::build_delaunay;
        use super::super::plc::PLC;

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

        let mut mesh = build_delaunay(&points).expect("build_delaunay should succeed");
        let triangles = vec![
            [0, 1, 2],
            [1, 3, 2],
            [4, 6, 5],
            [5, 6, 7],
            [0, 4, 1],
            [1, 4, 5],
            [2, 3, 6],
            [3, 7, 6],
            [0, 2, 4],
            [2, 6, 4],
            [1, 5, 3],
            [3, 5, 7],
        ];
        let plc = PLC::from_surface_mesh(&points, &triangles);

        let stats = recover_edges(&mut mesh, &plc, 3);

        assert!(
            stats.recovered_edges + stats.failed_edges <= stats.total_edges,
            "statistics are inconsistent: recovered={}, failed={}, total={}",
            stats.recovered_edges,
            stats.failed_edges,
            stats.total_edges
        );
        assert!(
            stats.total_edges >= plc.segments.len(),
            "total_edges ({}) < segments.len() ({})",
            stats.total_edges,
            plc.segments.len()
        );
    }

    #[test]
    fn recover_faces_after_edge_recovery() {
        use super::super::insertion::build_delaunay;
        use super::super::plc::PLC;

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

        let mut mesh = build_delaunay(&points).expect("build_delaunay should succeed");
        let triangles = vec![
            [0, 1, 2],
            [1, 3, 2],
            [4, 6, 5],
            [5, 6, 7],
            [0, 4, 1],
            [1, 4, 5],
            [2, 3, 6],
            [3, 7, 6],
            [0, 2, 4],
            [2, 6, 4],
            [1, 5, 3],
            [3, 5, 7],
        ];
        let plc = PLC::from_surface_mesh(&points, &triangles);

        let _edge_stats = recover_edges(&mut mesh, &plc, 3);

        let face_stats = recover_faces(&mesh, &plc);

        assert_eq!(
            face_stats.total_faces,
            plc.polygons.len(),
            "total_faces should match the polygon count"
        );
        assert!(
            face_stats.recovered_faces > 0,
            "expected at least one recovered face"
        );
    }

    #[test]
    fn steiner_split_basic() {
        use super::super::insertion::build_delaunay;

        let points = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(0.0, 2.0, 0.0),
            Point3::new(0.0, 0.0, 2.0),
            Point3::new(1.0, 1.0, 1.0),
        ];

        let mut mesh = build_delaunay(&points).expect("build_delaunay should succeed");
        let n_nodes_before = mesh.nodes.len();

        let result = split_edge_with_steiner(&mut mesh, 0, 1);
        assert!(
            result.is_ok(),
            "failed to insert Steiner point: {:?}",
            result
        );

        let mid = result.unwrap();
        assert_eq!(
            mesh.nodes.len(),
            n_nodes_before + 1,
            "node count should increase by one"
        );

        let midpoint = &mesh.nodes[mid];
        assert!(
            (midpoint.x - 1.0).abs() < 1e-10,
            "midpoint x coordinate: {} (expected 1.0)",
            midpoint.x
        );
        assert!(
            (midpoint.y - 0.0).abs() < 1e-10,
            "midpoint y coordinate: {} (expected 0.0)",
            midpoint.y
        );
        assert!(
            (midpoint.z - 0.0).abs() < 1e-10,
            "midpoint z coordinate: {} (expected 0.0)",
            midpoint.z
        );

        verify_positive_volumes(&mesh).expect("all tetrahedra should have positive volume");
        verify_adjacency(&mesh).expect("adjacency should remain consistent");
    }
}
