//! Coverage tests for degenerate analytic-surface intersection pairs.
//!
//! These cases exercise coaxial, parallel, tangent, separated, cone half-angle,
//! and self-intersecting torus configurations to ensure
//! `face_face_intersection` does not panic and returns an empty vector when no
//! intersection exists.

use neco_brep::boolean3d::intersect3d::face_face_intersection;
use neco_brep::boolean3d::BooleanEvent;
use neco_brep::vec3;
use neco_brep::{
    apply_transform, shell_from_ellipsoid, shell_from_sphere, shell_from_torus, Curve3D, EdgeRef,
    Face, Shell, Surface,
};
use std::f64::consts::{FRAC_PI_4, PI, TAU};

// ============================================================
// Helpers: build minimal one-face shells.
// ============================================================

/// Build a minimal shell containing one planar face.
fn plane_shell(origin: [f64; 3], normal: [f64; 3]) -> Shell {
    let surface = Surface::Plane { origin, normal };
    let n = vec3::normalized(normal);
    let u = {
        let up = if n[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        vec3::normalized(vec3::cross(n, up))
    };
    let v = vec3::cross(n, u);

    let mut shell = Shell::new();
    let size = 100.0;
    let corners = [
        vec3::add(
            vec3::add(origin, vec3::scale(u, -size)),
            vec3::scale(v, -size),
        ),
        vec3::add(
            vec3::add(origin, vec3::scale(u, size)),
            vec3::scale(v, -size),
        ),
        vec3::add(
            vec3::add(origin, vec3::scale(u, size)),
            vec3::scale(v, size),
        ),
        vec3::add(
            vec3::add(origin, vec3::scale(u, -size)),
            vec3::scale(v, size),
        ),
    ];
    let vi: Vec<usize> = corners.iter().map(|c| shell.add_vertex(*c)).collect();
    let mut edges = Vec::new();
    for i in 0..4 {
        let j = (i + 1) % 4;
        let eid = shell.add_edge(
            vi[i],
            vi[j],
            Curve3D::Line {
                start: corners[i],
                end: corners[j],
            },
        );
        edges.push(EdgeRef {
            edge_id: eid,
            forward: true,
        });
    }
    shell.faces.push(Face {
        loop_edges: edges,
        surface,
        orientation_reversed: false,
    });
    shell
}

/// Build a minimal shell containing one cylindrical face.
fn cylinder_shell(origin: [f64; 3], axis: [f64; 3], radius: f64) -> Shell {
    let surface = Surface::Cylinder {
        origin,
        axis,
        radius,
    };
    let axis_len = vec3::length(axis);
    let axis_n = vec3::normalized(axis);
    let u_dir = {
        let up = if axis_n[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        vec3::normalized(vec3::cross(axis_n, up))
    };
    let v_dir = vec3::cross(axis_n, u_dir);

    let mut shell = Shell::new();
    let n_circ = 4;
    let mut bottom_vi = Vec::new();
    let mut top_vi = Vec::new();
    for i in 0..n_circ {
        let theta = TAU * i as f64 / n_circ as f64;
        let r_vec = vec3::add(
            vec3::scale(u_dir, radius * theta.cos()),
            vec3::scale(v_dir, radius * theta.sin()),
        );
        let bp = vec3::add(origin, r_vec);
        let tp = vec3::add(vec3::add(origin, vec3::scale(axis_n, axis_len)), r_vec);
        bottom_vi.push(shell.add_vertex(bp));
        top_vi.push(shell.add_vertex(tp));
    }
    let mut edges = Vec::new();
    for i in 0..n_circ {
        let j = (i + 1) % n_circ;
        let eid = shell.add_edge(
            bottom_vi[i],
            bottom_vi[j],
            Curve3D::Line {
                start: shell.vertices[bottom_vi[i]],
                end: shell.vertices[bottom_vi[j]],
            },
        );
        edges.push(EdgeRef {
            edge_id: eid,
            forward: true,
        });
    }
    shell.faces.push(Face {
        loop_edges: edges,
        surface,
        orientation_reversed: false,
    });
    shell
}

/// Build a minimal shell containing one conical face.
fn cone_shell(origin: [f64; 3], axis: [f64; 3], half_angle: f64) -> Shell {
    let surface = Surface::Cone {
        origin,
        axis,
        half_angle,
    };
    let mut shell = Shell::new();
    let vi = shell.add_vertex(origin);
    let tip = vec3::add(origin, axis);
    let vi2 = shell.add_vertex(tip);
    let eid = shell.add_edge(
        vi,
        vi2,
        Curve3D::Line {
            start: origin,
            end: tip,
        },
    );
    shell.faces.push(Face {
        loop_edges: vec![EdgeRef {
            edge_id: eid,
            forward: true,
        }],
        surface,
        orientation_reversed: false,
    });
    shell
}

/// Build an ellipsoid shell.
fn ellipsoid_shell_at(center: [f64; 3], rx: f64, ry: f64, rz: f64) -> Shell {
    let mut s = shell_from_ellipsoid(rx, ry, rz);
    if center[0].abs() > 1e-15 || center[1].abs() > 1e-15 || center[2].abs() > 1e-15 {
        let m = [
            [1.0, 0.0, 0.0, center[0]],
            [0.0, 1.0, 0.0, center[1]],
            [0.0, 0.0, 1.0, center[2]],
            [0.0, 0.0, 0.0, 1.0],
        ];
        s = apply_transform(&s, &m);
    }
    s
}

/// sphere shell at a given center
fn sphere_shell_at(center: [f64; 3], radius: f64) -> Shell {
    let mut s = shell_from_sphere(radius);
    if center[0].abs() > 1e-15 || center[1].abs() > 1e-15 || center[2].abs() > 1e-15 {
        let m = [
            [1.0, 0.0, 0.0, center[0]],
            [0.0, 1.0, 0.0, center[1]],
            [0.0, 0.0, 1.0, center[2]],
            [0.0, 0.0, 0.0, 1.0],
        ];
        s = apply_transform(&s, &m);
    }
    s
}

/// torus shell at a given center with Y axis
fn torus_shell_at(center: [f64; 3], major: f64, minor: f64) -> Shell {
    let mut s = shell_from_torus(major, minor);
    if center[0].abs() > 1e-15 || center[1].abs() > 1e-15 || center[2].abs() > 1e-15 {
        let m = [
            [1.0, 0.0, 0.0, center[0]],
            [0.0, 1.0, 0.0, center[1]],
            [0.0, 0.0, 1.0, center[2]],
            [0.0, 0.0, 0.0, 1.0],
        ];
        s = apply_transform(&s, &m);
    }
    s
}

/// Run `face_face_intersection` on two single-face shells and return the curves.
fn intersect_faces(shell_a: &Shell, shell_b: &Shell) -> Vec<Curve3D> {
    let mut events: Vec<BooleanEvent> = Vec::new();
    let mut all_curves = Vec::new();
    for fa in &shell_a.faces {
        for fb in &shell_b.faces {
            let curves = face_face_intersection(fa, shell_a, fb, shell_b, &mut events);
            all_curves.extend(curves);
        }
    }
    all_curves
}

// ============================================================
// B-1: shared degenerate cases (coaxial, parallel, tangent, separated)
// ============================================================

mod coaxial {
    use super::*;

    #[test]
    fn cylinder_cylinder_coaxial() {
        let a = cylinder_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], 1.0);
        let b = cylinder_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], 1.0);
        let _ = intersect_faces(&a, &b);
    }

    #[test]
    fn sphere_sphere_concentric() {
        let a = sphere_shell_at([0.0, 0.0, 0.0], 2.0);
        let b = sphere_shell_at([0.0, 0.0, 0.0], 1.0);
        let curves = intersect_faces(&a, &b);
        assert!(
            curves.is_empty(),
            "concentric spheres should not intersect, got {} curves",
            curves.len()
        );
    }

    #[test]
    fn sphere_sphere_identical() {
        let a = sphere_shell_at([0.0, 0.0, 0.0], 1.0);
        let b = sphere_shell_at([0.0, 0.0, 0.0], 1.0);
        let _ = intersect_faces(&a, &b);
    }

    #[test]
    fn cylinder_cylinder_coaxial_different_radius() {
        let a = cylinder_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], 1.0);
        let b = cylinder_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], 0.5);
        let curves = intersect_faces(&a, &b);
        assert!(
            curves.is_empty(),
            "coaxial cylinders with different radii should not intersect, got {} curves",
            curves.len()
        );
    }

    #[test]
    fn cone_cone_coaxial() {
        let a = cone_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], FRAC_PI_4);
        let b = cone_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], FRAC_PI_4);
        let _ = intersect_faces(&a, &b);
    }
}

mod parallel {
    use super::*;

    #[test]
    fn cylinder_cylinder_parallel_no_touch() {
        let a = cylinder_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], 1.0);
        let b = cylinder_shell([5.0, 0.0, 0.0], [0.0, 0.0, 5.0], 1.0);
        let curves = intersect_faces(&a, &b);
        assert!(
            curves.is_empty(),
            "separated parallel cylinders should not intersect, got {} curves",
            curves.len()
        );
    }

    #[test]
    fn cylinder_cylinder_parallel_equal_radius_touching() {
        let a = cylinder_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], 1.0);
        let b = cylinder_shell([2.0, 0.0, 0.0], [0.0, 0.0, 5.0], 1.0);
        let _ = intersect_faces(&a, &b);
    }

    #[test]
    fn plane_plane_parallel() {
        let a = plane_shell([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        let b = plane_shell([0.0, 0.0, 5.0], [0.0, 0.0, 1.0]);
        let curves = intersect_faces(&a, &b);
        assert!(
            curves.is_empty(),
            "parallel planes should not intersect, got {} curves",
            curves.len()
        );
    }

    #[test]
    fn cone_cone_parallel_axes() {
        let a = cone_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], FRAC_PI_4);
        let b = cone_shell([10.0, 0.0, 0.0], [0.0, 0.0, 5.0], FRAC_PI_4);
        let curves = intersect_faces(&a, &b);
        assert!(
            curves.is_empty(),
            "separated parallel cones should not intersect, got {} curves",
            curves.len()
        );
    }
}

mod tangent {
    use super::*;

    #[test]
    fn sphere_sphere_tangent_external() {
        let a = sphere_shell_at([0.0, 0.0, 0.0], 1.0);
        let b = sphere_shell_at([2.0, 0.0, 0.0], 1.0);
        let _ = intersect_faces(&a, &b);
    }

    #[test]
    fn sphere_sphere_tangent_internal() {
        let a = sphere_shell_at([0.0, 0.0, 0.0], 2.0);
        let b = sphere_shell_at([1.0, 0.0, 0.0], 1.0);
        let _ = intersect_faces(&a, &b);
    }

    #[test]
    fn sphere_plane_tangent() {
        let s = sphere_shell_at([0.0, 1.0, 0.0], 1.0);
        let p = plane_shell([0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let _ = intersect_faces(&s, &p);
    }

    #[test]
    fn cylinder_plane_tangent() {
        let c = cylinder_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], 1.0);
        let p = plane_shell([1.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
        let _ = intersect_faces(&c, &p);
    }

    #[test]
    fn sphere_cylinder_tangent() {
        let s = sphere_shell_at([2.0, 0.0, 0.0], 1.0);
        let c = cylinder_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], 1.0);
        let _ = intersect_faces(&s, &c);
    }

    #[test]
    fn ellipsoid_plane_tangent() {
        let e = ellipsoid_shell_at([0.0, 1.0, 0.0], 2.0, 1.0, 1.5);
        let p = plane_shell([0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let _ = intersect_faces(&e, &p);
    }
}

mod separated {
    use super::*;

    #[test]
    fn sphere_sphere_separated() {
        let a = sphere_shell_at([0.0, 0.0, 0.0], 1.0);
        let b = sphere_shell_at([10.0, 0.0, 0.0], 1.0);
        let curves = intersect_faces(&a, &b);
        assert!(
            curves.is_empty(),
            "separated spheres should not intersect, got {} curves",
            curves.len()
        );
    }

    #[test]
    fn sphere_plane_separated() {
        let s = sphere_shell_at([0.0, 10.0, 0.0], 1.0);
        let p = plane_shell([0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let curves = intersect_faces(&s, &p);
        assert!(
            curves.is_empty(),
            "separated sphere/plane pair should not intersect, got {} curves",
            curves.len()
        );
    }

    #[test]
    fn cylinder_sphere_separated() {
        let c = cylinder_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], 1.0);
        let s = sphere_shell_at([10.0, 0.0, 0.0], 1.0);
        let curves = intersect_faces(&c, &s);
        assert!(
            curves.is_empty(),
            "separated cylinder/sphere pair should not intersect, got {} curves",
            curves.len()
        );
    }

    #[test]
    fn ellipsoid_ellipsoid_separated() {
        let a = ellipsoid_shell_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
        let b = ellipsoid_shell_at([10.0, 0.0, 0.0], 1.0, 1.0, 1.0);
        let curves = intersect_faces(&a, &b);
        assert!(
            curves.is_empty(),
            "separated ellipsoids should not intersect, got {} curves",
            curves.len()
        );
    }
}

// ============================================================
// B-2: cone-specific half-angle boundary values
// ============================================================

mod cone_degenerate {
    use super::*;

    const NEAR_ZERO: f64 = 1e-6;
    const NEAR_PI_2: f64 = PI / 2.0 - 1e-6;

    #[test]
    fn plane_cone_half_angle_near_zero() {
        let p = plane_shell([0.0, 0.0, 2.0], [0.0, 0.0, 1.0]);
        let c = cone_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], NEAR_ZERO);
        let _ = intersect_faces(&p, &c);
    }

    #[test]
    fn plane_cone_half_angle_near_pi_2() {
        let p = plane_shell([0.0, 0.0, 2.0], [0.0, 0.0, 1.0]);
        let c = cone_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], NEAR_PI_2);
        let _ = intersect_faces(&p, &c);
    }

    #[test]
    fn sphere_cone_half_angle_near_zero() {
        let s = sphere_shell_at([0.0, 0.0, 2.0], 1.0);
        let c = cone_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], NEAR_ZERO);
        let _ = intersect_faces(&s, &c);
    }

    #[test]
    fn sphere_cone_half_angle_near_pi_2() {
        let s = sphere_shell_at([0.0, 0.0, 2.0], 1.0);
        let c = cone_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], NEAR_PI_2);
        let _ = intersect_faces(&s, &c);
    }

    #[test]
    fn cylinder_cone_half_angle_near_zero() {
        let cyl = cylinder_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], 1.0);
        let c = cone_shell([1.0, 0.0, 0.0], [0.0, 0.0, 5.0], NEAR_ZERO);
        let _ = intersect_faces(&cyl, &c);
    }

    #[test]
    fn cylinder_cone_half_angle_near_pi_2() {
        let cyl = cylinder_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], 1.0);
        let c = cone_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], NEAR_PI_2);
        let _ = intersect_faces(&cyl, &c);
    }

    #[test]
    fn cone_cone_both_near_zero() {
        let a = cone_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], NEAR_ZERO);
        let b = cone_shell([1.0, 0.0, 0.0], [0.0, 0.0, 5.0], NEAR_ZERO);
        let _ = intersect_faces(&a, &b);
    }

    #[test]
    fn cone_cone_both_near_pi_2() {
        let a = cone_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], NEAR_PI_2);
        let b = cone_shell([0.0, 0.0, 0.5], [0.0, 0.0, 5.0], NEAR_PI_2);
        let _ = intersect_faces(&a, &b);
    }

    #[test]
    fn ellipsoid_cone_half_angle_near_zero() {
        let e = ellipsoid_shell_at([0.0, 0.0, 2.0], 1.5, 1.0, 1.0);
        let c = cone_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], NEAR_ZERO);
        let _ = intersect_faces(&e, &c);
    }

    #[test]
    fn ellipsoid_cone_half_angle_near_pi_2() {
        let e = ellipsoid_shell_at([0.0, 0.0, 2.0], 1.5, 1.0, 1.0);
        let c = cone_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], NEAR_PI_2);
        let _ = intersect_faces(&e, &c);
    }
}

// ============================================================
// B-3: torus-specific cases (self-intersecting when major == minor)
// ============================================================

mod torus_degenerate {
    use super::*;

    fn self_intersecting_torus() -> Shell {
        // `major_radius == minor_radius` creates a self-intersecting torus.
        // In neco-brep, `shell_from_torus(major, minor)` is centered at the
        // origin around the Z axis. The original source test used a Y-axis
        // torus, but this degenerate case only needs to avoid panicking.
        shell_from_torus(1.0, 1.0)
    }

    #[test]
    fn sphere_self_intersecting_torus() {
        let s = sphere_shell_at([1.0, 0.0, 0.0], 0.5);
        let t = self_intersecting_torus();
        let _ = intersect_faces(&s, &t);
    }

    #[test]
    fn cylinder_self_intersecting_torus() {
        let c = cylinder_shell([0.0, -2.0, 0.0], [0.0, 4.0, 0.0], 0.5);
        let t = self_intersecting_torus();
        let _ = intersect_faces(&c, &t);
    }

    #[test]
    fn plane_self_intersecting_torus() {
        let p = plane_shell([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        let t = self_intersecting_torus();
        let _ = intersect_faces(&p, &t);
    }

    #[test]
    fn plane_torus_axis_parallel() {
        let p = plane_shell([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
        let t = shell_from_torus(2.0, 0.5);
        let _ = intersect_faces(&p, &t);
    }

    #[test]
    fn plane_torus_through_center() {
        // The torus equatorial plane for a Z-axis torus is the XY plane.
        let p = plane_shell([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        let t = shell_from_torus(2.0, 0.5);
        let _ = intersect_faces(&p, &t);
    }
}

// ============================================================
// B-4: all 21 non-intersection pairs
// ============================================================

mod no_intersection_all_pairs {
    use super::*;

    const FAR: f64 = 100.0;

    fn far_plane() -> Shell {
        plane_shell([FAR, 0.0, 0.0], [1.0, 0.0, 0.0])
    }

    fn near_sphere() -> Shell {
        sphere_shell_at([0.0, 0.0, 0.0], 1.0)
    }

    fn far_sphere() -> Shell {
        sphere_shell_at([FAR, 0.0, 0.0], 1.0)
    }

    fn near_cylinder() -> Shell {
        cylinder_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], 1.0)
    }

    fn far_cylinder() -> Shell {
        cylinder_shell([FAR, 0.0, 0.0], [0.0, 0.0, 5.0], 1.0)
    }

    fn near_cone() -> Shell {
        cone_shell([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], FRAC_PI_4)
    }

    fn far_cone() -> Shell {
        cone_shell([FAR, 0.0, 0.0], [0.0, 0.0, 5.0], FRAC_PI_4)
    }

    fn near_ellipsoid() -> Shell {
        ellipsoid_shell_at([0.0, 0.0, 0.0], 1.0, 0.8, 0.6)
    }

    fn far_ellipsoid() -> Shell {
        ellipsoid_shell_at([FAR, 0.0, 0.0], 1.0, 0.8, 0.6)
    }

    fn near_torus() -> Shell {
        torus_shell_at([0.0, 0.0, 0.0], 2.0, 0.5)
    }

    fn far_torus() -> Shell {
        torus_shell_at([FAR, 0.0, 0.0], 2.0, 0.5)
    }

    fn assert_no_intersection(a: &Shell, b: &Shell, label: &str) {
        let curves = intersect_faces(a, b);
        assert!(
            curves.is_empty(),
            "{}: expected no intersection, got {} curves",
            label,
            curves.len()
        );
    }

    #[test]
    fn no_intersect_plane_plane() {
        let a = plane_shell([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        let b = plane_shell([0.0, 0.0, FAR], [0.0, 0.0, 1.0]);
        assert_no_intersection(&a, &b, "Plane×Plane");
    }

    #[test]
    fn no_intersect_plane_sphere() {
        assert_no_intersection(&far_plane(), &near_sphere(), "Plane×Sphere");
    }

    #[test]
    fn no_intersect_plane_cylinder() {
        assert_no_intersection(&far_plane(), &near_cylinder(), "Plane×Cylinder");
    }

    #[test]
    fn no_intersect_plane_cone() {
        assert_no_intersection(&far_plane(), &near_cone(), "Plane×Cone");
    }

    #[test]
    fn no_intersect_plane_ellipsoid() {
        assert_no_intersection(&far_plane(), &near_ellipsoid(), "Plane×Ellipsoid");
    }

    #[test]
    fn no_intersect_plane_torus() {
        assert_no_intersection(&far_plane(), &near_torus(), "Plane×Torus");
    }

    #[test]
    fn no_intersect_sphere_sphere() {
        assert_no_intersection(&near_sphere(), &far_sphere(), "Sphere×Sphere");
    }

    #[test]
    fn no_intersect_sphere_cylinder() {
        assert_no_intersection(&near_sphere(), &far_cylinder(), "Sphere×Cylinder");
    }

    #[test]
    fn no_intersect_sphere_cone() {
        assert_no_intersection(&near_sphere(), &far_cone(), "Sphere×Cone");
    }

    #[test]
    fn no_intersect_sphere_ellipsoid() {
        assert_no_intersection(&near_sphere(), &far_ellipsoid(), "Sphere×Ellipsoid");
    }

    #[test]
    fn no_intersect_sphere_torus() {
        assert_no_intersection(&near_sphere(), &far_torus(), "Sphere×Torus");
    }

    #[test]
    fn no_intersect_cylinder_cylinder() {
        assert_no_intersection(&near_cylinder(), &far_cylinder(), "Cylinder×Cylinder");
    }

    #[test]
    fn no_intersect_cylinder_cone() {
        assert_no_intersection(&near_cylinder(), &far_cone(), "Cylinder×Cone");
    }

    #[test]
    fn no_intersect_cylinder_ellipsoid() {
        assert_no_intersection(&near_cylinder(), &far_ellipsoid(), "Cylinder×Ellipsoid");
    }

    #[test]
    fn no_intersect_cylinder_torus() {
        assert_no_intersection(&near_cylinder(), &far_torus(), "Cylinder×Torus");
    }

    #[test]
    fn no_intersect_cone_cone() {
        assert_no_intersection(&near_cone(), &far_cone(), "Cone×Cone");
    }

    #[test]
    fn no_intersect_cone_ellipsoid() {
        assert_no_intersection(&near_cone(), &far_ellipsoid(), "Cone×Ellipsoid");
    }

    #[test]
    fn no_intersect_cone_torus() {
        assert_no_intersection(&near_cone(), &far_torus(), "Cone×Torus");
    }

    #[test]
    fn no_intersect_ellipsoid_ellipsoid() {
        assert_no_intersection(&near_ellipsoid(), &far_ellipsoid(), "Ellipsoid×Ellipsoid");
    }

    #[test]
    fn no_intersect_ellipsoid_torus() {
        assert_no_intersection(&near_ellipsoid(), &far_torus(), "Ellipsoid×Torus");
    }

    #[test]
    fn no_intersect_torus_torus() {
        assert_no_intersection(&near_torus(), &far_torus(), "Torus×Torus");
    }
}
