use crate::point3::Point3;

const DEGENERATE_VOL_EPS: f64 = 1e-30;

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

fn dist(a: &Point3, b: &Point3) -> f64 {
    a.distance(b)
}

fn edge_lengths(pts: [&Point3; 4]) -> [f64; 6] {
    [
        dist(pts[0], pts[1]),
        dist(pts[0], pts[2]),
        dist(pts[0], pts[3]),
        dist(pts[1], pts[2]),
        dist(pts[1], pts[3]),
        dist(pts[2], pts[3]),
    ]
}

fn tet_pts<'a>(nodes: &'a [Point3], tet: &[usize; 4]) -> [&'a Point3; 4] {
    [
        &nodes[tet[0]],
        &nodes[tet[1]],
        &nodes[tet[2]],
        &nodes[tet[3]],
    ]
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

pub fn circumcenter(nodes: &[Point3], tet: &[usize; 4]) -> Option<Point3> {
    let [a, b, c, d] = tet_pts(nodes, tet);

    let ab = b.sub(a);
    let ac = c.sub(a);
    let ad = d.sub(a);

    let d_ab = Point3::dot(&ab, &ab);
    let d_ac = Point3::dot(&ac, &ac);
    let d_ad = Point3::dot(&ad, &ad);

    let det = ab.x * (ac.y * ad.z - ac.z * ad.y) - ab.y * (ac.x * ad.z - ac.z * ad.x)
        + ab.z * (ac.x * ad.y - ac.y * ad.x);

    if det.abs() < DEGENERATE_VOL_EPS {
        return None;
    }

    let inv_det = 1.0 / (2.0 * det);

    let ux = d_ab * (ac.y * ad.z - ac.z * ad.y) - d_ac * (ab.y * ad.z - ab.z * ad.y)
        + d_ad * (ab.y * ac.z - ab.z * ac.y);

    let uy = -(d_ab * (ac.x * ad.z - ac.z * ad.x) - d_ac * (ab.x * ad.z - ab.z * ad.x)
        + d_ad * (ab.x * ac.z - ab.z * ac.x));

    let uz = d_ab * (ac.x * ad.y - ac.y * ad.x) - d_ac * (ab.x * ad.y - ab.y * ad.x)
        + d_ad * (ab.x * ac.y - ab.y * ac.x);

    Some(Point3::new(
        a.x + ux * inv_det,
        a.y + uy * inv_det,
        a.z + uz * inv_det,
    ))
}

pub fn circumradius(nodes: &[Point3], tet: &[usize; 4]) -> f64 {
    match circumcenter(nodes, tet) {
        Some(center) => center.distance(&nodes[tet[0]]),
        None => f64::INFINITY,
    }
}

// ---------------------------------------------------------------------------
// radius-edge ratio
// ---------------------------------------------------------------------------

pub fn radius_edge_ratio(nodes: &[Point3], tet: &[usize; 4]) -> f64 {
    let pts = tet_pts(nodes, tet);
    let edges = edge_lengths(pts);

    let min_edge = edges.iter().copied().fold(f64::INFINITY, f64::min);
    if min_edge < 1e-30 {
        return f64::INFINITY;
    }

    let r = circumradius(nodes, tet);
    if r == f64::INFINITY {
        return f64::INFINITY;
    }

    r / min_edge
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

fn dihedral_angles(nodes: &[Point3], tet: &[usize; 4]) -> [f64; 6] {
    let [a, b, c, d] = tet_pts(nodes, tet);

    //

    let compute = |p0: &Point3, p1: &Point3, p2: &Point3, p3: &Point3| -> f64 {
        let e01 = p1.sub(p0);
        let e02 = p2.sub(p0);
        let e03 = p3.sub(p0);

        let e01_len_sq = Point3::dot(&e01, &e01);
        if e01_len_sq < 1e-30 {
            return 0.0;
        }

        let proj2 = Point3::dot(&e02, &e01) / e01_len_sq;
        let perp2 = e02.sub(&e01.scale(proj2));

        let proj3 = Point3::dot(&e03, &e01) / e01_len_sq;
        let perp3 = e03.sub(&e01.scale(proj3));

        let len2 = perp2.length();
        let len3 = perp3.length();
        if len2 < 1e-30 || len3 < 1e-30 {
            return 0.0;
        }

        let cos_angle = Point3::dot(&perp2, &perp3) / (len2 * len3);
        let cos_clamped = cos_angle.clamp(-1.0, 1.0);
        cos_clamped.acos()
    };

    [
        compute(a, b, c, d),
        compute(a, c, b, d),
        compute(a, d, b, c),
        compute(b, c, a, d),
        compute(b, d, a, c),
        compute(c, d, a, b),
    ]
}

pub fn min_dihedral_angle(nodes: &[Point3], tet: &[usize; 4]) -> f64 {
    let angles = dihedral_angles(nodes, tet);
    angles.iter().copied().fold(f64::INFINITY, f64::min)
}

pub fn max_dihedral_angle(nodes: &[Point3], tet: &[usize; 4]) -> f64 {
    let angles = dihedral_angles(nodes, tet);
    angles.iter().copied().fold(f64::NEG_INFINITY, f64::max)
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct QualityStats {
    pub min_radius_edge: f64,
    pub max_radius_edge: f64,
    pub mean_radius_edge: f64,
    pub min_dihedral: f64,
    pub max_dihedral: f64,
    pub num_tets: usize,
    pub num_slivers: usize,
}

pub fn mesh_quality_stats(nodes: &[Point3], tets: &[[usize; 4]], threshold: f64) -> QualityStats {
    if tets.is_empty() {
        return QualityStats {
            min_radius_edge: 0.0,
            max_radius_edge: 0.0,
            mean_radius_edge: 0.0,
            min_dihedral: 0.0,
            max_dihedral: 0.0,
            num_tets: 0,
            num_slivers: 0,
        };
    }

    let mut min_re = f64::INFINITY;
    let mut max_re = f64::NEG_INFINITY;
    let mut sum_re = 0.0;
    let mut min_dih = f64::INFINITY;
    let mut max_dih = f64::NEG_INFINITY;
    let mut num_slivers = 0usize;

    for tet in tets {
        let re = radius_edge_ratio(nodes, tet);
        if re.total_cmp(&min_re).is_lt() {
            min_re = re;
        }
        if re.total_cmp(&max_re).is_gt() {
            max_re = re;
        }
        sum_re += re;

        if re.total_cmp(&threshold).is_gt() {
            num_slivers += 1;
        }

        let dih_min = min_dihedral_angle(nodes, tet);
        let dih_max = max_dihedral_angle(nodes, tet);
        if dih_min.total_cmp(&min_dih).is_lt() {
            min_dih = dih_min;
        }
        if dih_max.total_cmp(&max_dih).is_gt() {
            max_dih = dih_max;
        }
    }

    QualityStats {
        min_radius_edge: min_re,
        max_radius_edge: max_re,
        mean_radius_edge: sum_re / tets.len() as f64,
        min_dihedral: min_dih,
        max_dihedral: max_dih,
        num_tets: tets.len(),
        num_slivers,
    }
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    const TOL: f64 = 1e-10;

    fn regular_tet() -> (Vec<Point3>, [usize; 4]) {
        let nodes = vec![
            Point3::new(1.0, 1.0, 1.0),
            Point3::new(1.0, -1.0, -1.0),
            Point3::new(-1.0, 1.0, -1.0),
            Point3::new(-1.0, -1.0, 1.0),
        ];
        (nodes, [0, 1, 2, 3])
    }

    fn degenerate_tet() -> (Vec<Point3>, [usize; 4]) {
        let nodes = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.5, 0.5, 0.0),
        ];
        (nodes, [0, 1, 2, 3])
    }

    #[test]
    fn test_circumcenter_regular_tet() {
        let (nodes, tet) = regular_tet();
        let cc = circumcenter(&nodes, &tet).expect("regular tetrahedron should not be degenerate");
        assert!(cc.x.abs() < TOL, "cx={}", cc.x);
        assert!(cc.y.abs() < TOL, "cy={}", cc.y);
        assert!(cc.z.abs() < TOL, "cz={}", cc.z);

        let r0 = cc.distance(&nodes[0]);
        for (i, node) in nodes.iter().enumerate().take(4).skip(1) {
            let ri = cc.distance(node);
            assert!(
                (ri - r0).abs() < TOL,
                "distance from vertex {i} ({ri}) differs from vertex 0 ({r0})"
            );
        }
    }

    #[test]
    fn test_circumcenter_degenerate() {
        let (nodes, tet) = degenerate_tet();
        assert!(
            circumcenter(&nodes, &tet).is_none(),
            "degenerate tetrahedron should not have a circumcenter"
        );
    }

    #[test]
    fn test_circumradius_regular_tet() {
        let (nodes, tet) = regular_tet();
        let r = circumradius(&nodes, &tet);
        let expected = 3.0_f64.sqrt();
        assert!(
            (r - expected).abs() < TOL,
            "circumradius={r}, expected={expected}"
        );
    }

    #[test]
    fn test_circumradius_degenerate() {
        let (nodes, tet) = degenerate_tet();
        assert_eq!(circumradius(&nodes, &tet), f64::INFINITY);
    }

    #[test]
    fn test_radius_edge_ratio_regular_tet() {
        let (nodes, tet) = regular_tet();
        let re = radius_edge_ratio(&nodes, &tet);
        let expected = 6.0_f64.sqrt() / 4.0;
        assert!(
            (re - expected).abs() < TOL,
            "radius_edge_ratio={re}, expected={expected}"
        );
    }

    #[test]
    fn test_radius_edge_ratio_degenerate() {
        let (nodes, tet) = degenerate_tet();
        assert_eq!(radius_edge_ratio(&nodes, &tet), f64::INFINITY);
    }

    #[test]
    fn test_dihedral_angles_regular_tet() {
        let (nodes, tet) = regular_tet();
        let expected = (1.0_f64 / 3.0).acos();

        let min_d = min_dihedral_angle(&nodes, &tet);
        let max_d = max_dihedral_angle(&nodes, &tet);

        assert!(
            (min_d - expected).abs() < TOL,
            "min_dihedral={min_d}, expected={expected}"
        );
        assert!(
            (max_d - expected).abs() < TOL,
            "max_dihedral={max_d}, expected={expected}"
        );
    }

    #[test]
    fn test_dihedral_angle_range() {
        let nodes = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
        ];
        let tet = [0, 1, 2, 3];
        let min_d = min_dihedral_angle(&nodes, &tet);
        let max_d = max_dihedral_angle(&nodes, &tet);
        assert!(min_d > 0.0, "min_dihedral should be positive: {min_d}");
        assert!(
            max_d < PI,
            "max_dihedral should be smaller than pi: {max_d}"
        );
        assert!(min_d <= max_d, "expected min <= max");
    }

    #[test]
    fn test_mesh_quality_stats() {
        let (nodes, tet) = regular_tet();
        let tets = vec![tet];
        let stats = mesh_quality_stats(&nodes, &tets, 1.0);

        assert_eq!(stats.num_tets, 1);
        assert_eq!(stats.num_slivers, 0);

        let expected_re = 6.0_f64.sqrt() / 4.0;
        assert!((stats.min_radius_edge - expected_re).abs() < TOL);
        assert!((stats.max_radius_edge - expected_re).abs() < TOL);
        assert!((stats.mean_radius_edge - expected_re).abs() < TOL);

        let expected_dih = (1.0_f64 / 3.0).acos();
        assert!((stats.min_dihedral - expected_dih).abs() < TOL);
        assert!((stats.max_dihedral - expected_dih).abs() < TOL);
    }

    #[test]
    fn test_mesh_quality_stats_empty() {
        let nodes: Vec<Point3> = vec![];
        let tets: Vec<[usize; 4]> = vec![];
        let stats = mesh_quality_stats(&nodes, &tets, 1.0);
        assert_eq!(stats.num_tets, 0);
        assert_eq!(stats.num_slivers, 0);
    }

    #[test]
    fn test_mesh_quality_stats_with_slivers() {
        let (reg_nodes, reg_tet) = regular_tet();
        let (deg_nodes, _) = degenerate_tet();

        let mut nodes = reg_nodes;
        let offset = nodes.len();
        nodes.extend_from_slice(&deg_nodes);
        let deg_tet = [offset, offset + 1, offset + 2, offset + 3];

        let tets = vec![reg_tet, deg_tet];
        let stats = mesh_quality_stats(&nodes, &tets, 1.0);

        assert_eq!(stats.num_tets, 2);
        assert_eq!(stats.num_slivers, 1);
    }
}
