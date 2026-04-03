//! Constrained Delaunay Refinement.

use std::collections::HashSet;

use crate::point3::Point3;

use super::insertion::insert_vertex;
use super::plc::PLC;
use super::quality::{circumcenter, min_dihedral_angle, radius_edge_ratio};
use super::tet_mesh::TetMesh;

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RefinementParams {
    pub max_radius_edge_ratio: f64,
    pub min_dihedral_angle: f64,
    pub max_iterations: usize,
    pub max_steiner_points: usize,
    pub relaxed_radius_factor: f64,
}

impl Default for RefinementParams {
    fn default() -> Self {
        Self {
            max_radius_edge_ratio: 2.0,
            min_dihedral_angle: 5.0_f64.to_radians(), // 0.0873 rad
            max_iterations: 10_000,
            max_steiner_points: 50_000,
            relaxed_radius_factor: 0.3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RefinementStats {
    pub iterations: usize,
    pub steiner_points_added: usize,
    pub skinny_tets_remaining: usize,
    pub final_max_radius_edge: f64,
    pub final_min_dihedral: f64,
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

pub fn encroaches_segment(mesh: &TetMesh, plc: &PLC, point: &Point3) -> Option<usize> {
    for (i, seg) in plc.segments.iter().enumerate() {
        let a = &mesh.nodes[seg[0]];
        let b = &mesh.nodes[seg[1]];

        let mid = Point3::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5, (a.z + b.z) * 0.5);
        let radius_sq = a.distance(b).powi(2) * 0.25;
        let dist_sq =
            (point.x - mid.x).powi(2) + (point.y - mid.y).powi(2) + (point.z - mid.z).powi(2);

        if dist_sq < radius_sq - 1e-14 {
            return Some(i);
        }
    }
    None
}

pub fn split_segment(mesh: &mut TetMesh, plc: &mut PLC, seg_idx: usize) -> Result<usize, String> {
    let seg = plc.segments[seg_idx];
    let a = &mesh.nodes[seg[0]];
    let b = &mesh.nodes[seg[1]];
    let mid = Point3::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5, (a.z + b.z) * 0.5);

    let vi = insert_vertex(mesh, mid)?;

    let v0 = seg[0];
    let v1 = seg[1];
    plc.segments[seg_idx] = [v0, vi];
    plc.segments.push([vi, v1]);

    let mut new_polygons = Vec::new();
    let mut to_replace = Vec::new();

    for (pi, poly) in plc.polygons.iter().enumerate() {
        let has_v0 = poly.contains(&v0);
        let has_v1 = poly.contains(&v1);
        if has_v0 && has_v1 {
            let v2 = poly
                .iter()
                .find(|&&v| v != v0 && v != v1)
                .copied()
                .ok_or_else(|| format!("polygon {pi} does not have a third vertex"))?;
            to_replace.push((pi, [v0, vi, v2], [vi, v1, v2]));
        }
    }

    for (pi, tri1, tri2) in to_replace.into_iter().rev() {
        plc.polygons[pi] = tri1;
        new_polygons.push(tri2);
    }
    plc.polygons.extend(new_polygons);

    Ok(vi)
}

pub fn encroaches_polygon(mesh: &TetMesh, plc: &PLC, point: &Point3) -> Option<usize> {
    for (i, poly) in plc.polygons.iter().enumerate() {
        let a = &mesh.nodes[poly[0]];
        let b = &mesh.nodes[poly[1]];
        let c = &mesh.nodes[poly[2]];

        if let Some((center, radius_sq)) = triangle_circumsphere(a, b, c) {
            let dist_sq = (point.x - center.x).powi(2)
                + (point.y - center.y).powi(2)
                + (point.z - center.z).powi(2);

            if dist_sq < radius_sq - 1e-14 {
                return Some(i);
            }
        }
    }
    None
}

fn triangle_circumsphere(a: &Point3, b: &Point3, c: &Point3) -> Option<(Point3, f64)> {
    let ab = b.sub(a);
    let ac = c.sub(a);

    let normal = Point3::cross(&ab, &ac);
    let normal_len_sq = Point3::dot(&normal, &normal);
    if normal_len_sq < 1e-30 {
        return None;
    }

    //   2 * dot(ab, ab) * t1 + 2 * dot(ab, ac) * t2 = dot(ab, ab)
    //   2 * dot(ab, ac) * t1 + 2 * dot(ac, ac) * t2 = dot(ac, ac)
    let d_ab = Point3::dot(&ab, &ab);
    let d_ac = Point3::dot(&ac, &ac);
    let d_ab_ac = Point3::dot(&ab, &ac);

    let det = 2.0 * (d_ab * d_ac - d_ab_ac * d_ab_ac);
    if det.abs() < 1e-30 {
        return None;
    }

    let t1 = (d_ac * d_ab - d_ab_ac * d_ac) / det;
    let t2 = (d_ab * d_ac - d_ab_ac * d_ab) / det;

    let center = Point3::new(
        a.x + ab.x * t1 + ac.x * t2,
        a.y + ab.y * t1 + ac.y * t2,
        a.z + ab.z * t1 + ac.z * t2,
    );

    let radius_sq = (center.x - a.x).powi(2) + (center.y - a.y).powi(2) + (center.z - a.z).powi(2);

    Some((center, radius_sq))
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

fn insertion_radius(nodes: &[Point3], tet: &[usize; 4], cc: &Point3) -> f64 {
    tet.iter()
        .map(|&vi| cc.distance(&nodes[vi]))
        .fold(f64::INFINITY, f64::min)
}

fn min_edge_length(nodes: &[Point3], tet: &[usize; 4]) -> f64 {
    let mut min_len = f64::INFINITY;
    for i in 0..4 {
        for j in (i + 1)..4 {
            let len = nodes[tet[i]].distance(&nodes[tet[j]]);
            if len < min_len {
                min_len = len;
            }
        }
    }
    min_len
}

pub fn refine_mesh(
    mesh: &mut TetMesh,
    plc: &mut PLC,
    params: &RefinementParams,
) -> RefinementStats {
    let mut steiner_count = 0usize;
    let mut iterations = 0usize;
    let mut skipped_tets: HashSet<usize> = HashSet::new();

    loop {
        if iterations >= params.max_iterations || steiner_count >= params.max_steiner_points {
            break;
        }

        let mut worst_idx: Option<usize> = None;
        let mut worst_score = 0.0_f64;

        for (ti, tet) in mesh.tets.iter().enumerate() {
            if mesh.is_tombstone(ti) {
                continue;
            }
            if skipped_tets.contains(&ti) {
                continue;
            }

            let re = radius_edge_ratio(&mesh.nodes, tet);
            let dih = min_dihedral_angle(&mesh.nodes, tet);

            let is_bad = re > params.max_radius_edge_ratio
                || (params.min_dihedral_angle > 0.0 && dih < params.min_dihedral_angle);

            if is_bad {
                let dih_penalty = if dih < params.min_dihedral_angle {
                    (params.min_dihedral_angle - dih) * 10.0
                } else {
                    0.0
                };
                let score = re + dih_penalty;
                if score > worst_score {
                    worst_score = score;
                    worst_idx = Some(ti);
                }
            }
        }

        let ti = match worst_idx {
            Some(ti) => ti,
            None => break,
        };

        let tet = mesh.tets[ti];
        let cc = match circumcenter(&mesh.nodes, &tet) {
            Some(cc) => cc,
            None => {
                skipped_tets.insert(ti);
                iterations += 1;
                continue;
            }
        };

        if params.relaxed_radius_factor > 0.0 {
            let ir = insertion_radius(&mesh.nodes, &tet, &cc);
            let min_edge = min_edge_length(&mesh.nodes, &tet);
            let threshold = params.relaxed_radius_factor * min_edge;
            if ir < threshold {
                skipped_tets.insert(ti);
                iterations += 1;
                continue;
            }
        }

        if let Some(seg_idx) = encroaches_segment(mesh, plc, &cc) {
            match split_segment(mesh, plc, seg_idx) {
                Ok(_) => {
                    steiner_count += 1;
                    skipped_tets.clear();
                    iterations += 1;
                    continue;
                }
                Err(_) => {
                    iterations += 1;
                    continue;
                }
            }
        }

        if let Some(_poly_idx) = encroaches_polygon(mesh, plc, &cc) {
            match insert_vertex(mesh, cc) {
                Ok(_) => {
                    steiner_count += 1;
                    skipped_tets.clear();
                    iterations += 1;
                    continue;
                }
                Err(_) => {
                    iterations += 1;
                    continue;
                }
            }
        }

        match insert_vertex(mesh, cc) {
            Ok(_) => {
                steiner_count += 1;
                skipped_tets.clear();
            }
            Err(_) => {
                skipped_tets.insert(ti);
            }
        }

        iterations += 1;
    }

    let (skinny_count, max_re, min_dih) = compute_final_stats(mesh, params.max_radius_edge_ratio);

    RefinementStats {
        iterations,
        steiner_points_added: steiner_count,
        skinny_tets_remaining: skinny_count,
        final_max_radius_edge: max_re,
        final_min_dihedral: min_dih,
    }
}

fn compute_final_stats(mesh: &TetMesh, threshold: f64) -> (usize, f64, f64) {
    let mut skinny_count = 0usize;
    let mut max_re = 0.0_f64;
    let mut min_dih = f64::INFINITY;

    for tet in &mesh.tets {
        if *tet == TetMesh::TOMBSTONE {
            continue;
        }
        let re = radius_edge_ratio(&mesh.nodes, tet);
        if re > threshold {
            skinny_count += 1;
        }
        if re > max_re {
            max_re = re;
        }
        let dih = min_dihedral_angle(&mesh.nodes, tet);
        if dih < min_dih {
            min_dih = dih;
        }
    }

    (skinny_count, max_re, min_dih)
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh3d::insertion::build_delaunay;

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

    #[test]
    fn test_default_params() {
        let params = RefinementParams::default();
        assert!((params.max_radius_edge_ratio - 2.0).abs() < 1e-10);
        assert!((params.min_dihedral_angle - 5.0_f64.to_radians()).abs() < 1e-10);
        assert_eq!(params.max_iterations, 10_000);
        assert_eq!(params.max_steiner_points, 50_000);
        assert!((params.relaxed_radius_factor - 0.3).abs() < 1e-10);
    }

    #[test]
    fn test_no_refinement_needed() {
        let points = vec![
            Point3::new(1.0, 1.0, 1.0),
            Point3::new(1.0, -1.0, -1.0),
            Point3::new(-1.0, 1.0, -1.0),
            Point3::new(-1.0, -1.0, 1.0),
        ];
        let mut mesh = build_delaunay(&points).expect("build_delaunay");
        let mut plc = PLC::from_surface_mesh(&mesh.nodes, &[]);
        let params = RefinementParams::default();

        let stats = refine_mesh(&mut mesh, &mut plc, &params);

        assert_eq!(
            stats.steiner_points_added, 0,
            "no Steiner points should be necessary"
        );
        assert_eq!(
            stats.skinny_tets_remaining, 0,
            "no skinny tetrahedra should remain"
        );
    }

    #[test]
    fn test_max_iterations_termination() {
        let points = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(10.0, 0.0, 0.0),
            Point3::new(0.0, 10.0, 0.0),
            Point3::new(0.0, 0.0, 0.01),
        ];
        let mut mesh = build_delaunay(&points).expect("build_delaunay");
        let mut plc = PLC::from_surface_mesh(&mesh.nodes, &[]);
        let params = RefinementParams {
            max_radius_edge_ratio: 2.0,
            max_iterations: 3,
            max_steiner_points: 50_000,
            ..Default::default()
        };

        let stats = refine_mesh(&mut mesh, &mut plc, &params);

        assert!(
            stats.iterations <= 3,
            "iterations {} exceeded max_iterations = 3",
            stats.iterations
        );
    }

    #[test]
    fn test_basic_refinement() {
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
        let mut mesh = build_delaunay(&points).expect("build_delaunay");
        let _initial_tet_count = mesh.tets.len();

        let mut plc = PLC::from_surface_mesh(&mesh.nodes, &[]);
        let params = RefinementParams {
            max_radius_edge_ratio: 2.0,
            max_iterations: 200,
            max_steiner_points: 100,
            ..Default::default()
        };

        let stats = refine_mesh(&mut mesh, &mut plc, &params);

        assert!(
            stats.iterations <= 200,
            "iterations {} exceeded the limit of 200",
            stats.iterations
        );

        for (i, tet) in mesh.tets.iter().enumerate() {
            assert_ne!(
                *tet,
                TetMesh::TOMBSTONE,
                "tet {} is still marked as tombstone",
                i
            );
        }
    }

    #[test]
    fn test_box_volume_preservation() {
        let points = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 1.0),
            Point3::new(0.0, 1.0, 1.0),
            Point3::new(1.0, 1.0, 1.0),
            Point3::new(0.5, 0.5, 0.5),
        ];
        let mut mesh = build_delaunay(&points).expect("build_delaunay");

        let vol_before: f64 = mesh
            .tets
            .iter()
            .map(|t| signed_volume(&mesh.nodes, t).abs())
            .sum();

        let mut plc = PLC::from_surface_mesh(&mesh.nodes, &[]);
        let params = RefinementParams {
            max_radius_edge_ratio: 2.0,
            max_iterations: 100,
            max_steiner_points: 50,
            ..Default::default()
        };

        let _stats = refine_mesh(&mut mesh, &mut plc, &params);

        let vol_after: f64 = mesh
            .tets
            .iter()
            .filter(|t| **t != TetMesh::TOMBSTONE)
            .map(|t| signed_volume(&mesh.nodes, t).abs())
            .sum();

        assert!(
            (vol_before - vol_after).abs() < 0.1,
            "volume is not preserved: before={}, after={}",
            vol_before,
            vol_after
        );
    }

    #[test]
    fn test_encroaches_segment_basic() {
        let mesh = TetMesh {
            nodes: vec![Point3::new(0.0, 0.0, 0.0), Point3::new(2.0, 0.0, 0.0)],
            tets: vec![],
            neighbors: vec![],
        };
        let plc = PLC {
            vertices: mesh.nodes.clone(),
            segments: vec![[0, 1]],
            polygons: vec![],
        };

        let p_inside = Point3::new(1.0, 0.1, 0.0);
        assert!(encroaches_segment(&mesh, &plc, &p_inside).is_some());

        let p_outside = Point3::new(1.0, 2.0, 0.0);
        assert!(encroaches_segment(&mesh, &plc, &p_outside).is_none());
    }

    #[test]
    fn test_encroaches_polygon_basic() {
        let mesh = TetMesh {
            nodes: vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(2.0, 0.0, 0.0),
                Point3::new(1.0, 2.0, 0.0),
            ],
            tets: vec![],
            neighbors: vec![],
        };
        let plc = PLC {
            vertices: mesh.nodes.clone(),
            segments: vec![],
            polygons: vec![[0, 1, 2]],
        };

        let p_inside = Point3::new(1.0, 0.5, 0.0);
        assert!(encroaches_polygon(&mesh, &plc, &p_inside).is_some());

        let p_outside = Point3::new(1.0, 0.5, 10.0);
        assert!(encroaches_polygon(&mesh, &plc, &p_outside).is_none());
    }
}
