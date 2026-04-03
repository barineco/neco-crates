//! NURBS curve intersection via Bezier clipping.
//!
//! 1. Decompose both curves into Bezier spans
//! 2. AABB overlap test per span pair
//! 3. Recursive AABB bisection on overlapping pairs
//! 4. Newton refinement after convergence
//! 5. Merge nearby intersections

use super::{dist2, Intersection};
use neco_nurbs::NurbsCurve2D;

/// Convergence threshold on parameter range
const CONVERGENCE_EPS: f64 = 1e-10;
/// Maximum recursion depth
const MAX_DEPTH: usize = 50;
/// Merge tolerance in parameter space
const MERGE_TOL: f64 = 1e-6;
/// Merge tolerance in spatial distance
const MERGE_SPATIAL_TOL: f64 = 1e-6;

/// Find all intersections between two NURBS curves.
pub fn find_intersections(a: &NurbsCurve2D, b: &NurbsCurve2D) -> Vec<Intersection> {
    let spans_a = a.to_bezier_spans();
    let spans_b = b.to_bezier_spans();

    let mut raw_intersections: Vec<Intersection> = Vec::new();

    for span_a in &spans_a {
        for span_b in &spans_b {
            // AABB overlap test (control point bbox contains the Bezier convex hull)
            let (a_min, a_max) = span_a.bounding_box();
            let (b_min, b_max) = span_b.bounding_box();
            if !aabb_overlap(&a_min, &a_max, &b_min, &b_max) {
                continue;
            }

            // Global parameter range
            let n_a = span_a.control_points.len();
            let n_b = span_b.control_points.len();
            let ta_min = span_a.knots[span_a.degree];
            let ta_max = span_a.knots[n_a];
            let tb_min = span_b.knots[span_b.degree];
            let tb_max = span_b.knots[n_b];

            // Recursive bisection
            subdivide_and_intersect(
                a,
                b,
                span_a.clone(),
                span_b.clone(),
                ta_min,
                ta_max,
                tb_min,
                tb_max,
                0,
                &mut raw_intersections,
            );
        }
    }

    // Merge nearby intersections
    merge_intersections(&mut raw_intersections);
    raw_intersections
}

/// AABB overlap test
fn aabb_overlap(a_min: &[f64; 2], a_max: &[f64; 2], b_min: &[f64; 2], b_max: &[f64; 2]) -> bool {
    a_min[0] <= b_max[0] && a_max[0] >= b_min[0] && a_min[1] <= b_max[1] && a_max[1] >= b_min[1]
}

/// Line segment intersection (analytic). Returns overlap endpoints for collinear case.
fn intersect_line_segments(
    original_a: &NurbsCurve2D,
    original_b: &NurbsCurve2D,
    ta_min: f64,
    ta_max: f64,
    tb_min: f64,
    tb_max: f64,
    result: &mut Vec<Intersection>,
) {
    let p0 = original_a.evaluate(ta_min);
    let p1 = original_a.evaluate(ta_max);
    let q0 = original_b.evaluate(tb_min);
    let q1 = original_b.evaluate(tb_max);

    let da = [p1[0] - p0[0], p1[1] - p0[1]];
    let db = [q1[0] - q0[0], q1[1] - q0[1]];
    let dq = [q0[0] - p0[0], q0[1] - p0[1]];

    let cross_ab = da[0] * db[1] - da[1] * db[0];
    let cross_aq = dq[0] * da[1] - dq[1] * da[0];

    const EPS: f64 = 1e-10;

    if cross_ab.abs() < EPS {
        // Parallel or collinear
        if cross_aq.abs() > EPS {
            return; // Parallel but separated
        }
        // Collinear: register overlap interval endpoints
        // Project Q0, Q1 onto A's parameter space
        let len_a_sq = da[0] * da[0] + da[1] * da[1];
        if len_a_sq < EPS {
            return;
        }
        let proj_q0 = (dq[0] * da[0] + dq[1] * da[1]) / len_a_sq;
        let dq1 = [q1[0] - p0[0], q1[1] - p0[1]];
        let proj_q1 = (dq1[0] * da[0] + dq1[1] * da[1]) / len_a_sq;

        let (s_min, s_max) = if proj_q0 < proj_q1 {
            (proj_q0, proj_q1)
        } else {
            (proj_q1, proj_q0)
        };

        // Overlap interval: [max(0, s_min), min(1, s_max)]
        let overlap_start = s_min.max(0.0);
        let overlap_end = s_max.min(1.0);
        if overlap_start >= overlap_end - EPS {
            // Point contact case
            if (overlap_start - overlap_end).abs() < EPS
                && (-EPS..=1.0 + EPS).contains(&overlap_start)
            {
                let ta = ta_min + overlap_start.clamp(0.0, 1.0) * (ta_max - ta_min);
                let pt = original_a.evaluate(ta);
                // Recover B-side parameter
                let len_b_sq = db[0] * db[0] + db[1] * db[1];
                if len_b_sq > EPS {
                    let dp = [pt[0] - q0[0], pt[1] - q0[1]];
                    let sb = (dp[0] * db[0] + dp[1] * db[1]) / len_b_sq;
                    if (-EPS..=1.0 + EPS).contains(&sb) {
                        let tb = tb_min + sb.clamp(0.0, 1.0) * (tb_max - tb_min);
                        result.push(Intersection::Point {
                            point: pt,
                            t_a: ta,
                            t_b: tb,
                        });
                    }
                }
            }
            return;
        }
        // Register overlap as a single interval
        let ta_start = ta_min + overlap_start.clamp(0.0, 1.0) * (ta_max - ta_min);
        let ta_end = ta_min + overlap_end.clamp(0.0, 1.0) * (ta_max - ta_min);
        let len_b_sq = db[0] * db[0] + db[1] * db[1];
        if len_b_sq > EPS {
            let pt_start = original_a.evaluate(ta_start);
            let pt_end = original_a.evaluate(ta_end);
            let dp_s = [pt_start[0] - q0[0], pt_start[1] - q0[1]];
            let dp_e = [pt_end[0] - q0[0], pt_end[1] - q0[1]];
            let sb_start = (dp_s[0] * db[0] + dp_s[1] * db[1]) / len_b_sq;
            let sb_end = (dp_e[0] * db[0] + dp_e[1] * db[1]) / len_b_sq;
            if sb_start >= -EPS && sb_end <= 1.0 + EPS {
                let tb_start = tb_min + sb_start.clamp(0.0, 1.0) * (tb_max - tb_min);
                let tb_end = tb_min + sb_end.clamp(0.0, 1.0) * (tb_max - tb_min);
                result.push(Intersection::Overlap {
                    t_a: (ta_start, ta_end),
                    t_b: (tb_start, tb_end),
                });
            }
        }
        return;
    }

    // Non-parallel: standard line segment intersection
    let t = (dq[0] * db[1] - dq[1] * db[0]) / cross_ab;
    let u = (dq[0] * da[1] - dq[1] * da[0]) / cross_ab;

    if (-EPS..=1.0 + EPS).contains(&t) && (-EPS..=1.0 + EPS).contains(&u) {
        let t_clamped = t.clamp(0.0, 1.0);
        let u_clamped = u.clamp(0.0, 1.0);
        let ta = ta_min + t_clamped * (ta_max - ta_min);
        let tb = tb_min + u_clamped * (tb_max - tb_min);
        let pt = original_a.evaluate(ta);
        result.push(Intersection::Point {
            point: pt,
            t_a: ta,
            t_b: tb,
        });
    }
}

/// Recursive AABB bisection for intersection finding.
///
/// Bisects the longer parameter range; recurses only when BBs overlap.
/// Degree-1 pairs (lines) are solved analytically.
#[allow(clippy::too_many_arguments)]
fn subdivide_and_intersect(
    original_a: &NurbsCurve2D,
    original_b: &NurbsCurve2D,
    curve_a: NurbsCurve2D,
    curve_b: NurbsCurve2D,
    ta_min: f64,
    ta_max: f64,
    tb_min: f64,
    tb_max: f64,
    depth: usize,
    result: &mut Vec<Intersection>,
) {
    // Control-point AABB overlap test
    let (a_min, a_max) = curve_a.bounding_box();
    let (b_min, b_max) = curve_b.bounding_box();
    if !aabb_overlap(&a_min, &a_max, &b_min, &b_max) {
        return;
    }

    // Degree-1 pair (lines): analytic solution including collinear case
    if curve_a.degree == 1 && curve_b.degree == 1 {
        intersect_line_segments(
            original_a, original_b, ta_min, ta_max, tb_min, tb_max, result,
        );
        return;
    }

    let range_a = ta_max - ta_min;
    let range_b = tb_max - tb_min;

    // Convergence check
    if range_a < CONVERGENCE_EPS && range_b < CONVERGENCE_EPS {
        let ta_mid = 0.5 * (ta_min + ta_max);
        let tb_mid = 0.5 * (tb_min + tb_max);
        if let Some(ix) = newton_refine(original_a, original_b, ta_mid, tb_mid) {
            result.push(ix);
        } else {
            // Record even if Newton did not converge, if points are close enough
            let pa = original_a.evaluate(ta_mid);
            let pb = original_b.evaluate(tb_mid);
            if dist2(pa, pb) < 1e-6 {
                result.push(Intersection::Point {
                    point: [0.5 * (pa[0] + pb[0]), 0.5 * (pa[1] + pb[1])],
                    t_a: ta_mid,
                    t_b: tb_mid,
                });
            }
        }
        return;
    }

    // Maximum depth
    if depth >= MAX_DEPTH {
        let ta_mid = 0.5 * (ta_min + ta_max);
        let tb_mid = 0.5 * (tb_min + tb_max);
        let pa = original_a.evaluate(ta_mid);
        let pb = original_b.evaluate(tb_mid);
        if dist2(pa, pb) < 1e-4 {
            result.push(Intersection::Point {
                point: [0.5 * (pa[0] + pb[0]), 0.5 * (pa[1] + pb[1])],
                t_a: ta_mid,
                t_b: tb_mid,
            });
        }
        return;
    }

    // Bisect the longer range
    if range_a >= range_b {
        let ta_mid = 0.5 * (ta_min + ta_max);
        let (left_a, right_a) = curve_a.split_at(ta_mid);
        subdivide_and_intersect(
            original_a,
            original_b,
            left_a,
            curve_b.clone(),
            ta_min,
            ta_mid,
            tb_min,
            tb_max,
            depth + 1,
            result,
        );
        subdivide_and_intersect(
            original_a,
            original_b,
            right_a,
            curve_b,
            ta_mid,
            ta_max,
            tb_min,
            tb_max,
            depth + 1,
            result,
        );
    } else {
        let tb_mid = 0.5 * (tb_min + tb_max);
        let (left_b, right_b) = curve_b.split_at(tb_mid);
        subdivide_and_intersect(
            original_a,
            original_b,
            curve_a.clone(),
            left_b,
            ta_min,
            ta_max,
            tb_min,
            tb_mid,
            depth + 1,
            result,
        );
        subdivide_and_intersect(
            original_a,
            original_b,
            curve_a,
            right_b,
            ta_min,
            ta_max,
            tb_mid,
            tb_max,
            depth + 1,
            result,
        );
    }
}

/// Newton refinement of intersection point.
///
/// Solves S1(ta) = S2(tb) using finite-difference tangents.
fn newton_refine(
    a: &NurbsCurve2D,
    b: &NurbsCurve2D,
    ta_init: f64,
    tb_init: f64,
) -> Option<Intersection> {
    let mut ta = ta_init;
    let mut tb = tb_init;

    let h = 1e-8;
    let n_a = a.control_points.len();
    let n_b = b.control_points.len();
    let ta_lo = a.knots[a.degree];
    let ta_hi = a.knots[n_a];
    let tb_lo = b.knots[b.degree];
    let tb_hi = b.knots[n_b];

    for _ in 0..5 {
        let pa = a.evaluate(ta);
        let pb = b.evaluate(tb);

        let dx = pa[0] - pb[0];
        let dy = pa[1] - pb[1];

        if dx * dx + dy * dy < 1e-24 {
            return Some(Intersection::Point {
                point: [0.5 * (pa[0] + pb[0]), 0.5 * (pa[1] + pb[1])],
                t_a: ta,
                t_b: tb,
            });
        }

        // Finite-difference tangents
        let ta_fwd = (ta + h).min(ta_hi);
        let tb_fwd = (tb + h).min(tb_hi);
        let pa_fwd = a.evaluate(ta_fwd);
        let pb_fwd = b.evaluate(tb_fwd);

        let dt_a = ta_fwd - ta;
        let dt_b = tb_fwd - tb;

        if dt_a.abs() < 1e-15 || dt_b.abs() < 1e-15 {
            break;
        }

        let dax = (pa_fwd[0] - pa[0]) / dt_a;
        let day = (pa_fwd[1] - pa[1]) / dt_a;
        let dbx = (pb_fwd[0] - pb[0]) / dt_b;
        let dby = (pb_fwd[1] - pb[1]) / dt_b;

        // J = [[dax, -dbx], [day, -dby]]
        // J * [dta, dtb]^T = -[dx, dy]^T
        let det = dax * (-dby) - (-dbx) * day;
        if det.abs() < 1e-20 {
            break;
        }

        let inv = 1.0 / det;
        let dta = inv * (-dby * (-dx) - (-dbx) * (-dy));
        let dtb = inv * (dax * (-dy) - day * (-dx));

        ta = (ta + dta).clamp(ta_lo, ta_hi);
        tb = (tb + dtb).clamp(tb_lo, tb_hi);
    }

    let pa = a.evaluate(ta);
    let pb = b.evaluate(tb);
    if dist2(pa, pb) < 1e-6 {
        Some(Intersection::Point {
            point: [0.5 * (pa[0] + pb[0]), 0.5 * (pa[1] + pb[1])],
            t_a: ta,
            t_b: tb,
        })
    } else {
        None
    }
}

/// Merge nearby intersections
fn merge_intersections(intersections: &mut Vec<Intersection>) {
    if intersections.len() <= 1 {
        return;
    }

    let mut points: Vec<Intersection> = Vec::new();
    let mut overlaps: Vec<Intersection> = Vec::new();
    for ix in intersections.drain(..) {
        match &ix {
            Intersection::Point { .. } => points.push(ix),
            Intersection::Overlap { .. } => overlaps.push(ix),
        }
    }

    points.sort_by(|a, b| {
        let ta_a = match a {
            Intersection::Point { t_a, .. } => *t_a,
            _ => 0.0,
        };
        let ta_b = match b {
            Intersection::Point { t_a, .. } => *t_a,
            _ => 0.0,
        };
        ta_a.total_cmp(&ta_b)
    });

    let mut merged_points: Vec<Intersection> = Vec::new();
    for ix in &points {
        if let Intersection::Point { point, t_a, t_b } = ix {
            let is_dup = merged_points.iter().any(|existing| {
                if let Intersection::Point {
                    point: ep,
                    t_a: eta,
                    t_b: etb,
                } = existing
                {
                    let param_close =
                        (eta - t_a).abs() < MERGE_TOL && (etb - t_b).abs() < MERGE_TOL;
                    let spatial_close = dist2(*ep, *point) < MERGE_SPATIAL_TOL;
                    param_close || spatial_close
                } else {
                    false
                }
            });
            if !is_dup {
                merged_points.push(ix.clone());
            }
        }
    }

    *intersections = merged_points;
    intersections.extend(overlaps);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a degree-1 line NURBS
    fn make_line(p0: [f64; 2], p1: [f64; 2]) -> NurbsCurve2D {
        NurbsCurve2D::new(1, vec![p0, p1], vec![0.0, 0.0, 1.0, 1.0])
    }

    #[test]
    fn test_two_lines_intersect() {
        // (0,0)->(2,2) and (0,2)->(2,0) intersection
        // Expected: 1 intersection at (1,1), t_a=0.5, t_b=0.5
        let a = make_line([0.0, 0.0], [2.0, 2.0]);
        let b = make_line([0.0, 2.0], [2.0, 0.0]);

        let result = find_intersections(&a, &b);
        assert_eq!(
            result.len(),
            1,
            "expected 1 intersection: got {}",
            result.len()
        );

        let Intersection::Point { point, t_a, t_b } = &result[0] else {
            panic!("expected Point intersection");
        };
        assert!((point[0] - 1.0).abs() < 1e-6);
        assert!((point[1] - 1.0).abs() < 1e-6);
        assert!((*t_a - 0.5).abs() < 1e-4);
        assert!((*t_b - 0.5).abs() < 1e-4);
    }

    #[test]
    fn test_quadratic_and_line() {
        // Parabola: (0,0),(1,2),(2,0) degree=2 with horizontal line y=0.5
        let parabola = NurbsCurve2D::new(
            2,
            vec![[0.0, 0.0], [1.0, 2.0], [2.0, 0.0]],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        );
        let line = make_line([-0.5, 0.5], [2.5, 0.5]);

        let result = find_intersections(&parabola, &line);
        assert_eq!(
            result.len(),
            2,
            "expected 2 intersections: got {}",
            result.len()
        );

        // Symmetric about x=1
        let mut xs: Vec<f64> = result
            .iter()
            .filter_map(|ix| {
                if let Intersection::Point { point, .. } = ix {
                    Some(point[0])
                } else {
                    None
                }
            })
            .collect();
        xs.sort_by(|a, b| a.total_cmp(b));
        assert!((xs[0] + xs[1] - 2.0).abs() < 1e-4);
        for ix in &result {
            let Intersection::Point { point, .. } = ix else {
                panic!("expected Point intersection");
            };
            assert!((point[1] - 0.5).abs() < 1e-4);
        }
    }

    #[test]
    fn test_no_intersection() {
        // Parallel horizontal lines
        let a = make_line([0.0, 0.0], [2.0, 0.0]);
        let b = make_line([0.0, 1.0], [2.0, 1.0]);

        let result = find_intersections(&a, &b);
        assert!(
            result.is_empty(),
            "expected no intersection: got {}",
            result.len()
        );
    }

    #[test]
    fn test_arc_and_line() {
        // Quarter circle (1,0)->(0,1) with line y=x
        // Intersection: (sqrt(2)/2, sqrt(2)/2)
        let w = std::f64::consts::FRAC_1_SQRT_2;
        let arc = NurbsCurve2D::new_rational(
            2,
            vec![[1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            vec![1.0, w, 1.0],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        );
        let line = make_line([0.0, 0.0], [1.0, 1.0]);

        let result = find_intersections(&arc, &line);
        assert_eq!(
            result.len(),
            1,
            "expected 1 intersection: got {}",
            result.len()
        );

        let Intersection::Point { point, .. } = &result[0] else {
            panic!("expected Point intersection");
        };
        let expected = std::f64::consts::FRAC_1_SQRT_2;
        assert!((point[0] - expected).abs() < 1e-4);
        assert!((point[1] - expected).abs() < 1e-4);
    }

    #[test]
    fn test_two_circles_intersect() {
        // Unit circle and circle centered at (1,0)
        // Two intersections expected
        let w = std::f64::consts::FRAC_1_SQRT_2;

        let circle_a = NurbsCurve2D::new_rational(
            2,
            vec![
                [1.0, 0.0],
                [1.0, 1.0],
                [0.0, 1.0],
                [-1.0, 1.0],
                [-1.0, 0.0],
                [-1.0, -1.0],
                [0.0, -1.0],
                [1.0, -1.0],
                [1.0, 0.0],
            ],
            vec![1.0, w, 1.0, w, 1.0, w, 1.0, w, 1.0],
            vec![
                0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0,
            ],
        );

        let circle_b = NurbsCurve2D::new_rational(
            2,
            vec![
                [2.0, 0.0],
                [2.0, 1.0],
                [1.0, 1.0],
                [0.0, 1.0],
                [0.0, 0.0],
                [0.0, -1.0],
                [1.0, -1.0],
                [2.0, -1.0],
                [2.0, 0.0],
            ],
            vec![1.0, w, 1.0, w, 1.0, w, 1.0, w, 1.0],
            vec![
                0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0,
            ],
        );

        let result = find_intersections(&circle_a, &circle_b);
        assert_eq!(
            result.len(),
            2,
            "expected 2 intersections: got {}",
            result.len()
        );

        for ix in &result {
            let Intersection::Point { point, .. } = ix else {
                panic!("expected Point intersection");
            };
            assert!((point[0] - 0.5).abs() < 1e-3);
            assert!((point[1].abs() - 0.75_f64.sqrt()).abs() < 1e-3);
        }

        let mut ys: Vec<f64> = result
            .iter()
            .filter_map(|ix| {
                if let Intersection::Point { point, .. } = ix {
                    Some(point[1])
                } else {
                    None
                }
            })
            .collect();
        ys.sort_by(|a, b| a.total_cmp(b));
        assert!(ys[0] < 0.0, "first y should be negative: {}", ys[0]);
        assert!(ys[1] > 0.0, "second y should be positive: {}", ys[1]);
    }
}
