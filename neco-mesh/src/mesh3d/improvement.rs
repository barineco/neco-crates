use std::collections::HashSet;

use crate::point3::Point3;

use super::insertion::insert_vertex;
use super::quality::{circumcenter, max_dihedral_angle, min_dihedral_angle};
use super::recovery::{flip_2_3, flip_3_2, flipnm};
use super::tet_mesh::TetMesh;

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ImprovementParams {
    pub max_dihedral_threshold: f64,
    pub max_rounds: usize,
    pub smoothing_iterations: usize,
    pub flipnm_level: usize,
    pub quality_smoothing_iterations: usize,
    pub enable_vertex_insertion: bool,
    pub enable_edge_collapse: bool,
    pub max_insertions_per_round: usize,
}

impl Default for ImprovementParams {
    fn default() -> Self {
        Self {
            max_dihedral_threshold: 150.0_f64.to_radians(),
            max_rounds: 20,
            smoothing_iterations: 8,
            flipnm_level: 3,
            quality_smoothing_iterations: 3,
            enable_vertex_insertion: true,
            enable_edge_collapse: true,
            max_insertions_per_round: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImprovementStats {
    pub rounds: usize,
    pub flips_performed: usize,
    pub vertices_smoothed: usize,
    pub initial_bad_tets: usize,
    pub final_bad_tets: usize,
    pub vertices_inserted: usize,
    pub edges_collapsed: usize,
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

pub fn detect_boundary_nodes(mesh: &TetMesh) -> HashSet<usize> {
    let mut boundary = HashSet::new();
    for (ti, nb) in mesh.neighbors.iter().enumerate() {
        let tet = &mesh.tets[ti];
        if *tet == TetMesh::TOMBSTONE {
            continue;
        }
        for (fi, neighbor) in nb.iter().enumerate().take(4) {
            if neighbor.is_none() {
                for (li, &v) in tet.iter().enumerate() {
                    if li != fi {
                        boundary.insert(v);
                    }
                }
            }
        }
    }
    boundary
}

// ---------------------------------------------------------------------------
// Laplacian smoothing
// ---------------------------------------------------------------------------

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

pub fn smooth_vertices(
    mesh: &mut TetMesh,
    boundary_nodes: &HashSet<usize>,
    iterations: usize,
) -> usize {
    let mut total_smoothed = 0usize;

    for _ in 0..iterations {
        let n_nodes = mesh.nodes.len();
        let mut adjacency: Vec<HashSet<usize>> = vec![HashSet::new(); n_nodes];

        for tet in &mesh.tets {
            if *tet == TetMesh::TOMBSTONE {
                continue;
            }
            for i in 0..4 {
                for j in (i + 1)..4 {
                    adjacency[tet[i]].insert(tet[j]);
                    adjacency[tet[j]].insert(tet[i]);
                }
            }
        }

        for (vi, neighbors) in adjacency.iter().enumerate().take(n_nodes) {
            if boundary_nodes.contains(&vi) {
                continue;
            }
            if neighbors.is_empty() {
                continue;
            }

            let mut cx = 0.0;
            let mut cy = 0.0;
            let mut cz = 0.0;
            for &ni in neighbors {
                cx += mesh.nodes[ni].x;
                cy += mesh.nodes[ni].y;
                cz += mesh.nodes[ni].z;
            }
            let count = neighbors.len() as f64;
            let new_pos = Point3::new(cx / count, cy / count, cz / count);

            let old_pos = mesh.nodes[vi];

            mesh.nodes[vi] = new_pos;

            let mut inverted = false;
            for tet in &mesh.tets {
                if *tet == TetMesh::TOMBSTONE {
                    continue;
                }
                if !tet.contains(&vi) {
                    continue;
                }
                if signed_volume(&mesh.nodes, tet) <= 0.0 {
                    inverted = true;
                    break;
                }
            }

            if inverted {
                mesh.nodes[vi] = old_pos;
            } else {
                total_smoothed += 1;
            }
        }
    }

    total_smoothed
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

pub fn quality_smooth_vertices(
    mesh: &mut TetMesh,
    boundary_nodes: &HashSet<usize>,
    iterations: usize,
) -> usize {
    let mut total_smoothed = 0usize;
    let avg_edge = compute_avg_edge_length(mesh);
    let mut step = avg_edge * 0.1;

    for _ in 0..iterations {
        let n_nodes = mesh.nodes.len();

        let mut vert_tets: Vec<Vec<usize>> = vec![Vec::new(); n_nodes];
        for (ti, tet) in mesh.tets.iter().enumerate() {
            if *tet == TetMesh::TOMBSTONE {
                continue;
            }
            for &v in tet {
                vert_tets[v].push(ti);
            }
        }

        for (vi, adj_tets) in vert_tets.iter().enumerate().take(n_nodes) {
            if boundary_nodes.contains(&vi) {
                continue;
            }
            if adj_tets.is_empty() {
                continue;
            }

            let current_quality = adj_tets
                .iter()
                .map(|&ti| min_dihedral_angle(&mesh.nodes, &mesh.tets[ti]))
                .fold(f64::INFINITY, f64::min);

            let old_pos = mesh.nodes[vi];
            let mut best_pos = old_pos;
            let mut best_quality = current_quality;

            let directions = [
                Point3::new(step, 0.0, 0.0),
                Point3::new(-step, 0.0, 0.0),
                Point3::new(0.0, step, 0.0),
                Point3::new(0.0, -step, 0.0),
                Point3::new(0.0, 0.0, step),
                Point3::new(0.0, 0.0, -step),
            ];

            for dir in &directions {
                let candidate =
                    Point3::new(old_pos.x + dir.x, old_pos.y + dir.y, old_pos.z + dir.z);
                mesh.nodes[vi] = candidate;

                let mut inverted = false;
                let mut min_qual = f64::INFINITY;
                for &ti in adj_tets {
                    let vol = signed_volume(&mesh.nodes, &mesh.tets[ti]);
                    if vol <= 0.0 {
                        inverted = true;
                        break;
                    }
                    let q = min_dihedral_angle(&mesh.nodes, &mesh.tets[ti]);
                    if q < min_qual {
                        min_qual = q;
                    }
                }

                if !inverted && min_qual > best_quality {
                    best_quality = min_qual;
                    best_pos = candidate;
                }
            }

            if best_quality > current_quality {
                mesh.nodes[vi] = best_pos;
                total_smoothed += 1;
            } else {
                mesh.nodes[vi] = old_pos;
            }
        }

        step *= 0.7;
    }

    total_smoothed
}

fn compute_avg_edge_length(mesh: &TetMesh) -> f64 {
    let mut total = 0.0_f64;
    let mut count = 0usize;
    let mut seen = HashSet::new();

    for tet in &mesh.tets {
        if *tet == TetMesh::TOMBSTONE {
            continue;
        }
        for i in 0..4 {
            for j in (i + 1)..4 {
                let key = if tet[i] < tet[j] {
                    (tet[i], tet[j])
                } else {
                    (tet[j], tet[i])
                };
                if seen.insert(key) {
                    total += mesh.nodes[tet[i]].distance(&mesh.nodes[tet[j]]);
                    count += 1;
                }
            }
        }
    }

    if count == 0 {
        1.0
    } else {
        total / count as f64
    }
}

// ---------------------------------------------------------------------------
// Edge collapse
// ---------------------------------------------------------------------------

/// `vertices(link(a)) ∩ vertices(link(b)) == edge_link(a, b)`
fn link_condition(mesh: &TetMesh, a: usize, b: usize) -> bool {
    let link_vertices = |v: usize| -> HashSet<usize> {
        let star = mesh.star(v);
        let mut link = HashSet::new();
        for &ti in &star {
            if mesh.is_tombstone(ti) {
                continue;
            }
            let tet = &mesh.tets[ti];
            for (fi, &node) in tet.iter().enumerate() {
                if node == v {
                    for (li, &u) in tet.iter().enumerate() {
                        if li != fi {
                            link.insert(u);
                        }
                    }
                    break;
                }
            }
        }
        link
    };

    let link_a = link_vertices(a);
    let link_b = link_vertices(b);
    let intersection: HashSet<usize> = link_a.intersection(&link_b).copied().collect();

    let ring = mesh.edge_ring(a, b);
    let mut edge_link = HashSet::new();
    for &ti in &ring {
        if mesh.is_tombstone(ti) {
            continue;
        }
        let tet = &mesh.tets[ti];
        for &v in tet {
            if v != a && v != b {
                edge_link.insert(v);
            }
        }
    }

    intersection == edge_link
}

fn try_collapse_edge(
    mesh: &mut TetMesh,
    a: usize,
    b: usize,
    boundary_nodes: &HashSet<usize>,
) -> bool {
    if boundary_nodes.contains(&a) || boundary_nodes.contains(&b) {
        return false;
    }

    if !link_condition(mesh, a, b) {
        return false;
    }

    let midpoint = Point3::new(
        (mesh.nodes[a].x + mesh.nodes[b].x) * 0.5,
        (mesh.nodes[a].y + mesh.nodes[b].y) * 0.5,
        (mesh.nodes[a].z + mesh.nodes[b].z) * 0.5,
    );

    let b_star = mesh.star(b);
    for &ti in &b_star {
        if mesh.is_tombstone(ti) {
            continue;
        }
        let tet = mesh.tets[ti];
        if tet.contains(&a) && tet.contains(&b) {
            continue;
        }
        let mut new_tet = tet;
        for v in &mut new_tet {
            if *v == b {
                *v = a;
            }
        }
        let vol = signed_volume_with_pos(&mesh.nodes, &new_tet, a, &midpoint);
        if vol <= 0.0 {
            return false;
        }
    }

    mesh.nodes[a] = midpoint;

    for &ti in &b_star {
        if mesh.is_tombstone(ti) {
            continue;
        }
        for v in &mut mesh.tets[ti] {
            if *v == b {
                *v = a;
            }
        }
        let tet = mesh.tets[ti];
        let mut unique = HashSet::new();
        let is_degenerate = !tet.iter().all(|&v| unique.insert(v));
        if is_degenerate {
            mesh.tets[ti] = TetMesh::TOMBSTONE;
            mesh.neighbors[ti] = [None; 4];
        }
    }

    true
}

fn signed_volume_with_pos(
    nodes: &[Point3],
    tet: &[usize; 4],
    replaced_vertex: usize,
    new_pos: &Point3,
) -> f64 {
    let get = |v: usize| -> &Point3 {
        if v == replaced_vertex {
            new_pos
        } else {
            &nodes[v]
        }
    };
    let a = get(tet[0]);
    let b = get(tet[1]);
    let c = get(tet[2]);
    let d = get(tet[3]);
    let ab = b.sub(a);
    let ac = c.sub(a);
    let ad = d.sub(a);
    Point3::dot(&ab, &Point3::cross(&ac, &ad)) / 6.0
}

fn collapse_bad_edges(
    mesh: &mut TetMesh,
    threshold: f64,
    boundary_nodes: &HashSet<usize>,
) -> usize {
    let mut collapsed = 0usize;

    let bad_tets: Vec<usize> = mesh
        .tets
        .iter()
        .enumerate()
        .filter(|(_, tet)| {
            **tet != TetMesh::TOMBSTONE && max_dihedral_angle(&mesh.nodes, tet) > threshold
        })
        .map(|(ti, _)| ti)
        .collect();

    let mut tried_edges: HashSet<(usize, usize)> = HashSet::new();
    for &ti in &bad_tets {
        if mesh.is_tombstone(ti) {
            continue;
        }
        let tet = mesh.tets[ti];
        if tet == TetMesh::TOMBSTONE {
            continue;
        }

        let edge_pairs = [
            (tet[0], tet[1]),
            (tet[0], tet[2]),
            (tet[0], tet[3]),
            (tet[1], tet[2]),
            (tet[1], tet[3]),
            (tet[2], tet[3]),
        ];
        let mut shortest_edge = (0, 0);
        let mut shortest_len = f64::INFINITY;
        for (a, b) in edge_pairs {
            let len = mesh.nodes[a].distance(&mesh.nodes[b]);
            if len < shortest_len {
                shortest_len = len;
                shortest_edge = if a < b { (a, b) } else { (b, a) };
            }
        }

        if !tried_edges.insert(shortest_edge) {
            continue;
        }

        let (a, b) = shortest_edge;
        if try_collapse_edge(mesh, a, b, boundary_nodes) {
            collapsed += 1;
        }
    }

    if collapsed > 0 {
        mesh.compact_tombstones();
        mesh.neighbors = TetMesh::build_adjacency(&mesh.tets);
    }

    collapsed
}

// ---------------------------------------------------------------------------
// Circumcenter insertion
// ---------------------------------------------------------------------------

fn try_insert_circumcenter(mesh: &mut TetMesh, tet_idx: usize) -> bool {
    if mesh.is_tombstone(tet_idx) {
        return false;
    }
    let tet = mesh.tets[tet_idx];

    let cc = match circumcenter(&mesh.nodes, &tet) {
        Some(c) => c,
        None => return false,
    };

    let edge_pairs = [
        (tet[0], tet[1]),
        (tet[0], tet[2]),
        (tet[0], tet[3]),
        (tet[1], tet[2]),
        (tet[1], tet[3]),
        (tet[2], tet[3]),
    ];
    let mut longest_edge = 0.0_f64;
    let mut shortest_edge = f64::INFINITY;
    for (a, b) in edge_pairs {
        let len = mesh.nodes[a].distance(&mesh.nodes[b]);
        if len > longest_edge {
            longest_edge = len;
        }
        if len < shortest_edge {
            shortest_edge = len;
        }
    }

    let centroid = Point3::new(
        (mesh.nodes[tet[0]].x + mesh.nodes[tet[1]].x + mesh.nodes[tet[2]].x + mesh.nodes[tet[3]].x)
            * 0.25,
        (mesh.nodes[tet[0]].y + mesh.nodes[tet[1]].y + mesh.nodes[tet[2]].y + mesh.nodes[tet[3]].y)
            * 0.25,
        (mesh.nodes[tet[0]].z + mesh.nodes[tet[1]].z + mesh.nodes[tet[2]].z + mesh.nodes[tet[3]].z)
            * 0.25,
    );

    if cc.distance(&centroid) > 2.0 * longest_edge {
        return false;
    }

    let min_spacing = 0.1 * shortest_edge;
    for &v in &tet {
        if cc.distance(&mesh.nodes[v]) < min_spacing {
            return false;
        }
    }

    if mesh.locate_point(&cc).is_none() {
        return false;
    }

    insert_vertex(mesh, cc).is_ok()
}

fn insert_circumcenters(
    mesh: &mut TetMesh,
    threshold: f64,
    boundary_nodes: &HashSet<usize>,
    max_insertions: usize,
) -> usize {
    let mut inserted = 0usize;

    for _ in 0..max_insertions {
        let mut bad_tets: Vec<(usize, f64)> = mesh
            .tets
            .iter()
            .enumerate()
            .filter(|(_, tet)| {
                **tet != TetMesh::TOMBSTONE && max_dihedral_angle(&mesh.nodes, tet) > threshold
            })
            .map(|(ti, tet)| (ti, max_dihedral_angle(&mesh.nodes, tet)))
            .collect();

        if bad_tets.is_empty() {
            break;
        }

        bad_tets.sort_by(|a, b| b.1.total_cmp(&a.1));

        let mut did_insert = false;
        for (ti, _) in &bad_tets {
            if !mesh.is_tombstone(*ti) {
                let tet = mesh.tets[*ti];
                let all_boundary = tet.iter().all(|&v| boundary_nodes.contains(&v));
                if all_boundary {
                    continue;
                }
            }

            if try_insert_circumcenter(mesh, *ti) {
                inserted += 1;
                did_insert = true;
                break;
            }
        }

        if !did_insert {
            break;
        }
    }

    inserted
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

fn count_bad_tets(mesh: &TetMesh, threshold: f64) -> usize {
    mesh.tets
        .iter()
        .filter(|tet| {
            **tet != TetMesh::TOMBSTONE && max_dihedral_angle(&mesh.nodes, tet) > threshold
        })
        .count()
}

fn try_flip_edge(
    mesh: &mut TetMesh,
    a: usize,
    b: usize,
    threshold: f64,
    flipnm_level: usize,
) -> bool {
    let ring = mesh.edge_ring(a, b);
    if ring.is_empty() {
        return false;
    }

    let current_worst = ring
        .iter()
        .filter(|&&ti| ti < mesh.tets.len() && mesh.tets[ti] != TetMesh::TOMBSTONE)
        .map(|&ti| max_dihedral_angle(&mesh.nodes, &mesh.tets[ti]))
        .fold(0.0_f64, f64::max);

    if current_worst <= threshold {
        return false;
    }

    if ring.len() == 3 {
        let backup_tets = mesh.tets.clone();
        let backup_neighbors = mesh.neighbors.clone();

        if flip_3_2(mesh, (a, b)).is_ok() {
            let new_worst = mesh
                .tets
                .iter()
                .filter(|tet| **tet != TetMesh::TOMBSTONE)
                .map(|tet| max_dihedral_angle(&mesh.nodes, tet))
                .fold(0.0_f64, f64::max);

            if new_worst < current_worst {
                return true;
            }
            mesh.tets = backup_tets;
            mesh.neighbors = backup_neighbors;
        }
    }

    for i in 0..ring.len() {
        let ti = ring[i];
        if ti >= mesh.tets.len() || mesh.tets[ti] == TetMesh::TOMBSTONE {
            continue;
        }
        for &tj in ring.iter().skip(i + 1) {
            if tj >= mesh.tets.len() || mesh.tets[tj] == TetMesh::TOMBSTONE {
                continue;
            }
            let adjacent = mesh.neighbors[ti].contains(&Some(tj));
            if !adjacent {
                continue;
            }

            let backup_tets = mesh.tets.clone();
            let backup_neighbors = mesh.neighbors.clone();

            if flip_2_3(mesh, ti, tj).is_ok() {
                let new_worst = mesh
                    .tets
                    .iter()
                    .filter(|tet| **tet != TetMesh::TOMBSTONE)
                    .map(|tet| max_dihedral_angle(&mesh.nodes, tet))
                    .fold(0.0_f64, f64::max);

                if new_worst < current_worst {
                    return true;
                }
                mesh.tets = backup_tets;
                mesh.neighbors = backup_neighbors;
            }
        }
    }

    {
        let backup_tets = mesh.tets.clone();
        let backup_neighbors = mesh.neighbors.clone();

        if flipnm(mesh, (a, b), flipnm_level) {
            let new_worst = mesh
                .tets
                .iter()
                .filter(|tet| **tet != TetMesh::TOMBSTONE)
                .map(|tet| max_dihedral_angle(&mesh.nodes, tet))
                .fold(0.0_f64, f64::max);

            if new_worst < current_worst {
                return true;
            }
            mesh.tets = backup_tets;
            mesh.neighbors = backup_neighbors;
        }
    }

    false
}

pub fn improve_mesh(mesh: &mut TetMesh, params: &ImprovementParams) -> ImprovementStats {
    let initial_bad = count_bad_tets(mesh, params.max_dihedral_threshold);

    let mut stats = ImprovementStats {
        rounds: 0,
        flips_performed: 0,
        vertices_smoothed: 0,
        initial_bad_tets: initial_bad,
        final_bad_tets: initial_bad,
        vertices_inserted: 0,
        edges_collapsed: 0,
    };

    if initial_bad == 0 {
        return stats;
    }

    let mut boundary_nodes = detect_boundary_nodes(mesh);
    let mut prev_bad = initial_bad;

    for round in 0..params.max_rounds {
        stats.rounds = round + 1;

        let bad_tet_indices: Vec<usize> = mesh
            .tets
            .iter()
            .enumerate()
            .filter(|(_, tet)| {
                **tet != TetMesh::TOMBSTONE
                    && max_dihedral_angle(&mesh.nodes, tet) > params.max_dihedral_threshold
            })
            .map(|(ti, _)| ti)
            .collect();

        if bad_tet_indices.is_empty() {
            break;
        }

        let mut edges_to_try: Vec<(usize, usize)> = Vec::new();
        let mut edge_set: HashSet<(usize, usize)> = HashSet::new();

        for &ti in &bad_tet_indices {
            if ti >= mesh.tets.len() || mesh.tets[ti] == TetMesh::TOMBSTONE {
                continue;
            }
            let tet = mesh.tets[ti];
            let edges = [
                (tet[0], tet[1]),
                (tet[0], tet[2]),
                (tet[0], tet[3]),
                (tet[1], tet[2]),
                (tet[1], tet[3]),
                (tet[2], tet[3]),
            ];
            for (a, b) in edges {
                let key = if a < b { (a, b) } else { (b, a) };
                if edge_set.insert(key) {
                    edges_to_try.push(key);
                }
            }
        }

        for (a, b) in edges_to_try {
            if try_flip_edge(
                mesh,
                a,
                b,
                params.max_dihedral_threshold,
                params.flipnm_level,
            ) {
                stats.flips_performed += 1;
            }
        }

        // 3. Laplacian smoothing
        let smoothed = smooth_vertices(mesh, &boundary_nodes, params.smoothing_iterations);
        stats.vertices_smoothed += smoothed;

        if params.quality_smoothing_iterations > 0 {
            let q_smoothed =
                quality_smooth_vertices(mesh, &boundary_nodes, params.quality_smoothing_iterations);
            stats.vertices_smoothed += q_smoothed;
        }

        let current_bad = count_bad_tets(mesh, params.max_dihedral_threshold);
        stats.final_bad_tets = current_bad;

        if current_bad == 0 {
            break;
        }

        let flip_smooth_stagnated = current_bad >= prev_bad;

        if flip_smooth_stagnated && current_bad > 0 {
            let mut vertex_ops_improved = false;

            // 5a. edge collapse
            if params.enable_edge_collapse {
                let collapsed =
                    collapse_bad_edges(mesh, params.max_dihedral_threshold, &boundary_nodes);
                stats.edges_collapsed += collapsed;
                if collapsed > 0 {
                    vertex_ops_improved = true;
                }
            }

            let boundary_nodes_updated = detect_boundary_nodes(mesh);

            // 5c. circumcenter insertion
            if params.enable_vertex_insertion {
                let bad_count = count_bad_tets(mesh, params.max_dihedral_threshold);
                let max_ins = if params.max_insertions_per_round > 0 {
                    params.max_insertions_per_round
                } else {
                    (bad_count / 10).max(1)
                };
                let inserted = insert_circumcenters(
                    mesh,
                    params.max_dihedral_threshold,
                    &boundary_nodes_updated,
                    max_ins,
                );
                stats.vertices_inserted += inserted;
                if inserted > 0 {
                    vertex_ops_improved = true;
                }
            }

            let after_vertex_ops = count_bad_tets(mesh, params.max_dihedral_threshold);
            stats.final_bad_tets = after_vertex_ops;

            boundary_nodes = if vertex_ops_improved {
                detect_boundary_nodes(mesh)
            } else {
                boundary_nodes_updated
            };

            if !vertex_ops_improved || after_vertex_ops >= current_bad {
                break;
            }

            prev_bad = after_vertex_ops;
        } else {
            prev_bad = current_bad;
        }
    }

    stats
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh3d::insertion::build_delaunay;

    fn box_points() -> Vec<Point3> {
        vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 1.0),
            Point3::new(0.0, 1.0, 1.0),
            Point3::new(1.0, 1.0, 1.0),
        ]
    }

    #[test]
    fn test_detect_boundary_nodes_single_tet() {
        let mesh = TetMesh {
            nodes: vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
                Point3::new(0.0, 0.0, 1.0),
            ],
            tets: vec![[0, 1, 2, 3]],
            neighbors: vec![[None; 4]],
        };

        let boundary = detect_boundary_nodes(&mesh);
        assert_eq!(
            boundary.len(),
            4,
            "all vertices of a single tet should be boundary vertices: {}",
            boundary.len()
        );
        for i in 0..4 {
            assert!(
                boundary.contains(&i),
                "vertex {} is missing from the boundary set",
                i
            );
        }
    }

    #[test]
    fn test_detect_boundary_nodes_box() {
        let points = box_points();
        let mesh = build_delaunay(&points).expect("build_delaunay");
        let boundary = detect_boundary_nodes(&mesh);

        assert_eq!(
            boundary.len(),
            8,
            "all cube vertices should be boundary vertices: {}",
            boundary.len()
        );
    }

    #[test]
    fn test_smooth_vertices_no_inversion() {
        let mut points = box_points();
        points.push(Point3::new(0.4, 0.4, 0.4));
        points.push(Point3::new(0.6, 0.6, 0.6));

        let mut mesh = build_delaunay(&points).expect("build_delaunay");
        let boundary = detect_boundary_nodes(&mesh);

        smooth_vertices(&mut mesh, &boundary, 5);

        for (ti, tet) in mesh.tets.iter().enumerate() {
            if *tet == TetMesh::TOMBSTONE {
                continue;
            }
            let vol = signed_volume(&mesh.nodes, tet);
            assert!(
                vol > -1e-15,
                "tet {} became inverted after smoothing (vol={})",
                ti,
                vol
            );
        }
    }

    #[test]
    fn test_improve_mesh_basic() {
        let mut points = box_points();
        points.push(Point3::new(0.5, 0.5, 0.5));

        let mut mesh = build_delaunay(&points).expect("build_delaunay");
        let params = ImprovementParams::default();

        let stats = improve_mesh(&mut mesh, &params);

        assert!(stats.rounds <= params.max_rounds);

        for (ti, tet) in mesh.tets.iter().enumerate() {
            if *tet == TetMesh::TOMBSTONE {
                continue;
            }
            let vol = signed_volume(&mesh.nodes, tet);
            assert!(vol > -1e-15, "tet {} became inverted (vol={})", ti, vol);
        }
    }

    #[test]
    fn test_improve_mesh_no_bad_tets() {
        let points = vec![
            Point3::new(1.0, 1.0, 1.0),
            Point3::new(1.0, -1.0, -1.0),
            Point3::new(-1.0, 1.0, -1.0),
            Point3::new(-1.0, -1.0, 1.0),
        ];
        let mut mesh = build_delaunay(&points).expect("build_delaunay");
        let params = ImprovementParams::default();

        let stats = improve_mesh(&mut mesh, &params);

        assert_eq!(stats.initial_bad_tets, 0);
        assert_eq!(stats.final_bad_tets, 0);
        assert_eq!(stats.rounds, 0);
    }

    #[test]
    fn test_default_params() {
        let params = ImprovementParams::default();
        let expected = 150.0_f64.to_radians();
        assert!(
            (params.max_dihedral_threshold - expected).abs() < 1e-10,
            "threshold={}, expected={}",
            params.max_dihedral_threshold,
            expected
        );
        assert_eq!(params.max_rounds, 20);
        assert_eq!(params.smoothing_iterations, 8);
        assert_eq!(params.flipnm_level, 3);
        assert_eq!(params.quality_smoothing_iterations, 3);
        assert!(params.enable_vertex_insertion);
        assert!(params.enable_edge_collapse);
        assert_eq!(params.max_insertions_per_round, 0);
    }

    // -----------------------------------------------------------------------
    // -----------------------------------------------------------------------

    #[test]
    fn test_link_condition_valid() {
        let mut points = box_points();
        points.push(Point3::new(0.5, 0.5, 0.5));
        points.push(Point3::new(0.3, 0.3, 0.3));
        let mesh = build_delaunay(&points).expect("build_delaunay");

        let ring = mesh.edge_ring(8, 9);
        if !ring.is_empty() {
            let result = link_condition(&mesh, 8, 9);
            assert!(result, "link condition failed on an interior Delaunay edge");
        }
    }

    #[test]
    fn test_collapse_boundary_rejected() {
        let points = box_points();
        let mut mesh = build_delaunay(&points).expect("build_delaunay");
        let boundary = detect_boundary_nodes(&mesh);

        let result = try_collapse_edge(&mut mesh, 0, 1, &boundary);
        assert!(!result, "collapsing a boundary vertex should be rejected");
    }

    #[test]
    fn test_collapse_no_inversion() {
        let mut points = box_points();
        points.push(Point3::new(0.49, 0.5, 0.5));
        points.push(Point3::new(0.51, 0.5, 0.5));
        let mut mesh = build_delaunay(&points).expect("build_delaunay");
        let boundary = detect_boundary_nodes(&mesh);

        let tets_before = mesh
            .tets
            .iter()
            .filter(|t| **t != TetMesh::TOMBSTONE)
            .count();

        let _ = try_collapse_edge(&mut mesh, 8, 9, &boundary);
        mesh.compact_tombstones();
        mesh.neighbors = TetMesh::build_adjacency(&mesh.tets);

        for (ti, tet) in mesh.tets.iter().enumerate() {
            if *tet == TetMesh::TOMBSTONE {
                continue;
            }
            let vol = signed_volume(&mesh.nodes, tet);
            assert!(
                vol > -1e-15,
                "tet {} became inverted after collapse (vol={})",
                ti,
                vol
            );
        }

        let tets_after = mesh
            .tets
            .iter()
            .filter(|t| **t != TetMesh::TOMBSTONE)
            .count();
        assert!(
            tets_after <= tets_before,
            "tetrahedron count increased after collapse"
        );
    }

    // -----------------------------------------------------------------------
    // -----------------------------------------------------------------------

    #[test]
    fn test_circumcenter_insertion_mesh_integrity() {
        let mut points = box_points();
        points.push(Point3::new(0.5, 0.5, 0.5));
        let mut mesh = build_delaunay(&points).expect("build_delaunay");

        let threshold = 150.0_f64.to_radians();
        let bad_tets: Vec<usize> = mesh
            .tets
            .iter()
            .enumerate()
            .filter(|(_, tet)| {
                **tet != TetMesh::TOMBSTONE && max_dihedral_angle(&mesh.nodes, tet) > threshold
            })
            .map(|(ti, _)| ti)
            .collect();

        if let Some(&ti) = bad_tets.first() {
            let nodes_before = mesh.nodes.len();
            let success = try_insert_circumcenter(&mut mesh, ti);
            if success {
                assert!(
                    mesh.nodes.len() > nodes_before,
                    "node count did not increase after insertion"
                );
                for (ti, tet) in mesh.tets.iter().enumerate() {
                    if *tet == TetMesh::TOMBSTONE {
                        continue;
                    }
                    let vol = signed_volume(&mesh.nodes, tet);
                    assert!(
                        vol > -1e-15,
                        "tet {} became inverted after insertion (vol={})",
                        ti,
                        vol
                    );
                }
            }
        }
    }

    #[test]
    fn test_circumcenter_distance_guard() {
        let points = vec![
            Point3::new(1.0, 1.0, 1.0),
            Point3::new(1.0, -1.0, -1.0),
            Point3::new(-1.0, 1.0, -1.0),
            Point3::new(-1.0, -1.0, 1.0),
        ];
        let mut mesh = build_delaunay(&points).expect("build_delaunay");
        let boundary = detect_boundary_nodes(&mesh);
        let inserted = insert_circumcenters(&mut mesh, 150.0_f64.to_radians(), &boundary, 10);
        assert_eq!(
            inserted, 0,
            "a vertex was inserted even though there were no bad tetrahedra"
        );
    }

    // -----------------------------------------------------------------------
    // -----------------------------------------------------------------------

    #[test]
    fn test_vertex_ops_enabled_vs_disabled() {
        let mut points = box_points();
        points.push(Point3::new(0.5, 0.5, 0.5));
        points.push(Point3::new(0.2, 0.8, 0.3));
        points.push(Point3::new(0.7, 0.2, 0.8));
        points.push(Point3::new(0.1, 0.3, 0.9));

        let mut mesh_disabled = build_delaunay(&points).expect("build_delaunay");
        let params_disabled = ImprovementParams {
            enable_vertex_insertion: false,
            enable_edge_collapse: false,
            ..ImprovementParams::default()
        };
        let stats_disabled = improve_mesh(&mut mesh_disabled, &params_disabled);

        let mut mesh_enabled = build_delaunay(&points).expect("build_delaunay");
        let params_enabled = ImprovementParams::default();
        let stats_enabled = improve_mesh(&mut mesh_enabled, &params_enabled);

        assert!(
            stats_enabled.final_bad_tets <= stats_disabled.final_bad_tets,
            "enabling vertex ops produced more bad tetrahedra ({}) than disabling them ({})",
            stats_enabled.final_bad_tets,
            stats_disabled.final_bad_tets
        );

        for (ti, tet) in mesh_enabled.tets.iter().enumerate() {
            if *tet == TetMesh::TOMBSTONE {
                continue;
            }
            let vol = signed_volume(&mesh_enabled.nodes, tet);
            assert!(vol > -1e-15, "tet {} became inverted (vol={})", ti, vol);
        }
    }

    #[test]
    fn test_improvement_stats_vertex_ops_fields() {
        let mut points = box_points();
        points.push(Point3::new(0.5, 0.5, 0.5));
        let mut mesh = build_delaunay(&points).expect("build_delaunay");
        let params = ImprovementParams::default();
        let stats = improve_mesh(&mut mesh, &params);

        let _total_vertex_ops = stats.vertices_inserted + stats.edges_collapsed;
        assert!(stats.final_bad_tets <= stats.initial_bad_tets);
    }
}
