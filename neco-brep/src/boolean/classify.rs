//! Segment inside/outside classification via ray casting

use super::SegmentKind;
use neco_nurbs::{dedup_piecewise_sample, NurbsCurve2D};

/// Normal direction classification for overlap edges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OverlapClass {
    /// Normals of A and B point the same way (shared edge is outer boundary)
    SameDirection,
    /// Normals of A and B point opposite ways (shared edge is buried inside)
    OppositeDirection,
}

/// Segment location classification.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Location {
    Inside,
    Outside,
    Boundary(OverlapClass),
}

/// Test if point p is inside a closed piecewise NURBS boundary via ray casting.
pub fn point_in_nurbs_region(p: &[f64; 2], boundary: &[NurbsCurve2D]) -> bool {
    let polygon = dedup_piecewise_sample(boundary.iter(), 0.01);
    point_in_polygon(p, &polygon)
}

/// Point-in-polygon test via +x ray casting (odd crossing count = inside).
fn point_in_polygon(p: &[f64; 2], polygon: &[[f64; 2]]) -> bool {
    let n = polygon.len();
    if n < 3 {
        return false;
    }

    let mut crossings = 0;
    for i in 0..n {
        let j = (i + 1) % n;
        let yi = polygon[i][1];
        let yj = polygon[j][1];

        // Check if edge straddles the ray's y coordinate
        if (yi <= p[1] && yj > p[1]) || (yj <= p[1] && yi > p[1]) {
            // Compute x coordinate of intersection
            let t = (p[1] - yi) / (yj - yi);
            let x_intersect = polygon[i][0] + t * (polygon[j][0] - polygon[i][0]);
            if p[0] < x_intersect {
                crossings += 1;
            }
        }
    }

    crossings % 2 == 1
}

/// Outward normal at a point on a CCW NURBS curve (finite difference).
/// Right-hand 90-degree rotation of tangent (dx, dy) -> (dy, -dx).
fn outward_normal(curve: &NurbsCurve2D, t: f64) -> [f64; 2] {
    let h = 1e-8;
    let n = curve.control_points.len();
    let t_lo = curve.knots[curve.degree];
    let t_hi = curve.knots[n];
    let t_fwd = (t + h).min(t_hi);
    let t_bwd = (t - h).max(t_lo);
    let p_fwd = curve.evaluate(t_fwd);
    let p_bwd = curve.evaluate(t_bwd);
    let dx = p_fwd[0] - p_bwd[0];
    let dy = p_fwd[1] - p_bwd[1];
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-15 {
        return [0.0, 0.0];
    }
    [dy / len, -dx / len]
}

/// Classify overlap segment by comparing outward normals.
fn classify_overlap(
    curve_a: &NurbsCurve2D,
    t_a_mid: f64,
    curve_b: &NurbsCurve2D,
    t_b_mid: f64,
) -> OverlapClass {
    let normal_a = outward_normal(curve_a, t_a_mid);
    let normal_b = outward_normal(curve_b, t_b_mid);
    let dot = normal_a[0] * normal_b[0] + normal_a[1] * normal_b[1];
    if dot > 0.0 {
        OverlapClass::SameDirection
    } else {
        OverlapClass::OppositeDirection
    }
}

/// Classify each segment as inside/outside/boundary.
///
/// Normal segments use ray casting; overlap segments use normal comparison.
pub fn classify_segments_with_kinds(
    segments: &[NurbsCurve2D],
    kinds: &[SegmentKind],
    other_boundary: &[NurbsCurve2D],
    _my_boundary: &[NurbsCurve2D],
) -> Vec<Location> {
    let polygon = dedup_piecewise_sample(other_boundary.iter(), 0.01);

    segments
        .iter()
        .zip(kinds.iter())
        .map(|(seg, kind)| match kind {
            SegmentKind::Normal => {
                let n = seg.control_points.len();
                let t_min = seg.knots[seg.degree];
                let t_max = seg.knots[n];
                let t_mid = 0.5 * (t_min + t_max);
                let mid_pt = seg.evaluate(t_mid);
                if point_in_polygon(&mid_pt, &polygon) {
                    Location::Inside
                } else {
                    Location::Outside
                }
            }
            SegmentKind::Overlap {
                other_curve_t_mid,
                other_curve_index,
            } => {
                let n = seg.control_points.len();
                let t_min = seg.knots[seg.degree];
                let t_max = seg.knots[n];
                let t_mid = 0.5 * (t_min + t_max);
                // seg is already a split segment, take normal directly
                let other_seg = &other_boundary[*other_curve_index];
                let oc = classify_overlap(seg, t_mid, other_seg, *other_curve_t_mid);
                Location::Boundary(oc)
            }
        })
        .collect()
}
