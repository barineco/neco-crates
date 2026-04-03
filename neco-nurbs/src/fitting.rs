//! NURBS curve fitting with adaptive knot insertion.

use nalgebra::{DMatrix, DVector};

use crate::NurbsCurve3D;

fn dist(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// Evaluate B-spline basis function N_{i,p}(t) via Cox-de Boor recursion.
fn bspline_basis(knots: &[f64], i: usize, degree: usize, t: f64) -> f64 {
    if degree == 0 {
        return if knots[i] <= t && t < knots[i + 1] {
            1.0
        } else {
            0.0
        };
    }
    let left = {
        let denom = knots[i + degree] - knots[i];
        if denom.abs() < 1e-14 {
            0.0
        } else {
            (t - knots[i]) / denom * bspline_basis(knots, i, degree - 1, t)
        }
    };
    let right = {
        let denom = knots[i + degree + 1] - knots[i + 1];
        if denom.abs() < 1e-14 {
            0.0
        } else {
            (knots[i + degree + 1] - t) / denom * bspline_basis(knots, i + 1, degree - 1, t)
        }
    };
    left + right
}

fn bspline_basis_row(knots: &[f64], degree: usize, n_cp: usize, t: f64) -> Vec<f64> {
    let t_clamped = if (t - knots[knots.len() - 1]).abs() < 1e-14 {
        t - 1e-14
    } else {
        t
    };
    (0..n_cp)
        .map(|i| bspline_basis(knots, i, degree, t_clamped))
        .collect()
}

/// Chord-length parameterization: assign [0,1] parameters based on cumulative chord lengths.
fn chord_length_params(points: &[[f64; 3]]) -> Vec<f64> {
    let n = points.len();
    if n < 2 {
        return vec![0.0; n];
    }
    let mut dists = vec![0.0; n];
    for i in 1..n {
        dists[i] = dists[i - 1] + dist(&points[i], &points[i - 1]);
    }
    let total = dists[n - 1];
    if total < 1e-14 {
        return (0..n).map(|i| i as f64 / (n - 1) as f64).collect();
    }
    dists.iter().map(|d| d / total).collect()
}

/// Generate a clamped uniform knot vector.
fn make_uniform_knots(degree: usize, n_cp: usize) -> Vec<f64> {
    let n_knots = n_cp + degree + 1;
    let n_internal = n_knots - 2 * (degree + 1);
    let mut knots = vec![0.0; degree + 1];
    for i in 1..=n_internal {
        knots.push(i as f64 / (n_internal + 1) as f64);
    }
    knots.extend(vec![1.0; degree + 1]);
    knots
}

/// Error type for NURBS curve fitting.
#[derive(Debug)]
pub enum NurbsFitError {
    InsufficientPoints { given: usize, min_required: usize },
    DegenerateInput(String),
    SingularSystem,
    ConvergenceFailure { max_err: f64 },
}

impl std::fmt::Display for NurbsFitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InsufficientPoints {
                given,
                min_required,
            } => {
                write!(
                    f,
                    "insufficient points: {given} (minimum {min_required} required)"
                )
            }
            Self::DegenerateInput(msg) => write!(f, "degenerate input: {msg}"),
            Self::SingularSystem => write!(f, "least-squares system is singular (SVD failed)"),
            Self::ConvergenceFailure { max_err } => {
                write!(f, "convergence failure: max error={max_err:.6}")
            }
        }
    }
}

impl std::error::Error for NurbsFitError {}

/// Insert new knot values into a sorted knot vector.
fn insert_knots_at(knots: &[f64], new_ts: &[f64]) -> Vec<f64> {
    let mut result = knots.to_vec();
    for &t in new_ts {
        let pos = result.partition_point(|&k| k < t);
        result.insert(pos, t);
    }
    result
}

const DEFAULT_DEGREE: usize = 3;
const MAX_ITERATIONS: usize = 20;

/// Fit a NURBS curve to points with adaptive knot insertion until error <= tolerance.
pub fn fit_nurbs_curve(points: &[[f64; 3]], tolerance: f64) -> Result<NurbsCurve3D, NurbsFitError> {
    let n = points.len();
    if n < DEFAULT_DEGREE + 1 {
        return Err(NurbsFitError::InsufficientPoints {
            given: n,
            min_required: DEFAULT_DEGREE + 1,
        });
    }

    let total_len: f64 = points.windows(2).map(|w| dist(&w[1], &w[0])).sum();
    if total_len < 1e-14 {
        return Err(NurbsFitError::DegenerateInput(
            "all points are coincident".into(),
        ));
    }

    let params = chord_length_params(points);
    let max_cp = n / 2;
    let initial_n_cp = (DEFAULT_DEGREE + 1).max(n / 10);
    let mut knots = make_uniform_knots(DEFAULT_DEGREE, initial_n_cp);
    let mut prev_max_err = f64::MAX;

    for _iter in 0..MAX_ITERATIONS {
        let n_cp = knots.len() - DEFAULT_DEGREE - 1;
        let curve = fit_nurbs_least_squares(points, &params, DEFAULT_DEGREE, n_cp, &knots)?;

        let mut max_err = 0.0_f64;
        let mut worst: Vec<(f64, f64)> = Vec::new();
        for (i, pt) in points.iter().enumerate() {
            let eval = curve.evaluate(params[i]);
            let err = dist(&eval, pt);
            max_err = max_err.max(err);
            if err > tolerance {
                worst.push((err, params[i]));
            }
        }

        if max_err <= tolerance {
            return Ok(curve);
        }

        if max_err >= prev_max_err * 0.95 && _iter > 0 {
            return Err(NurbsFitError::ConvergenceFailure { max_err });
        }
        prev_max_err = max_err;

        worst.sort_by(|a, b| b.0.total_cmp(&a.0));
        let min_knot_dist = 1.0 / (knots.len() as f64 * 2.0);
        let mut insert_ts: Vec<f64> = Vec::new();
        for &(_, t) in &worst {
            if !(1e-6..=1.0 - 1e-6).contains(&t) {
                continue;
            }
            let too_close_to_knot = knots.iter().any(|&k| (k - t).abs() < min_knot_dist);
            let too_close_to_insert = insert_ts
                .iter()
                .any(|&s: &f64| (s - t).abs() < min_knot_dist);
            if !too_close_to_knot && !too_close_to_insert {
                insert_ts.push(t);
            }
            if insert_ts.len() >= 3 {
                break;
            }
        }
        if insert_ts.is_empty() {
            for w in knots.windows(2) {
                let gap = w[1] - w[0];
                if gap > min_knot_dist * 2.0 {
                    insert_ts.push((w[0] + w[1]) * 0.5);
                    if insert_ts.len() >= 3 {
                        break;
                    }
                }
            }
        }
        insert_ts.sort_by(|a, b| a.total_cmp(b));

        knots = insert_knots_at(&knots, &insert_ts);
        let new_n_cp = knots.len() - DEFAULT_DEGREE - 1;
        if new_n_cp > max_cp {
            return Err(NurbsFitError::ConvergenceFailure { max_err });
        }
    }

    Err(NurbsFitError::ConvergenceFailure {
        max_err: prev_max_err,
    })
}

/// Least-squares fit with a fixed knot vector and control point count.
fn fit_nurbs_least_squares(
    points: &[[f64; 3]],
    params: &[f64],
    degree: usize,
    n_cp: usize,
    knots: &[f64],
) -> Result<NurbsCurve3D, NurbsFitError> {
    let n = points.len();
    if n < degree + 1 {
        return Err(NurbsFitError::InsufficientPoints {
            given: n,
            min_required: degree + 1,
        });
    }

    let mut mat_n = DMatrix::<f64>::zeros(n, n_cp);
    for (row, &t) in params.iter().enumerate() {
        let basis = bspline_basis_row(knots, degree, n_cp, t);
        for (col, &val) in basis.iter().enumerate() {
            mat_n[(row, col)] = val;
        }
    }

    let mut rhs_x = DVector::<f64>::zeros(n);
    let mut rhs_y = DVector::<f64>::zeros(n);
    let mut rhs_z = DVector::<f64>::zeros(n);
    for (i, pt) in points.iter().enumerate() {
        rhs_x[i] = pt[0];
        rhs_y[i] = pt[1];
        rhs_z[i] = pt[2];
    }

    let svd = mat_n.svd(true, true);
    let cp_x = svd
        .solve(&rhs_x, 1e-12)
        .map_err(|_| NurbsFitError::SingularSystem)?;
    let cp_y = svd
        .solve(&rhs_y, 1e-12)
        .map_err(|_| NurbsFitError::SingularSystem)?;
    let cp_z = svd
        .solve(&rhs_z, 1e-12)
        .map_err(|_| NurbsFitError::SingularSystem)?;

    let control_points: Vec<[f64; 3]> = (0..n_cp).map(|i| [cp_x[i], cp_y[i], cp_z[i]]).collect();

    Ok(NurbsCurve3D {
        degree,
        control_points,
        weights: vec![1.0; n_cp],
        knots: knots.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bspline_basis_partition_of_unity() {
        let knots = vec![0.0, 0.0, 0.0, 0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0, 1.0, 1.0, 1.0];
        let degree = 3;
        let n_cp = 6;
        for &t in &[0.0, 0.1, 0.5, 0.9, 1.0] {
            let row = bspline_basis_row(&knots, degree, n_cp, t);
            let sum: f64 = row.iter().sum();
            assert!(
                (sum - 1.0).abs() < 1e-12,
                "partition of unity failed at t={t}: sum={sum}"
            );
        }
    }

    #[test]
    fn test_chord_length_params() {
        let pts = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [3.0, 0.0, 0.0]];
        let params = chord_length_params(&pts);
        assert_eq!(params.len(), 3);
        assert!((params[0] - 0.0).abs() < 1e-12);
        assert!((params[1] - 1.0 / 3.0).abs() < 1e-12);
        assert!((params[2] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_uniform_knots() {
        let knots = make_uniform_knots(3, 6);
        assert_eq!(knots.len(), 10);
        assert!(knots[0..4].iter().all(|&k| k == 0.0));
        assert!(knots[6..10].iter().all(|&k| (k - 1.0).abs() < 1e-12));
    }

    #[test]
    fn test_fit_straight_line() {
        let pts: Vec<[f64; 3]> = (0..20)
            .map(|i| {
                let t = i as f64 / 19.0;
                [t, 2.0 * t, 0.0]
            })
            .collect();
        let params = chord_length_params(&pts);
        let knots = make_uniform_knots(3, 4);
        let curve = fit_nurbs_least_squares(&pts, &params, 3, 4, &knots).unwrap();
        for (i, pt) in pts.iter().enumerate() {
            let eval = curve.evaluate(params[i]);
            let err = dist(&eval, pt);
            assert!(err < 1e-10, "point {i}: err={err}");
        }
    }

    #[test]
    fn test_insert_knots_at() {
        let base_knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        let new_knots = insert_knots_at(&base_knots, &[0.5]);
        assert_eq!(new_knots.len(), 9);
        assert!(new_knots.contains(&0.5));
        for w in new_knots.windows(2) {
            assert!(w[0] <= w[1]);
        }
    }

    #[test]
    fn test_fit_semicircle() {
        let n = 50;
        let pts: Vec<[f64; 3]> = (0..=n)
            .map(|i| {
                let theta = std::f64::consts::PI * i as f64 / n as f64;
                [theta.cos(), theta.sin(), 0.0]
            })
            .collect();
        let curve = fit_nurbs_curve(&pts, 1e-4).unwrap();
        assert!(
            curve.control_points.len() < pts.len() / 2,
            "control point count {} exceeds half of input",
            curve.control_points.len()
        );
        let params = chord_length_params(&pts);
        for (i, pt) in pts.iter().enumerate() {
            let eval = curve.evaluate(params[i]);
            let err = dist(&eval, pt);
            assert!(err < 1e-4, "point {i}: err={err}");
        }
    }

    #[test]
    fn test_fit_translation_covariance() {
        let n = 50;
        let pts: Vec<[f64; 3]> = (0..=n)
            .map(|i| {
                let theta = std::f64::consts::PI * i as f64 / n as f64;
                [theta.cos(), theta.sin(), 0.0]
            })
            .collect();
        let offset = [3.25, -1.75, 0.5];
        let translated_pts: Vec<[f64; 3]> = pts
            .iter()
            .map(|p| [p[0] + offset[0], p[1] + offset[1], p[2] + offset[2]])
            .collect();

        let curve = fit_nurbs_curve(&pts, 1e-4).unwrap();
        let translated_curve = fit_nurbs_curve(&translated_pts, 1e-4).unwrap();
        let params = chord_length_params(&pts);
        let translated_params = chord_length_params(&translated_pts);

        let sample_count = 16;
        let last = pts.len() - 1;
        for k in 0..sample_count {
            let i = k * last / (sample_count - 1);
            let eval = curve.evaluate(params[i]);
            let translated_eval = translated_curve.evaluate(translated_params[i]);
            let expected = [
                eval[0] + offset[0],
                eval[1] + offset[1],
                eval[2] + offset[2],
            ];
            let translated_err = dist(&translated_eval, &translated_pts[i]);
            let original_err = dist(&eval, &pts[i]);

            assert!(
                dist(&translated_eval, &expected) < 1e-4,
                "point {i}: translated_eval={translated_eval:?}, expected={expected:?}"
            );
            assert!(
                (translated_err - original_err).abs() < 1e-4,
                "point {i}: original_err={original_err}, translated_err={translated_err}"
            );
        }
    }

    #[test]
    fn test_fit_insufficient_points() {
        let pts = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
        let result = fit_nurbs_curve(&pts, 1e-4);
        assert!(result.is_err());
    }
}
