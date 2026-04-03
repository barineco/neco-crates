//! Sweep (profile along spine) B-Rep generation.
//!
//! Uses RMF (Rotation Minimizing Frame) via double reflection to compute
//! frames at each spine point, then positions the profile in 3D.

use crate::brep::{Curve3D, EdgeRef, Face, Shell, Surface};
use crate::vec3;

/// Local coordinate frame on the spine (position + orthonormal basis).
#[derive(Debug, Clone)]
pub struct Frame {
    pub origin: [f64; 3],
    /// Tangent (spine forward direction)
    pub tangent: [f64; 3],
    /// Normal (profile X axis)
    pub normal: [f64; 3],
    /// Binormal (profile Y axis)
    pub binormal: [f64; 3],
}

/// Correct RMF initial frame to align with profile XY coordinate system.
///
/// `initial_normal_binormal` produces an arbitrary frame orthogonal to the tangent,
/// but profile control points are defined in XY (`[1,0,0]`, `[0,1,0]`).
/// This computes the rotation delta and applies it to all frames so the result
/// matches the `[[1,0,0],[0,1,0],direction]` frame used by `shell_from_extrude_nurbs`.
fn correct_sweep_frames_to_profile_xy(frames: &mut [[[f64; 3]; 3]]) {
    if frames.is_empty() {
        return;
    }
    let n0 = frames[0][0];
    let b0 = frames[0][1];
    let t0 = frames[0][2];

    // Project X axis [1,0,0] onto the tangent-orthogonal plane to get target normal
    let x_axis = [1.0, 0.0, 0.0];
    let x_dot_t = vec3::dot(x_axis, t0);
    let proj_x = [
        x_axis[0] - x_dot_t * t0[0],
        x_axis[1] - x_dot_t * t0[1],
        x_axis[2] - x_dot_t * t0[2],
    ];
    let proj_x_len = vec3::length(proj_x);

    // Fall back to [0,1,0] if [1,0,0] is nearly parallel to tangent
    let target_n = if proj_x_len > 1e-6 {
        vec3::scale(proj_x, 1.0 / proj_x_len)
    } else {
        let y_axis = [0.0, 1.0, 0.0];
        let y_dot_t = vec3::dot(y_axis, t0);
        let proj_y = [
            y_axis[0] - y_dot_t * t0[0],
            y_axis[1] - y_dot_t * t0[1],
            y_axis[2] - y_dot_t * t0[2],
        ];
        let proj_y_len = vec3::length(proj_y);
        vec3::scale(proj_y, 1.0 / proj_y_len)
    };

    // Rotation angle: target_n = cos(theta)*n0 + sin(theta)*b0
    let cos_theta = vec3::dot(target_n, n0);
    let sin_theta = vec3::dot(target_n, b0);

    if (cos_theta - 1.0).abs() < 1e-12 && sin_theta.abs() < 1e-12 {
        return;
    }

    // Apply the same tangent-axis rotation to all frames
    for frame in frames.iter_mut() {
        let old_n = frame[0];
        let old_b = frame[1];
        frame[0] = [
            cos_theta * old_n[0] + sin_theta * old_b[0],
            cos_theta * old_n[1] + sin_theta * old_b[1],
            cos_theta * old_n[2] + sin_theta * old_b[2],
        ];
        frame[1] = [
            -sin_theta * old_n[0] + cos_theta * old_b[0],
            -sin_theta * old_n[1] + cos_theta * old_b[1],
            -sin_theta * old_n[2] + cos_theta * old_b[2],
        ];
    }
}

/// Compute RMF (Rotation Minimizing Frames) via double reflection.
///
/// Based on Wang et al. (2008) "Computation of Rotation Minimizing Frames".
pub fn compute_rmf(spine: &[[f64; 3]]) -> Vec<Frame> {
    let n = spine.len();
    if n < 2 {
        return vec![];
    }

    // Tangent vectors: central differences, one-sided at endpoints
    let tangents: Vec<[f64; 3]> = (0..n)
        .map(|i| {
            let t = if i == 0 {
                vec3::sub(spine[1], spine[0])
            } else if i == n - 1 {
                vec3::sub(spine[n - 1], spine[n - 2])
            } else {
                vec3::sub(spine[i + 1], spine[i - 1])
            };
            vec3::normalized(t)
        })
        .collect();

    let t0 = tangents[0];
    let (n0, b0) = initial_normal_binormal(t0);

    let mut frames = Vec::with_capacity(n);
    frames.push(Frame {
        origin: spine[0],
        tangent: t0,
        normal: n0,
        binormal: b0,
    });

    // Propagate frames via double reflection
    for i in 0..n - 1 {
        let prev = &frames[i];
        let ti = prev.tangent;
        let ri = prev.normal;
        let ti1 = tangents[i + 1];

        // First reflection across spine[i] -> spine[i+1]
        let v1 = vec3::sub(spine[i + 1], spine[i]);
        let c1 = vec3::dot(v1, v1);
        if c1 < 1e-30 {
            // Duplicate point: copy previous frame
            frames.push(Frame {
                origin: spine[i + 1],
                tangent: ti1,
                normal: prev.normal,
                binormal: prev.binormal,
            });
            continue;
        }

        let ri_l = vec3::sub(ri, vec3::scale(v1, 2.0 * vec3::dot(v1, ri) / c1));
        let ti_l = vec3::sub(ti, vec3::scale(v1, 2.0 * vec3::dot(v1, ti) / c1));

        // Second reflection across ti_L -> ti+1
        let v2 = vec3::sub(ti1, ti_l);
        let c2 = vec3::dot(v2, v2);
        let ri1 = if c2 < 1e-30 {
            ri_l
        } else {
            vec3::sub(ri_l, vec3::scale(v2, 2.0 * vec3::dot(v2, ri_l) / c2))
        };

        let ri1 = vec3::normalized(ri1);
        let bi1 = vec3::cross(ti1, ri1);

        frames.push(Frame {
            origin: spine[i + 1],
            tangent: ti1,
            normal: ri1,
            binormal: vec3::normalized(bi1),
        });
    }

    frames
}

/// Compute RMF for Bezier-decomposed spine, returning boundary frames per span.
///
/// Each span has `degree+1` control points with shared endpoints.
/// Returns `(span control points, [start frame, end frame])` per span.
pub fn compute_rmf_for_bezier_spans(
    spine_points: &[[f64; 3]],
    weights: &[f64],
    degree: usize,
    samples_per_span: usize,
) -> Vec<(Vec<[f64; 3]>, [Frame; 2])> {
    if spine_points.len() < degree + 1 {
        return vec![];
    }
    let n_spans = (spine_points.len() - 1) / degree;
    if n_spans == 0 {
        return vec![];
    }

    let mut all_samples = Vec::new();
    for i in 0..n_spans {
        let start = i * degree;
        let cps = &spine_points[start..start + degree + 1];
        let ws = &weights[start..start + degree + 1];
        let n_pts = if i == n_spans - 1 {
            samples_per_span + 1
        } else {
            samples_per_span
        };
        for j in 0..n_pts {
            let t = j as f64 / samples_per_span as f64;
            all_samples.push(crate::bezier::de_casteljau_rational_3d(cps, ws, t));
        }
    }

    let frames = compute_rmf(&all_samples);

    let mut result = Vec::with_capacity(n_spans);
    for i in 0..n_spans {
        let start = i * degree;
        let cps = spine_points[start..start + degree + 1].to_vec();
        let frame_start = frames[i * samples_per_span].clone();
        let frame_end = frames[((i + 1) * samples_per_span).min(frames.len() - 1)].clone();
        result.push((cps, [frame_start, frame_end]));
    }
    result
}

/// Check that the spine curvature radius exceeds the profile radius at every span.
///
/// Estimates curvature from circumradius of adjacent control point triples.
pub fn check_sweep_curvature(
    spine: &[[f64; 3]],
    degree: usize,
    max_profile_radius: f64,
) -> Result<(), String> {
    if degree == 0 {
        return Err("degree must be >= 1".into());
    }
    let n_spans = (spine.len().saturating_sub(1)) / degree;
    for i in 0..n_spans {
        let start = i * degree;
        let cps = &spine[start..start + degree + 1];
        let min_r = estimate_min_curvature_radius(cps);
        if min_r < max_profile_radius {
            return Err(format!(
                "sweep span {} curvature radius ({:.4}) is smaller than profile radius ({:.4}), causing self-intersection",
                i, min_r, max_profile_radius
            ));
        }
    }
    Ok(())
}

/// Estimate minimum curvature radius from control points (min circumradius of adjacent triples).
fn estimate_min_curvature_radius(cps: &[[f64; 3]]) -> f64 {
    let mut min_r = f64::MAX;
    for i in 0..cps.len().saturating_sub(2) {
        let a = vec3::sub(cps[i + 1], cps[i]);
        let b = vec3::sub(cps[i + 2], cps[i + 1]);
        let cr = vec3::cross(a, b);
        let cross_len = vec3::length(cr);
        if cross_len > 1e-12 {
            let a_len = vec3::length(a);
            let b_len = vec3::length(b);
            let c_len = vec3::length(vec3::sub(cps[i + 2], cps[i]));
            let r = a_len * b_len * c_len / (2.0 * cross_len);
            min_r = min_r.min(r);
        }
    }
    min_r
}

/// Generate an initial normal/binormal orthogonal to the tangent.
fn initial_normal_binormal(tangent: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let ax = tangent[0].abs();
    let ay = tangent[1].abs();
    let az = tangent[2].abs();
    let up = if ax <= ay && ax <= az {
        [1.0, 0.0, 0.0]
    } else if ay <= az {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    };
    let normal = vec3::normalized(vec3::cross(tangent, up));
    let binormal = vec3::cross(tangent, normal);
    (normal, vec3::normalized(binormal))
}

/// Sweep a NurbsRegion profile along a spine to produce a Shell.
///
/// The spine is expected as Bezier-decomposed control points.
/// Generates one SurfaceOfSweep face per spine span.
pub fn shell_from_sweep(
    profile: &neco_nurbs::NurbsRegion,
    spine: &[[f64; 3]],
) -> Result<Shell, String> {
    if spine.len() < 2 {
        return Err("sweep: spine requires at least 2 points".into());
    }

    let bezier_spans: Vec<_> = profile
        .outer
        .iter()
        .flat_map(|c| c.to_bezier_spans())
        .collect();
    if bezier_spans.is_empty() {
        return Err("profile has no Bezier spans".into());
    }

    // Infer spine degree: 2 pts -> 1, otherwise largest d where (len-1) % d == 0
    let degree = if spine.len() == 2 {
        1
    } else {
        let mut d = 3usize;
        while d >= 1 {
            if (spine.len() - 1) % d == 0 {
                break;
            }
            d -= 1;
        }
        d
    };

    let max_profile_radius = profile
        .outer
        .iter()
        .flat_map(|c| c.control_points.iter())
        .map(|cp| (cp[0] * cp[0] + cp[1] * cp[1]).sqrt())
        .fold(0.0_f64, f64::max);
    check_sweep_curvature(spine, degree, max_profile_radius)?;

    let spine_weights = vec![1.0; spine.len()];
    let rmf_spans = compute_rmf_for_bezier_spans(spine, &spine_weights, degree, 32);

    if rmf_spans.is_empty() {
        return Err("sweep: RMF span computation returned empty".into());
    }

    let mut shell = Shell::new();

    let fwd = |eid: usize| EdgeRef {
        edge_id: eid,
        forward: true,
    };

    // Merge all profile spans into a single side face
    let profile_degree = u32::try_from(bezier_spans[0].degree).expect("degree fits in u32");
    let n_profile_spans = u32::try_from(bezier_spans.len()).expect("span count fits in u32");
    let mut all_profile_cps: Vec<[f64; 2]> = Vec::new();
    let mut all_profile_weights: Vec<f64> = Vec::new();
    for span in &bezier_spans {
        for cp in &span.control_points {
            all_profile_cps.push(*cp);
        }
        all_profile_weights.extend_from_slice(&span.weights);
    }

    // Merge spine span control points and frames
    let mut all_spine_cps: Vec<[f64; 3]> = Vec::new();
    let mut all_frames: Vec<[[f64; 3]; 3]> = Vec::new();
    for (i, (span_cps, [frame0, frame1])) in rmf_spans.iter().enumerate() {
        let start = if i == 0 { 0 } else { 1 };
        for &cp in span_cps.iter().skip(start) {
            all_spine_cps.push(cp);
        }
        if i == 0 {
            all_frames.push([frame0.normal, frame0.binormal, frame0.tangent]);
        }
        all_frames.push([frame1.normal, frame1.binormal, frame1.tangent]);
    }
    let all_spine_weights = vec![1.0; all_spine_cps.len()];

    // Align RMF initial frame with profile XY coordinate system
    correct_sweep_frames_to_profile_xy(&mut all_frames);

    let surface = Surface::SurfaceOfSweep {
        spine_control_points: all_spine_cps,
        spine_weights: all_spine_weights,
        spine_degree: u32::try_from(degree).expect("degree fits in u32"),
        profile_control_points: all_profile_cps,
        profile_weights: all_profile_weights,
        profile_degree,
        n_profile_spans,
        frames: all_frames,
    };

    // Evaluate corner vertices for the face edge loop
    let p00 = surface.evaluate(0.0, 0.0);
    let p10 = surface.evaluate(1.0, 0.0);
    let p11 = surface.evaluate(1.0, 1.0);
    let p01 = surface.evaluate(0.0, 1.0);

    let v0 = shell.add_vertex(p00);
    let v1 = shell.add_vertex(p10);
    let v2 = shell.add_vertex(p11);
    let v3 = shell.add_vertex(p01);

    let e0 = shell.add_edge(
        v0,
        v1,
        Curve3D::Line {
            start: p00,
            end: p10,
        },
    );
    let e1 = shell.add_edge(
        v1,
        v2,
        Curve3D::Line {
            start: p10,
            end: p11,
        },
    );
    let e2 = shell.add_edge(
        v2,
        v3,
        Curve3D::Line {
            start: p11,
            end: p01,
        },
    );
    let e3 = shell.add_edge(
        v3,
        v0,
        Curve3D::Line {
            start: p01,
            end: p00,
        },
    );

    shell.faces.push(Face {
        loop_edges: vec![fwd(e0), fwd(e1), fwd(e2), fwd(e3)],
        surface,
        orientation_reversed: false,
    });

    Ok(shell)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rmf_straight_spine() {
        let spine: Vec<[f64; 3]> = (0..5).map(|i| [0.0, 0.0, i as f64 * 0.25]).collect();
        let frames = compute_rmf(&spine);
        assert_eq!(frames.len(), 5);
        for f in &frames {
            assert!((f.tangent[2] - 1.0).abs() < 1e-10);
            assert!(f.normal[2].abs() < 1e-10);
            assert!(f.binormal[2].abs() < 1e-10);
        }
    }

    #[test]
    fn sweep_frame_correction_x_tangent() {
        let mut frames = vec![[[0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]]];
        correct_sweep_frames_to_profile_xy(&mut frames);
        let n = frames[0][0];
        let b = frames[0][1];
        assert!(
            (n[0]).abs() < 1e-10 && (n[1] - 1.0).abs() < 1e-10 && (n[2]).abs() < 1e-10,
            "normal should be [0,1,0]: {:?}",
            n
        );
        assert!(
            (b[0]).abs() < 1e-10 && (b[1]).abs() < 1e-10 && (b[2] - 1.0).abs() < 1e-10,
            "binormal should be [0,0,1]: {:?}",
            b
        );
        let dot_nb = n[0] * b[0] + n[1] * b[1] + n[2] * b[2];
        assert!(
            dot_nb.abs() < 1e-10,
            "normal and binormal must be orthogonal: {dot_nb}"
        );
    }
}
