use neco_brep::bezier_decompose::decompose_to_bezier_patches;
use neco_brep::boolean3d::nurbs_intersect::{
    nurbs_nurbs_intersection, nurbs_plane_intersection, nurbs_quadric_intersection,
    nurbs_torus_intersection, quadric_implicit, torus_implicit,
};
use neco_brep::Surface;
use neco_nurbs::NurbsSurface3D;

/// Biquadratic patch with degree 2 x 2, 3x3 control points, and uniform weights.
fn biquadratic_patch() -> NurbsSurface3D {
    NurbsSurface3D {
        degree_u: 2,
        degree_v: 2,
        control_points: vec![
            vec![[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 2.0, 0.0]],
            vec![[1.0, 0.0, 0.5], [1.0, 1.0, 1.0], [1.0, 2.0, 0.5]],
            vec![[2.0, 0.0, 0.0], [2.0, 1.0, 0.0], [2.0, 2.0, 0.0]],
        ],
        weights: vec![vec![1.0; 3]; 3],
        knots_u: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        knots_v: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    }
}

#[test]
fn insert_knot_u_preserves_shape() {
    let surf = biquadratic_patch();
    let inserted = surf.insert_knot_u(0.5);

    // Knot-vector length and control-point count must stay consistent.
    assert_eq!(inserted.knots_u.len(), surf.knots_u.len() + 1);
    assert_eq!(inserted.control_points.len(), surf.control_points.len() + 1);
    // The V-direction control-point count stays unchanged.
    assert_eq!(
        inserted.control_points[0].len(),
        surf.control_points[0].len()
    );

    // Surface evaluation should remain unchanged after insertion.
    let tol = 1e-12;
    for &u in &[0.0, 0.25, 0.5, 0.75, 1.0] {
        for &v in &[0.0, 0.25, 0.5, 0.75, 1.0] {
            let p_orig = surf.evaluate(u, v);
            let p_ins = inserted.evaluate(u, v);
            let dist = ((p_orig[0] - p_ins[0]).powi(2)
                + (p_orig[1] - p_ins[1]).powi(2)
                + (p_orig[2] - p_ins[2]).powi(2))
            .sqrt();
            assert!(
                dist < tol,
                "u={u}, v={v}: dist={dist} (before={p_orig:?}, after={p_ins:?})"
            );
        }
    }
}

#[test]
fn insert_knot_v_preserves_shape() {
    let surf = biquadratic_patch();
    let inserted = surf.insert_knot_v(0.5);

    // Knot-vector length and control-point count must stay consistent.
    assert_eq!(inserted.knots_v.len(), surf.knots_v.len() + 1);
    assert_eq!(
        inserted.control_points[0].len(),
        surf.control_points[0].len() + 1
    );
    // The U-direction control-point count stays unchanged.
    assert_eq!(inserted.control_points.len(), surf.control_points.len());

    // Surface evaluation should remain unchanged after insertion.
    let tol = 1e-12;
    for &u in &[0.0, 0.25, 0.5, 0.75, 1.0] {
        for &v in &[0.0, 0.25, 0.5, 0.75, 1.0] {
            let p_orig = surf.evaluate(u, v);
            let p_ins = inserted.evaluate(u, v);
            let dist = ((p_orig[0] - p_ins[0]).powi(2)
                + (p_orig[1] - p_ins[1]).powi(2)
                + (p_orig[2] - p_ins[2]).powi(2))
            .sqrt();
            assert!(
                dist < tol,
                "u={u}, v={v}: dist={dist} (before={p_orig:?}, after={p_ins:?})"
            );
        }
    }
}

// ---- Bezier patch decomposition tests ----

/// Test surface with two U spans, degree 3, and an internal knot at 0.5.
fn two_span_u_patch() -> NurbsSurface3D {
    NurbsSurface3D {
        degree_u: 3,
        degree_v: 2,
        control_points: vec![
            vec![[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 2.0, 0.0]],
            vec![[0.75, 0.0, 0.8], [0.75, 1.0, 1.2], [0.75, 2.0, 0.8]],
            vec![[1.5, 0.0, 1.0], [1.5, 1.0, 1.5], [1.5, 2.0, 1.0]],
            vec![[2.25, 0.0, 0.8], [2.25, 1.0, 1.2], [2.25, 2.0, 0.8]],
            vec![[3.0, 0.0, 0.0], [3.0, 1.0, 0.0], [3.0, 2.0, 0.0]],
        ],
        weights: vec![vec![1.0; 3]; 5],
        knots_u: vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
        knots_v: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    }
}

#[test]
fn single_span_is_single_patch() {
    let surf = biquadratic_patch();
    let patches = decompose_to_bezier_patches(&surf);
    assert_eq!(
        patches.len(),
        1,
        "surface without internal knots should yield one patch"
    );

    let patch = &patches[0];
    assert_eq!(patch.degree_u, 2);
    assert_eq!(patch.degree_v, 2);
    assert_eq!(patch.control_points.len(), 3);
    assert_eq!(patch.control_points[0].len(), 3);

    // Patch evaluation must match the original surface.
    let tol = 1e-12;
    for &u in &[0.0, 0.25, 0.5, 0.75, 1.0] {
        for &v in &[0.0, 0.25, 0.5, 0.75, 1.0] {
            let p_orig = surf.evaluate(u, v);
            let p_patch = patch.evaluate(u, v);
            let dist = ((p_orig[0] - p_patch[0]).powi(2)
                + (p_orig[1] - p_patch[1]).powi(2)
                + (p_orig[2] - p_patch[2]).powi(2))
            .sqrt();
            assert!(
                dist < tol,
                "u={u}, v={v}: dist={dist} (surface={p_orig:?}, patch={p_patch:?})"
            );
        }
    }
}

#[test]
fn two_span_u_decomposes_to_two_patches() {
    let surf = two_span_u_patch();
    let patches = decompose_to_bezier_patches(&surf);
    assert_eq!(
        patches.len(),
        2,
        "two U spans should decompose into two patches"
    );

    // Check parameter ranges.
    let (u0_min, u0_max) = patches[0].u_range();
    let (u1_min, u1_max) = patches[1].u_range();
    assert!((u0_min - 0.0).abs() < 1e-14);
    assert!((u0_max - 0.5).abs() < 1e-14);
    assert!((u1_min - 0.5).abs() < 1e-14);
    assert!((u1_max - 1.0).abs() < 1e-14);

    // Each patch should have `(degree_u + 1) x (degree_v + 1)` control points.
    for patch in &patches {
        assert_eq!(patch.control_points.len(), 4); // degree_u + 1 = 4
        assert_eq!(patch.control_points[0].len(), 3); // degree_v + 1 = 3
    }

    // Boundary continuity: both patches should agree at `u = 0.5`.
    let tol = 1e-10;
    for &v in &[0.0, 0.25, 0.5, 0.75, 1.0] {
        let p0 = patches[0].evaluate(0.5, v);
        let p1 = patches[1].evaluate(0.5, v);
        let dist =
            ((p0[0] - p1[0]).powi(2) + (p0[1] - p1[1]).powi(2) + (p0[2] - p1[2]).powi(2)).sqrt();
        assert!(
            dist < tol,
            "v={v}: boundary mismatch dist={dist} (patch0={p0:?}, patch1={p1:?})"
        );
    }

    // Check against the original surface.
    for &u in &[0.0, 0.1, 0.25, 0.4, 0.5, 0.6, 0.75, 0.9, 1.0] {
        for &v in &[0.0, 0.5, 1.0] {
            let p_orig = surf.evaluate(u, v);
            // Evaluate on the corresponding patch.
            let p_patch = if u <= 0.5 {
                patches[0].evaluate(u, v)
            } else {
                patches[1].evaluate(u, v)
            };
            let dist = ((p_orig[0] - p_patch[0]).powi(2)
                + (p_orig[1] - p_patch[1]).powi(2)
                + (p_orig[2] - p_patch[2]).powi(2))
            .sqrt();
            assert!(
                dist < tol,
                "u={u}, v={v}: dist={dist} (surface={p_orig:?}, patch={p_patch:?})"
            );
        }
    }
}

#[test]
fn patch_aabb_contains_patch() {
    let surf = two_span_u_patch();
    let patches = decompose_to_bezier_patches(&surf);

    for (idx, patch) in patches.iter().enumerate() {
        let (bb_min, bb_max) = patch.aabb();
        let (u_min, u_max) = patch.u_range();
        let (v_min, v_max) = patch.v_range();

        // Sample points from the patch should stay inside the AABB.
        let tol = 1e-12;
        for i in 0..=10 {
            let u = u_min + (u_max - u_min) * (i as f64 / 10.0);
            for j in 0..=10 {
                let v = v_min + (v_max - v_min) * (j as f64 / 10.0);
                let pt = patch.evaluate(u, v);
                assert!(
                    pt[0] >= bb_min[0] - tol
                        && pt[0] <= bb_max[0] + tol
                        && pt[1] >= bb_min[1] - tol
                        && pt[1] <= bb_max[1] + tol
                        && pt[2] >= bb_min[2] - tol
                        && pt[2] <= bb_max[2] + tol,
                    "patch {idx}: point ({}, {}, {}) is outside AABB [{:?}, {:?}] at (u={u}, v={v})",
                    pt[0],
                    pt[1],
                    pt[2],
                    bb_min,
                    bb_max,
                );
            }
        }
    }
}

// ---- NurbsSurface x Plane intersection tests ----

/// Cut the biquadratic patch by the plane `z = 0.25` and verify all points lie near that height.
#[test]
fn nurbs_plane_bisects_biquadratic_patch() {
    let surf = biquadratic_patch();
    let origin = [0.0, 0.0, 0.25];
    let normal = [0.0, 0.0, 1.0];

    let polylines = nurbs_plane_intersection(&surf, &origin, &normal);
    assert!(
        !polylines.is_empty(),
        "the plane z=0.25 should intersect the biquadratic patch"
    );

    let tol = 1e-3;
    let mut total_points = 0;
    for polyline in &polylines {
        assert!(
            polyline.len() >= 2,
            "polyline needs at least two points: len={}",
            polyline.len()
        );
        for pt in polyline {
            assert!(
                (pt[2] - 0.25).abs() < tol,
                "intersection point z coordinate deviates from plane height 0.25: z={}, pt={:?}",
                pt[2],
                pt
            );
            total_points += 1;
        }
    }
    assert!(
        total_points >= 10,
        "expected enough intersection samples, got {total_points} points"
    );
}

/// The plane `z = 5.0` should not intersect the biquadratic patch with `z` range `[0, 1]`.
#[test]
fn nurbs_plane_no_intersection() {
    let surf = biquadratic_patch();
    let origin = [0.0, 0.0, 5.0];
    let normal = [0.0, 0.0, 1.0];

    let polylines = nurbs_plane_intersection(&surf, &origin, &normal);
    assert!(
        polylines.is_empty(),
        "the plane z=5.0 should miss the patch with z range [0, 1], but {} polylines were returned",
        polylines.len()
    );
}

/// Intersection of the two-span patch with the plane `z = 0.5`.
#[test]
fn nurbs_plane_two_span_intersection() {
    let surf = two_span_u_patch();
    let origin = [0.0, 0.0, 0.5];
    let normal = [0.0, 0.0, 1.0];

    let polylines = nurbs_plane_intersection(&surf, &origin, &normal);
    assert!(
        !polylines.is_empty(),
        "the plane z=0.5 should intersect the two-span patch"
    );

    let tol = 1e-3;
    let mut total_points = 0;
    for polyline in &polylines {
        for pt in polyline {
            assert!(
                (pt[2] - 0.5).abs() < tol,
                "intersection point z coordinate deviates from plane height 0.5: z={}, pt={:?}",
                pt[2],
                pt
            );
            total_points += 1;
        }
    }
    assert!(
        total_points >= 10,
        "expected enough intersection samples, got {total_points} points"
    );
}

// ---- NurbsSurface x Quadric intersection tests ----

/// Intersect `two_span_u_patch` with a sphere and verify every point lies on the sphere.
#[test]
fn nurbs_sphere_intersection() {
    let surf = two_span_u_patch();
    let sphere = Surface::Sphere {
        center: [1.5, 1.0, 0.5],
        radius: 1.0,
    };

    let polylines = nurbs_quadric_intersection(&surf, &sphere);
    assert!(
        !polylines.is_empty(),
        "Sphere should intersect two_span_u_patch"
    );

    let tol = 1e-3;
    let mut total_points = 0;
    for polyline in &polylines {
        assert!(
            polyline.len() >= 2,
            "polyline needs at least two points: len={}",
            polyline.len()
        );
        for pt in polyline {
            let val = quadric_implicit(&sphere, pt).abs();
            assert!(
                val < tol,
                "intersection point is not on the Sphere: implicit={val}, pt={pt:?}"
            );
            total_points += 1;
        }
    }
    assert!(
        total_points >= 10,
        "expected enough intersection samples, got {total_points} points"
    );
}

/// Intersect `two_span_u_patch` with a cylinder and verify every point lies on the cylinder.
#[test]
fn nurbs_cylinder_intersection() {
    let surf = two_span_u_patch();
    let cylinder = Surface::Cylinder {
        origin: [1.5, 0.0, 0.0],
        axis: [0.0, 1.0, 0.0],
        radius: 1.2,
    };

    let polylines = nurbs_quadric_intersection(&surf, &cylinder);
    assert!(
        !polylines.is_empty(),
        "Cylinder should intersect two_span_u_patch"
    );

    let tol = 1e-3;
    let mut total_points = 0;
    for polyline in &polylines {
        assert!(
            polyline.len() >= 2,
            "polyline needs at least two points: len={}",
            polyline.len()
        );
        for pt in polyline {
            let val = quadric_implicit(&cylinder, pt).abs();
            assert!(
                val < tol,
                "intersection point is not on the Cylinder: implicit={val}, pt={pt:?}"
            );
            total_points += 1;
        }
    }
    assert!(
        total_points >= 10,
        "expected enough intersection samples, got {total_points} points"
    );
}

/// `biquadratic_patch` against a far sphere should yield no intersection.
#[test]
fn nurbs_quadric_no_intersection() {
    let surf = biquadratic_patch();
    let sphere = Surface::Sphere {
        center: [100.0, 100.0, 100.0],
        radius: 1.0,
    };

    let polylines = nurbs_quadric_intersection(&surf, &sphere);
    assert!(
        polylines.is_empty(),
        "a far sphere should not intersect biquadratic_patch, but {} polylines were returned",
        polylines.len()
    );
}

// ---- NurbsSurface x Torus intersection tests ----

/// Intersect `two_span_u_patch` with a torus and verify every point lies on the torus.
#[test]
fn nurbs_torus_intersection_basic() {
    let surf = two_span_u_patch();
    let torus_center = [1.5, 1.0, 0.0];
    let torus_axis = [0.0, 0.0, 1.0];
    let major_r = 1.5;
    let minor_r = 0.5;

    let polylines = nurbs_torus_intersection(&surf, &torus_center, &torus_axis, major_r, minor_r);
    assert!(
        !polylines.is_empty(),
        "Torus should intersect two_span_u_patch"
    );

    let tol = 1e-3;
    let mut total_points = 0;
    for polyline in &polylines {
        assert!(
            polyline.len() >= 2,
            "polyline needs at least two points: len={}",
            polyline.len()
        );
        for pt in polyline {
            let val = torus_implicit(pt, &torus_center, &torus_axis, major_r, minor_r).abs();
            assert!(
                val < tol,
                "intersection point is not on the Torus: implicit={val}, pt={pt:?}"
            );
            total_points += 1;
        }
    }
    assert!(
        total_points >= 10,
        "expected enough intersection samples, got {total_points} points"
    );
}

// ---- NurbsSurface x NurbsSurface intersection tests ----

/// Intersect two NURBS patches and verify that each point stays close to both surfaces.
#[test]
fn nurbs_nurbs_intersection_basic() {
    let surf_a = biquadratic_patch();
    let surf_b = NurbsSurface3D {
        degree_u: 2,
        degree_v: 2,
        control_points: vec![
            vec![[0.0, 0.0, -1.0], [0.0, 1.0, -1.0], [0.0, 2.0, -1.0]],
            vec![[1.0, 0.0, 1.0], [1.0, 1.0, 3.0], [1.0, 2.0, 1.0]],
            vec![[2.0, 0.0, -1.0], [2.0, 1.0, -1.0], [2.0, 2.0, -1.0]],
        ],
        weights: vec![vec![1.0; 3]; 3],
        knots_u: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        knots_v: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    };

    let polylines = nurbs_nurbs_intersection(&surf_a, &surf_b);
    assert!(
        !polylines.is_empty(),
        "the two NURBS patches should intersect"
    );

    let tol = 1e-2;
    let mut total_points = 0;
    for polyline in &polylines {
        assert!(
            polyline.len() >= 2,
            "polyline needs at least two points: len={}",
            polyline.len()
        );
        for pt in polyline {
            let dist_a = min_dist_to_surface(&surf_a, pt);
            let dist_b = min_dist_to_surface(&surf_b, pt);
            assert!(
                dist_a < tol,
                "intersection point is too far from surf_a: dist={dist_a}, pt={pt:?}"
            );
            assert!(
                dist_b < tol,
                "intersection point is too far from surf_b: dist={dist_b}, pt={pt:?}"
            );
            total_points += 1;
        }
    }
    assert!(
        total_points >= 5,
        "expected enough intersection samples, got {total_points} points"
    );
}

/// Estimate the minimum distance from `target` to a NURBS surface via grid search and Newton projection.
fn min_dist_to_surface(surface: &NurbsSurface3D, target: &[f64; 3]) -> f64 {
    let (u_min, u_max) = surface.u_range();
    let (v_min, v_max) = surface.v_range();

    let n = 32;
    let mut best_u = u_min;
    let mut best_v = v_min;
    let mut best_dist_sq = f64::INFINITY;

    for i in 0..=n {
        let u = u_min + (u_max - u_min) * (i as f64 / n as f64);
        for j in 0..=n {
            let v = v_min + (v_max - v_min) * (j as f64 / n as f64);
            let pt = surface.evaluate(u, v);
            let dx = pt[0] - target[0];
            let dy = pt[1] - target[1];
            let dz = pt[2] - target[2];
            let d2 = dx * dx + dy * dy + dz * dz;
            if d2 < best_dist_sq {
                best_dist_sq = d2;
                best_u = u;
                best_v = v;
            }
        }
    }

    let mut u = best_u;
    let mut v = best_v;
    for _ in 0..30 {
        let pt = surface.evaluate(u, v);
        let du = surface.partial_u(u, v);
        let dv = surface.partial_v(u, v);

        let rx = pt[0] - target[0];
        let ry = pt[1] - target[1];
        let rz = pt[2] - target[2];

        let a11 = du[0] * du[0] + du[1] * du[1] + du[2] * du[2];
        let a12 = du[0] * dv[0] + du[1] * dv[1] + du[2] * dv[2];
        let a22 = dv[0] * dv[0] + dv[1] * dv[1] + dv[2] * dv[2];
        let b1 = du[0] * rx + du[1] * ry + du[2] * rz;
        let b2 = dv[0] * rx + dv[1] * ry + dv[2] * rz;

        let det = a11 * a22 - a12 * a12;
        if det.abs() < 1e-30 {
            break;
        }

        let delta_u = (a22 * b1 - a12 * b2) / det;
        let delta_v = (a11 * b2 - a12 * b1) / det;

        u = (u - delta_u).clamp(u_min, u_max);
        v = (v - delta_v).clamp(v_min, v_max);
    }

    let pt = surface.evaluate(u, v);
    let dx = pt[0] - target[0];
    let dy = pt[1] - target[1];
    let dz = pt[2] - target[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Check that pruning remains effective on multi-span NURBS input.
#[test]
fn convex_hull_pruning_reduces_candidate_patches() {
    let n_cp = 12;
    let degree_u = 2;
    let mut cp = Vec::with_capacity(n_cp);
    let mut w = Vec::with_capacity(n_cp);
    for i in 0..n_cp {
        let x = i as f64;
        cp.push(vec![[x, 0.0, 0.0], [x, 1.0, 0.0], [x, 2.0, 0.0]]);
        w.push(vec![1.0; 3]);
    }
    let mut knots_u = vec![0.0; degree_u + 1];
    for i in 1..=(n_cp - degree_u - 1) {
        knots_u.push(i as f64);
    }
    let last = *knots_u.last().unwrap();
    knots_u.push(last);
    knots_u.push(last);

    let surf = NurbsSurface3D {
        degree_u,
        degree_v: 2,
        control_points: cp,
        weights: w,
        knots_u,
        knots_v: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    };

    let patches = decompose_to_bezier_patches(&surf);
    assert!(
        patches.len() >= 8,
        "expected at least 8 patches, got {}",
        patches.len()
    );

    let sphere = Surface::Sphere {
        center: [2.0, 1.0, 0.0],
        radius: 0.5,
    };

    let polylines = nurbs_quadric_intersection(&surf, &sphere);
    for polyline in &polylines {
        for pt in polyline {
            let dist = ((pt[0] - 2.0).powi(2) + (pt[1] - 1.0).powi(2) + pt[2].powi(2)).sqrt();
            assert!(
                (dist - 0.5).abs() < 0.1,
                "point is not on the Sphere: dist={dist}"
            );
        }
    }
}

// ---- NurbsSurface degenerate-case tests ----

#[test]
fn nurbs_plane_tangent_no_panic() {
    let surf = biquadratic_patch();
    let z_max = surf.evaluate(0.5, 0.5)[2];
    let plane_origin = [0.0, 0.0, z_max];
    let plane_normal = [0.0, 0.0, 1.0];

    let polylines = nurbs_plane_intersection(&surf, &plane_origin, &plane_normal);
    for polyline in &polylines {
        for pt in polyline {
            assert!(
                pt[0].is_finite() && pt[1].is_finite() && pt[2].is_finite(),
                "intersection point is non-finite for plane tangency: {pt:?}"
            );
        }
    }
}

#[test]
fn nurbs_sphere_tangent_no_panic() {
    let surf = biquadratic_patch();
    let peak = surf.evaluate(0.5, 0.5);
    let radius = 1.0;
    let sphere = Surface::Sphere {
        center: [peak[0], peak[1], peak[2] + radius],
        radius,
    };

    let polylines = nurbs_quadric_intersection(&surf, &sphere);
    for polyline in &polylines {
        for pt in polyline {
            assert!(
                pt[0].is_finite() && pt[1].is_finite() && pt[2].is_finite(),
                "intersection point is non-finite for sphere tangency: {pt:?}"
            );
        }
    }
}
