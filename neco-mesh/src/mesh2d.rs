//! 2D constrained Delaunay triangulation.

use neco_cdt::{Cdt, CdtError};
use neco_nurbs::NurbsRegion;

use crate::types::TriMesh2D;

/// Mesh a rectangle into triangles.
pub fn mesh_rect(width: f64, height: f64, max_edge: f64) -> TriMesh2D {
    let nx = (width / max_edge).ceil() as usize + 1;
    let ny = (height / max_edge).ceil() as usize + 1;
    let dx = width / (nx - 1) as f64;
    let dy = height / (ny - 1) as f64;
    let x0 = -width * 0.5;
    let y0 = -height * 0.5;

    let mut nodes = Vec::with_capacity(nx * ny);
    for j in 0..ny {
        for i in 0..nx {
            nodes.push([x0 + i as f64 * dx, y0 + j as f64 * dy]);
        }
    }

    let mut triangles = Vec::with_capacity(2 * (nx - 1) * (ny - 1));
    for j in 0..(ny - 1) {
        for i in 0..(nx - 1) {
            let bl = j * nx + i;
            let br = bl + 1;
            let tl = bl + nx;
            let tr = tl + 1;
            triangles.push([bl, br, tl]);
            triangles.push([br, tr, tl]);
        }
    }

    TriMesh2D { nodes, triangles }
}

/// Mesh a polygon boundary into triangles.
pub fn mesh_polygon(boundary: &[[f64; 2]], max_edge: f64) -> Result<TriMesh2D, CdtError> {
    mesh_polygon_adaptive(boundary, max_edge, None)
}

/// Mesh a polygon with adaptive refinement based on minimum width.
pub fn mesh_polygon_adaptive(
    boundary: &[[f64; 2]],
    max_edge: f64,
    min_nodes_per_width: Option<usize>,
) -> Result<TriMesh2D, CdtError> {
    let effective_max_edge = if let Some(min_nodes) = min_nodes_per_width {
        let min_width = polygon_min_width(boundary);
        if min_width > 0.0 && min_nodes > 1 {
            let width_based = min_width / (min_nodes - 1) as f64;
            max_edge.min(width_based)
        } else {
            max_edge
        }
    } else {
        max_edge
    };

    let (min_x, max_x, min_y, max_y) = bounding_box(boundary);
    let mut cdt = Cdt::new((min_x - 1.0, min_y - 1.0, max_x + 1.0, max_y + 1.0));
    cdt.add_constraint_edges(boundary, true)?;

    let eps = effective_max_edge * 0.01;
    let margin = effective_max_edge * 0.1;
    let mut y = min_y + effective_max_edge;
    while y < max_y - margin {
        let mut x = min_x + effective_max_edge;
        while x < max_x - margin {
            let p = [x, y];
            if point_in_polygon(&p, boundary) && !is_near_boundary_edge(&p, boundary, eps) {
                cdt.insert(x, y);
            }
            x += effective_max_edge;
        }
        y += effective_max_edge;
    }

    let user_verts = cdt.user_vertices();
    let nodes = user_verts.to_vec();
    let all_tris = cdt.triangles();

    let mut triangles = Vec::new();
    for tri in &all_tris {
        let a = user_verts[tri[0]];
        let b = user_verts[tri[1]];
        let c = user_verts[tri[2]];
        let centroid = [(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0];

        if point_in_polygon(&centroid, boundary) {
            let area2 = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
            if area2.abs() >= 1e-14 {
                triangles.push(*tri);
            }
        }
    }

    Ok(compact_trimesh(nodes, triangles))
}

/// Mesh a NURBS region (outer boundary + holes).
pub fn mesh_region(region: &NurbsRegion, max_edge: f64) -> Result<TriMesh2D, CdtError> {
    mesh_region_adaptive(region, max_edge, None)
}

/// Mesh a NURBS region with adaptive refinement.
pub fn mesh_region_adaptive(
    region: &NurbsRegion,
    max_edge: f64,
    min_nodes_per_width: Option<usize>,
) -> Result<TriMesh2D, CdtError> {
    let outer_pts = region.outer_adaptive_sample(max_edge);
    let hole_pts: Vec<Vec<[f64; 2]>> = (0..region.holes_count())
        .map(|i| region.hole_adaptive_sample(i, max_edge))
        .collect();

    let effective_max_edge = if let Some(min_nodes) = min_nodes_per_width {
        let min_width = polygon_min_width(&outer_pts);
        if min_width > 0.0 && min_nodes > 1 {
            let width_based = min_width / (min_nodes - 1) as f64;
            max_edge.min(width_based)
        } else {
            max_edge
        }
    } else {
        max_edge
    };

    let (min_x, max_x, min_y, max_y) = bounding_box(&outer_pts);
    let mut cdt = Cdt::new((min_x - 1.0, min_y - 1.0, max_x + 1.0, max_y + 1.0));
    cdt.add_constraint_edges(&outer_pts, true)?;
    for hole in &hole_pts {
        cdt.add_constraint_edges(hole, true)?;
    }

    let eps = effective_max_edge * 0.01;
    let margin = effective_max_edge * 0.1;
    let mut y = min_y + effective_max_edge;
    while y < max_y - margin {
        let mut x = min_x + effective_max_edge;
        while x < max_x - margin {
            let p = [x, y];
            if point_in_polygon(&p, &outer_pts)
                && hole_pts.iter().all(|h| !point_in_polygon(&p, h))
                && !is_near_boundary_edge(&p, &outer_pts, eps)
                && !hole_pts.iter().any(|h| is_near_boundary_edge(&p, h, eps))
            {
                cdt.insert(x, y);
            }
            x += effective_max_edge;
        }
        y += effective_max_edge;
    }

    let user_verts = cdt.user_vertices();
    let nodes = user_verts.to_vec();
    let all_tris = cdt.triangles();

    let mut triangles = Vec::new();
    for tri in &all_tris {
        let a = user_verts[tri[0]];
        let b = user_verts[tri[1]];
        let c = user_verts[tri[2]];
        let centroid = [(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0];

        if point_in_polygon(&centroid, &outer_pts)
            && hole_pts.iter().all(|h| !point_in_polygon(&centroid, h))
        {
            let area2 = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
            if area2.abs() >= 1e-14 {
                triangles.push(*tri);
            }
        }
    }

    Ok(compact_trimesh(nodes, triangles))
}

fn compact_trimesh(nodes: Vec<[f64; 2]>, triangles: Vec<[usize; 3]>) -> TriMesh2D {
    let mut used = vec![false; nodes.len()];
    for tri in &triangles {
        used[tri[0]] = true;
        used[tri[1]] = true;
        used[tri[2]] = true;
    }

    let mut remap = vec![usize::MAX; nodes.len()];
    let mut compact_nodes = Vec::with_capacity(used.iter().filter(|&&u| u).count());
    for (old_idx, node) in nodes.into_iter().enumerate() {
        if used[old_idx] {
            remap[old_idx] = compact_nodes.len();
            compact_nodes.push(node);
        }
    }

    let compact_triangles = triangles
        .into_iter()
        .map(|tri| [remap[tri[0]], remap[tri[1]], remap[tri[2]]])
        .collect();

    TriMesh2D {
        nodes: compact_nodes,
        triangles: compact_triangles,
    }
}

fn bounding_box(points: &[[f64; 2]]) -> (f64, f64, f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for p in points {
        if p[0] < min_x {
            min_x = p[0];
        }
        if p[0] > max_x {
            max_x = p[0];
        }
        if p[1] < min_y {
            min_y = p[1];
        }
        if p[1] > max_y {
            max_y = p[1];
        }
    }
    (min_x, max_x, min_y, max_y)
}

/// Test whether a point lies inside a polygon (ray-casting).
pub fn point_in_polygon(p: &[f64; 2], polygon: &[[f64; 2]]) -> bool {
    let n = polygon.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let pi = &polygon[i];
        let pj = &polygon[j];
        if ((pi[1] > p[1]) != (pj[1] > p[1]))
            && (p[0] < (pj[0] - pi[0]) * (p[1] - pi[1]) / (pj[1] - pi[1]) + pi[0])
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn is_near_boundary_edge(p: &[f64; 2], boundary: &[[f64; 2]], eps: f64) -> bool {
    for i in 0..boundary.len() {
        let j = (i + 1) % boundary.len();
        if point_to_segment_dist(p, &boundary[i], &boundary[j]) < eps {
            return true;
        }
    }
    false
}

fn point_to_segment_dist(p: &[f64; 2], a: &[f64; 2], b: &[f64; 2]) -> f64 {
    let ab_x = b[0] - a[0];
    let ab_y = b[1] - a[1];
    let ap_x = p[0] - a[0];
    let ap_y = p[1] - a[1];
    let ab_len2 = ab_x * ab_x + ab_y * ab_y;
    if ab_len2 < 1e-30 {
        return (ap_x * ap_x + ap_y * ap_y).sqrt();
    }
    let t = ((ap_x * ab_x + ap_y * ab_y) / ab_len2).clamp(0.0, 1.0);
    let proj_x = a[0] + t * ab_x;
    let proj_y = a[1] + t * ab_y;
    let dx = p[0] - proj_x;
    let dy = p[1] - proj_y;
    (dx * dx + dy * dy).sqrt()
}

fn polygon_min_width(boundary: &[[f64; 2]]) -> f64 {
    let n = boundary.len();
    if n < 3 {
        return 0.0;
    }
    let mut min_dist = f64::INFINITY;
    for i in 0..n {
        let p = &boundary[i];
        let prev = if i == 0 { n - 1 } else { i - 1 };
        for j in 0..n {
            if j == prev || j == i {
                continue;
            }
            let j_next = (j + 1) % n;
            let d = point_to_segment_dist(p, &boundary[j], &boundary[j_next]);
            if d < min_dist {
                min_dist = d;
            }
        }
    }
    min_dist
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use neco_nurbs::{NurbsCurve2D, NurbsRegion};

    fn triangle_area(mesh: &TriMesh2D, tri_idx: usize) -> f64 {
        let [i0, i1, i2] = mesh.triangles[tri_idx];
        let p0 = mesh.nodes[i0];
        let p1 = mesh.nodes[i1];
        let p2 = mesh.nodes[i2];
        0.5 * ((p1[0] - p0[0]) * (p2[1] - p0[1]) - (p2[0] - p0[0]) * (p1[1] - p0[1])).abs()
    }

    fn mesh_area(mesh: &TriMesh2D) -> f64 {
        (0..mesh.triangles.len())
            .map(|i| triangle_area(mesh, i))
            .sum()
    }

    fn triangle_centroid(mesh: &TriMesh2D, tri_idx: usize) -> [f64; 2] {
        let [i0, i1, i2] = mesh.triangles[tri_idx];
        let p0 = mesh.nodes[i0];
        let p1 = mesh.nodes[i1];
        let p2 = mesh.nodes[i2];
        [(p0[0] + p1[0] + p2[0]) / 3.0, (p0[1] + p1[1] + p2[1]) / 3.0]
    }

    fn polygon_signed_area(points: &[[f64; 2]]) -> f64 {
        let mut sum = 0.0;
        for i in 0..points.len() {
            let j = (i + 1) % points.len();
            sum += points[i][0] * points[j][1] - points[j][0] * points[i][1];
        }
        0.5 * sum
    }

    fn degree1_closed_curve(points: &[[f64; 2]]) -> NurbsCurve2D {
        let mut cp = points.to_vec();
        cp.push(points[0]);
        NurbsCurve2D::new(1, cp, vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0])
    }

    #[test]
    fn mesh_rect_area() {
        let mesh = mesh_rect(3.0, 2.0, 0.5);
        let total: f64 = (0..mesh.triangles.len())
            .map(|i| triangle_area(&mesh, i))
            .sum();
        assert_relative_eq!(total, 6.0, epsilon = 1e-12);
    }

    #[test]
    fn mesh_l_shape() {
        let verts = [
            [0.0, 0.0],
            [2.0, 0.0],
            [2.0, 1.0],
            [1.0, 1.0],
            [1.0, 2.0],
            [0.0, 2.0],
        ];
        let mesh = mesh_polygon(&verts, 0.25).unwrap();
        let total: f64 = (0..mesh.triangles.len())
            .map(|i| triangle_area(&mesh, i))
            .sum();
        assert_relative_eq!(total, 3.0, epsilon = 1e-12);
    }

    #[test]
    fn test_point_in_polygon() {
        let square = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        assert!(point_in_polygon(&[0.5, 0.5], &square));
        assert!(!point_in_polygon(&[2.0, 0.5], &square));
        assert!(!point_in_polygon(&[-0.1, 0.5], &square));
    }

    #[test]
    fn mesh_polygon_dedup_near_coincident() {
        let verts = [[0.0, 0.0], [0.008, 0.0], [0.008, 0.002], [0.0, 0.002]];
        let mesh = mesh_polygon_adaptive(&verts, 0.001, None).unwrap();
        for i in 0..mesh.triangles.len() {
            assert!(
                triangle_area(&mesh, i) > 1e-15,
                "triangle {i} is degenerate"
            );
        }
        let total: f64 = (0..mesh.triangles.len())
            .map(|i| triangle_area(&mesh, i))
            .sum();
        assert_relative_eq!(total, 0.008 * 0.002, epsilon = 1e-7);
    }

    #[test]
    fn mesh_high_aspect_ratio_bar() {
        let verts = [[0.0, 0.0], [0.200, 0.0], [0.200, 0.008], [0.0, 0.008]];
        let mesh = mesh_polygon_adaptive(&verts, 0.002, None).unwrap();
        let unique_y: std::collections::BTreeSet<i64> = mesh
            .nodes
            .iter()
            .map(|n| (n[1] * 1e6).round() as i64)
            .collect();
        assert!(
            unique_y.len() >= 4,
            "insufficient width-direction nodes: {}",
            unique_y.len()
        );
        for i in 0..mesh.triangles.len() {
            assert!(triangle_area(&mesh, i) > 1e-15);
        }
    }

    #[test]
    fn mesh_thin_triangle_bar() {
        let w = 0.008;
        let verts = [
            [0.0, 0.0],
            [0.05, 0.0],
            [0.05 + w, 0.0],
            [w * 0.5 + 0.025, 0.05 * 1.732],
            [-w * 0.5 + 0.025, 0.05 * 1.732],
            [-w, 0.0],
        ];
        let mesh = mesh_polygon_adaptive(&verts, 0.002, None).unwrap();
        assert!(!mesh.triangles.is_empty());
        for i in 0..mesh.triangles.len() {
            assert!(triangle_area(&mesh, i) > 1e-15);
        }
    }

    #[test]
    fn test_mesh_region_simple() {
        let verts = vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]];
        let mut cp = verts.clone();
        cp.push(verts[0]);
        let knots = vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0];
        let outer = NurbsCurve2D::new(1, cp, knots);
        let region = NurbsRegion {
            outer: vec![outer],
            holes: vec![],
        };
        let mesh = mesh_region(&region, 0.5).unwrap();
        assert!(mesh.nodes.len() > 4);
        assert!(mesh.triangles.len() > 2);
    }

    #[test]
    fn mesh_polygon_adaptive_translation_invariant() {
        let boundary = [[0.0, 0.0], [3.0, 0.0], [3.0, 1.5], [0.0, 1.5]];
        let shifted = [[7.25, -2.75], [10.25, -2.75], [10.25, -1.25], [7.25, -1.25]];

        let mesh = mesh_polygon_adaptive(&boundary, 0.4, None).unwrap();
        let shifted_mesh = mesh_polygon_adaptive(&shifted, 0.4, None).unwrap();

        assert_eq!(mesh.triangles.len(), shifted_mesh.triangles.len());
        assert_relative_eq!(mesh_area(&mesh), mesh_area(&shifted_mesh), epsilon = 1e-12);
    }

    #[test]
    fn mesh_region_adaptive_degree1_hole_preserves_area_and_centroid_invariants() {
        let outer = [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]];
        let hole = [[1.0, 1.0], [1.0, 3.0], [3.0, 3.0], [3.0, 1.0]];
        let region = NurbsRegion {
            outer: vec![degree1_closed_curve(&outer)],
            holes: vec![vec![degree1_closed_curve(&hole)]],
        };

        let mesh = mesh_region_adaptive(&region, 0.5, None).unwrap();
        let expected_area = polygon_signed_area(&outer).abs() - polygon_signed_area(&hole).abs();
        let mesh_area = mesh_area(&mesh);

        assert!(!mesh.triangles.is_empty(), "mesh should have triangles");
        assert_relative_eq!(mesh_area, expected_area, epsilon = 1e-10);

        for tri_idx in 0..mesh.triangles.len() {
            let centroid = triangle_centroid(&mesh, tri_idx);
            assert!(
                point_in_polygon(&centroid, &outer),
                "triangle {tri_idx} centroid should stay inside the outer boundary"
            );
            assert!(
                !point_in_polygon(&centroid, &hole),
                "triangle {tri_idx} centroid should stay outside the hole"
            );
        }
    }

    #[test]
    fn polygon_min_width_rect() {
        let verts = [[0.0, 0.0], [0.200, 0.0], [0.200, 0.008], [0.0, 0.008]];
        let w = polygon_min_width(&verts);
        assert_relative_eq!(w, 0.008, epsilon = 1e-6);
    }

    #[test]
    fn polygon_min_width_square() {
        let verts = [[0.0, 0.0], [0.01, 0.0], [0.01, 0.01], [0.0, 0.01]];
        let w = polygon_min_width(&verts);
        assert_relative_eq!(w, 0.01, epsilon = 1e-6);
    }

    #[test]
    fn mesh_polygon_adaptive_width_nodes() {
        let verts = [[0.0, 0.0], [0.200, 0.0], [0.200, 0.008], [0.0, 0.008]];
        let mesh = mesh_polygon_adaptive(&verts, 0.005, Some(5)).unwrap();
        let unique_y: std::collections::BTreeSet<i64> = mesh
            .nodes
            .iter()
            .map(|n| (n[1] * 1e6).round() as i64)
            .collect();
        assert!(
            unique_y.len() >= 5,
            "width-direction nodes {} < 5",
            unique_y.len()
        );
        let total: f64 = (0..mesh.triangles.len())
            .map(|i| triangle_area(&mesh, i))
            .sum();
        assert_relative_eq!(total, 0.200 * 0.008, epsilon = 1e-5);
    }

    #[test]
    fn mesh_polygon_adaptive_none_is_default() {
        let verts = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let mesh_default = mesh_polygon_adaptive(&verts, 0.25, None).unwrap();
        let mesh_adaptive = mesh_polygon_adaptive(&verts, 0.25, None).unwrap();
        assert_eq!(mesh_default.nodes.len(), mesh_adaptive.nodes.len());
    }

    #[test]
    fn mesh_nurbs_degree1_rotated_quad() {
        let curve = NurbsCurve2D::new(
            1,
            vec![
                [0.3746, -0.3456],
                [3.2724, 0.4308],
                [2.6254, 2.8456],
                [-0.2724, 2.0692],
                [0.3746, -0.3456],
            ],
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
        );
        let boundary = curve.adaptive_sample(0.2);
        let mesh = mesh_polygon_adaptive(&boundary, 0.2, None).unwrap();
        let total: f64 = (0..mesh.triangles.len())
            .map(|i| triangle_area(&mesh, i))
            .sum();
        assert!(!mesh.triangles.is_empty(), "mesh should have triangles");
        assert_relative_eq!(total, 7.5, epsilon = 0.01);
        let zero_tris = (0..mesh.triangles.len())
            .filter(|&i| triangle_area(&mesh, i) < 1e-14)
            .count();
        assert_eq!(zero_tris, 0, "degenerate triangles should be filtered");
    }

    #[test]
    fn mesh_polygon_adaptive_compacts_unused_nodes() {
        let curve = NurbsCurve2D::new(
            1,
            vec![[0.0, 0.0], [2.0, 0.0], [2.0, 1.5], [0.0, 1.5], [0.0, 0.0]],
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
        );
        let boundary = curve.adaptive_sample(0.15);
        let mesh = mesh_polygon_adaptive(&boundary, 0.15, None).unwrap();

        let mut used = vec![false; mesh.nodes.len()];
        for tri in &mesh.triangles {
            used[tri[0]] = true;
            used[tri[1]] = true;
            used[tri[2]] = true;
        }

        assert!(
            used.into_iter().all(|u| u),
            "mesh should not retain unused nodes"
        );
    }
}
