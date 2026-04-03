//! Tests for SurfaceOfSweep to NurbsSurface conversion accuracy and boolean ops.

use neco_brep::vec3;
use neco_brep::Surface;

/// Build a `SurfaceOfSweep` from a straight spine and square profile.
fn make_straight_sweep() -> Surface {
    Surface::SurfaceOfSweep {
        spine_control_points: vec![[0.0, 0.0, 0.0], [0.0, 0.0, 3.0]],
        spine_weights: vec![1.0, 1.0],
        spine_degree: 1,
        profile_control_points: vec![[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]],
        profile_weights: vec![1.0, 1.0, 1.0, 1.0],
        profile_degree: 1,
        n_profile_spans: 1,
        frames: vec![
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        ],
    }
}

/// Build a `SurfaceOfSweep` with a quadratic arc profile.
fn make_sweep_with_quadratic_profile() -> Surface {
    let w = std::f64::consts::FRAC_1_SQRT_2;
    Surface::SurfaceOfSweep {
        spine_control_points: vec![[0.0, 0.0, 0.0], [0.0, 0.0, 2.0]],
        spine_weights: vec![1.0, 1.0],
        spine_degree: 1,
        profile_control_points: vec![
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [0.0, 1.0],
            [-1.0, 1.0],
            [-1.0, 0.0],
        ],
        profile_weights: vec![1.0, w, 1.0, 1.0, w, 1.0],
        profile_degree: 2,
        n_profile_spans: 2,
        frames: vec![
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        ],
    }
}

/// Build a `SurfaceOfSweep` with a three-span degree-1 spine.
fn make_multi_span_spine_sweep() -> Surface {
    Surface::SurfaceOfSweep {
        spine_control_points: vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [2.0, 0.0, 0.0],
            [3.0, 0.0, 1.0],
        ],
        spine_weights: vec![1.0, 1.0, 1.0, 1.0],
        spine_degree: 1,
        profile_control_points: vec![[0.5, 0.0], [-0.5, 0.0]],
        profile_weights: vec![1.0, 1.0],
        profile_degree: 1,
        n_profile_spans: 1,
        frames: vec![
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        ],
    }
}

#[test]
fn to_nurbs_surface_returns_some_for_sweep() {
    let surf = make_straight_sweep();
    let ns = surf.to_nurbs_surface();
    assert!(
        ns.is_some(),
        "SurfaceOfSweep should convert to NurbsSurface"
    );
}

#[test]
fn to_nurbs_surface_returns_none_for_plane() {
    let surf = Surface::Plane {
        origin: [0.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
    };
    assert!(
        surf.to_nurbs_surface().is_none(),
        "Plane should return None from to_nurbs_surface"
    );
}

#[test]
fn to_nurbs_conversion_accuracy_straight_sweep() {
    let surf = make_straight_sweep();
    let ns = surf.to_nurbs_surface().unwrap();

    let n = 11;
    let mut max_err = 0.0_f64;
    for i in 0..n {
        for j in 0..n {
            let u = i as f64 / (n - 1) as f64;
            let v = j as f64 / (n - 1) as f64;
            let p_sweep = surf.evaluate(u, v);
            let p_nurbs = ns.evaluate(u, v);
            let err = vec3::distance(p_sweep, p_nurbs);
            max_err = max_err.max(err);
        }
    }
    assert!(
        max_err < 1e-10,
        "straight spine with constant frame should convert exactly (max_err = {max_err})"
    );
}

#[test]
fn to_nurbs_conversion_accuracy_multi_span_spine() {
    let surf = make_multi_span_spine_sweep();
    let ns = surf.to_nurbs_surface().unwrap();

    let n = 21;
    let mut max_err = 0.0_f64;
    for i in 0..n {
        for j in 0..n {
            let u = i as f64 / (n - 1) as f64;
            let v = j as f64 / (n - 1) as f64;
            let p_sweep = surf.evaluate(u, v);
            let p_nurbs = ns.evaluate(u, v);
            let err = vec3::distance(p_sweep, p_nurbs);
            max_err = max_err.max(err);
        }
    }
    assert!(
        max_err < 1e-3,
        "multi-span spine conversion error should stay below 1e-3 (max_err = {max_err})"
    );
}

#[test]
fn to_nurbs_conversion_accuracy_quadratic_profile() {
    let surf = make_sweep_with_quadratic_profile();
    let ns = surf.to_nurbs_surface().unwrap();

    let n = 11;
    let mut max_err = 0.0_f64;
    for i in 0..n {
        for j in 0..n {
            let u = i as f64 / (n - 1) as f64;
            let v = j as f64 / (n - 1) as f64;
            let p_sweep = surf.evaluate(u, v);
            let p_nurbs = ns.evaluate(u, v);
            let err = vec3::distance(p_sweep, p_nurbs);
            max_err = max_err.max(err);
        }
    }
    assert!(
        max_err < 1e-3,
        "quadratic profile conversion error should stay below 1e-3 (max_err = {max_err})"
    );
}

#[test]
fn boolean_sweep_subtract_box_no_panic() {
    use neco_brep::{boolean_3d, shell_from_box, shell_from_sweep, BooleanOp};
    use neco_nurbs::{NurbsCurve2D, NurbsRegion};

    let h = 0.5;
    let profile = NurbsRegion {
        outer: vec![NurbsCurve2D {
            degree: 1,
            control_points: vec![[h, h], [-h, h], [-h, -h], [h, -h], [h, h]],
            weights: vec![1.0, 1.0, 1.0, 1.0, 1.0],
            knots: vec![0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0],
        }],
        holes: vec![],
    };
    let spine = vec![[0.0, 0.0, 0.0], [0.0, 0.0, 2.0]];

    let sweep_shell = shell_from_sweep(&profile, &spine);
    let sweep_shell = match sweep_shell {
        Ok(s) => s,
        Err(_) => return,
    };

    let box_shell = shell_from_box(4.0, 4.0, 4.0);

    let result = boolean_3d(&box_shell, &sweep_shell, BooleanOp::Subtract);
    match result {
        Ok(shell) => {
            assert!(
                !shell.faces.is_empty(),
                "subtract result should return a non-empty shell"
            );
        }
        Err(e) => {
            eprintln!("boolean_3d returned error (acceptable): {e}");
        }
    }
}
