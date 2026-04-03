//! Tests for smooth loft shell generation.

use neco_brep::vec3;
use neco_brep::{shell_from_loft, LoftMode, LoftSection, Surface};
use neco_nurbs::{NurbsCurve2D, NurbsRegion};

/// Translation matrix moving by `oz` along the Z axis.
fn translate_z(oz: f64) -> [[f64; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, oz],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// Build a square profile centered at the origin with edge length `size`.
fn make_square_profile(size: f64) -> NurbsRegion {
    let h = size / 2.0;
    let outer = NurbsCurve2D::new(
        1,
        vec![[h, h], [-h, h], [-h, -h], [h, -h], [h, h]],
        vec![0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0],
    );
    NurbsRegion {
        outer: vec![outer],
        holes: vec![],
    }
}

/// Build an arc profile from four rational Bezier quarter arcs.
fn make_arc_profile(r: f64) -> NurbsRegion {
    let w = std::f64::consts::FRAC_1_SQRT_2;
    let outer = NurbsCurve2D::new_rational(
        2,
        vec![
            [r, 0.0],
            [r, r],
            [0.0, r],
            [-r, r],
            [-r, 0.0],
            [-r, -r],
            [0.0, -r],
            [r, -r],
            [r, 0.0],
        ],
        vec![1.0, w, 1.0, w, 1.0, w, 1.0, w, 1.0],
        vec![
            0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0,
        ],
    );
    NurbsRegion {
        outer: vec![outer],
        holes: vec![],
    }
}

/// Check that a three-section smooth loft creates the expected face count.
#[test]
fn loft_smooth_shell_generation() {
    let sections = vec![
        LoftSection {
            profile: make_square_profile(1.0),
            transform: translate_z(0.0),
        },
        LoftSection {
            profile: make_square_profile(1.0),
            transform: translate_z(1.0),
        },
        LoftSection {
            profile: make_square_profile(1.0),
            transform: translate_z(2.0),
        },
    ];

    let shell = shell_from_loft(&sections, LoftMode::Smooth).unwrap();
    // face count = (n_sections - 1) side faces + 2 caps = 2 + 2 = 4
    assert_eq!(
        shell.faces.len(),
        4,
        "three-section smooth loft should have 4 faces"
    );
}

/// Check that smooth and straight lofts are geometrically identical for two sections.
#[test]
fn loft_smooth_two_sections_geometric_equivalence() {
    let sections = vec![
        LoftSection {
            profile: make_square_profile(1.0),
            transform: translate_z(0.0),
        },
        LoftSection {
            profile: make_square_profile(1.0),
            transform: translate_z(1.0),
        },
    ];

    let shell_smooth = shell_from_loft(&sections, LoftMode::Smooth).unwrap();
    let shell_straight = shell_from_loft(&sections, LoftMode::Straight).unwrap();

    let surf_smooth = &shell_smooth.faces[0].surface;
    let surf_straight = &shell_straight.faces[0].surface;

    for &u in &[0.0, 0.25, 0.5, 0.75, 1.0] {
        for &v in &[0.0, 0.25, 0.5, 0.75, 1.0] {
            let p_smooth = surf_smooth.evaluate(u, v);
            let p_straight = surf_straight.evaluate(u, v);
            let dist = vec3::distance(p_smooth, p_straight);
            assert!(
                dist < 1e-10,
                "u={u}, v={v}: smooth/straight distance {dist} exceeds tolerance"
            );
        }
    }
}

/// Check that each side face is a `NurbsSurface` with `degree_u = 3`.
#[test]
fn loft_smooth_surface_type() {
    let sections = vec![
        LoftSection {
            profile: make_square_profile(1.0),
            transform: translate_z(0.0),
        },
        LoftSection {
            profile: make_square_profile(1.0),
            transform: translate_z(1.0),
        },
        LoftSection {
            profile: make_square_profile(1.0),
            transform: translate_z(2.0),
        },
    ];

    let shell = shell_from_loft(&sections, LoftMode::Smooth).unwrap();

    for i in 0..2 {
        match &shell.faces[i].surface {
            Surface::NurbsSurface { data } => {
                assert_eq!(
                    data.degree_u, 3,
                    "smooth loft side face {i} should have degree_u = 3"
                );
            }
            other => {
                panic!("side face {i} should be NurbsSurface, got {:?}", other);
            }
        }
    }
}

/// Check C1 continuity by matching `dS/du` across adjacent segment boundaries.
#[test]
fn loft_smooth_c1_continuity() {
    let sections = vec![
        LoftSection {
            profile: make_square_profile(1.0),
            transform: translate_z(0.0),
        },
        LoftSection {
            profile: make_square_profile(1.2),
            transform: translate_z(1.0),
        },
        LoftSection {
            profile: make_square_profile(0.8),
            transform: translate_z(2.5),
        },
    ];

    let shell = shell_from_loft(&sections, LoftMode::Smooth).unwrap();

    let surf0 = match &shell.faces[0].surface {
        Surface::NurbsSurface { data } => data.as_ref(),
        _ => panic!("side face 0 should be NurbsSurface"),
    };
    let surf1 = match &shell.faces[1].surface {
        Surface::NurbsSurface { data } => data.as_ref(),
        _ => panic!("side face 1 should be NurbsSurface"),
    };

    for &v in &[0.0, 0.25, 0.5, 0.75, 1.0] {
        let du0 = surf0.partial_u(1.0, v);
        let du1 = surf1.partial_u(0.0, v);
        let diff = vec3::length(vec3::sub(du0, du1));
        assert!(
            diff < 1e-6,
            "v={v}: boundary dS/du mismatch {diff} exceeds tolerance (du0={:?}, du1={:?})",
            du0,
            du1
        );
    }
}

/// Check that a smooth loft over rational arc profiles produces a valid shell.
#[test]
fn loft_smooth_rational_profile() {
    let sections = vec![
        LoftSection {
            profile: make_arc_profile(0.5),
            transform: translate_z(0.0),
        },
        LoftSection {
            profile: make_arc_profile(0.3),
            transform: translate_z(1.0),
        },
        LoftSection {
            profile: make_arc_profile(0.4),
            transform: translate_z(2.0),
        },
    ];

    let shell = shell_from_loft(&sections, LoftMode::Smooth).unwrap();
    assert_eq!(
        shell.faces.len(),
        4,
        "rational three-section smooth loft should have 4 faces"
    );

    for i in 0..2 {
        match &shell.faces[i].surface {
            Surface::NurbsSurface { data } => {
                assert_eq!(
                    data.degree_u, 3,
                    "rational side face {i} should have degree_u = 3"
                );
            }
            other => {
                panic!("side face {i} should be NurbsSurface, got {:?}", other);
            }
        }
    }
}
