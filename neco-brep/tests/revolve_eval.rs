use neco_brep::vec3;
use neco_brep::Surface;
use std::f64::consts::{FRAC_PI_2, TAU};

/// Linear profile (degree 1): `r = 1 -> 1`, `z = 0 -> 2`.
fn linear_revolve() -> Surface {
    Surface::SurfaceOfRevolution {
        center: [0.0, 0.0, 0.0],
        axis: [0.0, 0.0, 1.0],
        frame_u: [1.0, 0.0, 0.0],
        frame_v: [0.0, 1.0, 0.0],
        profile_control_points: vec![[1.0, 0.0], [1.0, 2.0]],
        profile_weights: vec![1.0, 1.0],
        profile_degree: 1,
        n_profile_spans: 1,
        theta_start: 0.0,
        theta_range: TAU,
    }
}

/// Quarter-circle profile (degree 2 rational): `(r, z) = (1, 0) -> (0, 1)`.
fn semicircle_revolve() -> Surface {
    let w = std::f64::consts::FRAC_1_SQRT_2;
    Surface::SurfaceOfRevolution {
        center: [0.0, 0.0, 0.0],
        axis: [0.0, 0.0, 1.0],
        frame_u: [1.0, 0.0, 0.0],
        frame_v: [0.0, 1.0, 0.0],
        profile_control_points: vec![[1.0, 0.0], [0.0, 0.0], [0.0, 1.0]],
        profile_weights: vec![1.0, w, 1.0],
        profile_degree: 2,
        n_profile_spans: 1,
        theta_start: 0.0,
        theta_range: TAU,
    }
}

#[test]
fn linear_revolve_evaluate_theta0_t0() {
    let s = linear_revolve();
    let p = s.evaluate(0.0, 0.0);
    assert!((p[0] - 1.0).abs() < 1e-10, "x={}", p[0]);
    assert!(p[1].abs() < 1e-10, "y={}", p[1]);
    assert!(p[2].abs() < 1e-10, "z={}", p[2]);
}

#[test]
fn linear_revolve_evaluate_theta_pi2_t0() {
    let s = linear_revolve();
    let p = s.evaluate(FRAC_PI_2, 0.0);
    assert!(p[0].abs() < 1e-10, "x={}", p[0]);
    assert!((p[1] - 1.0).abs() < 1e-10, "y={}", p[1]);
    assert!(p[2].abs() < 1e-10, "z={}", p[2]);
}

#[test]
fn linear_revolve_evaluate_theta0_t05() {
    let s = linear_revolve();
    let p = s.evaluate(0.0, 0.5);
    assert!((p[0] - 1.0).abs() < 1e-10, "x={}", p[0]);
    assert!(p[1].abs() < 1e-10, "y={}", p[1]);
    assert!((p[2] - 1.0).abs() < 1e-10, "z={}", p[2]);
}

#[test]
fn semicircle_revolve_evaluate_north_pole() {
    let s = semicircle_revolve();
    let p = s.evaluate(0.0, 1.0);
    assert!(p[0].abs() < 1e-10, "x={}", p[0]);
    assert!(p[1].abs() < 1e-10, "y={}", p[1]);
    assert!((p[2] - 1.0).abs() < 1e-10, "z={}", p[2]);
}

#[test]
fn linear_revolve_normal_theta0_t0() {
    let s = linear_revolve();
    let n = s.normal_at(0.0, 0.0);
    let len = vec3::length(n);
    assert!(
        (len - 1.0).abs() < 1e-6,
        "normal is not unit length: len={}",
        len
    );
    assert!(n[0].abs() > 0.5, "normal x component is too small: {:?}", n);
}

#[test]
fn semicircle_revolve_normal_equator() {
    let s = semicircle_revolve();
    let n_mid = s.normal_at(0.0, 0.5);
    let len = vec3::length(n_mid);
    assert!(
        (len - 1.0).abs() < 1e-6,
        "normal is not unit length: len={}",
        len
    );
    assert!(
        n_mid[0].abs() > 0.1,
        "normal x component is too small: {:?}",
        n_mid
    );

    let n0 = s.normal_at(0.0, 0.0);
    assert!(
        n0[2].abs() > 0.9,
        "normal at t=0 should align with the axis direction: {:?}",
        n0
    );
}

#[test]
fn param_range_is_tau_01() {
    let s = linear_revolve();
    let (u0, u1, v0, v1) = s.param_range();
    assert!((u0 - 0.0).abs() < 1e-10);
    assert!((u1 - TAU).abs() < 1e-10);
    assert!((v0 - 0.0).abs() < 1e-10);
    assert!((v1 - 1.0).abs() < 1e-10);
}
