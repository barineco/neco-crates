//! Integration tests for 2D boolean operations.

use approx::assert_relative_eq;
use neco_brep::{boolean_2d, boolean_2d_all, BooleanOp};
use neco_nurbs::{NurbsCurve2D, NurbsRegion};

/// Build a closed polygon as a degree-1 NURBS curve.
fn make_closed_polygon_nurbs(vertices: &[[f64; 2]]) -> NurbsCurve2D {
    let mut pts = vertices.to_vec();
    pts.push(vertices[0]); // Close the polygon.

    let n = pts.len();
    let mut knots = Vec::with_capacity(n + 2);
    knots.push(0.0);
    for i in 0..n {
        knots.push(i as f64 / (n - 1) as f64);
    }
    knots.push(1.0);

    NurbsCurve2D::new(1, pts, knots)
}

/// Build a counter-clockwise rectangular region centered at `(cx, cy)`.
fn make_rect_region(cx: f64, cy: f64, w: f64, h: f64) -> NurbsRegion {
    let hw = w / 2.0;
    let hh = h / 2.0;
    let verts = vec![
        [cx - hw, cy - hh],
        [cx + hw, cy - hh],
        [cx + hw, cy + hh],
        [cx - hw, cy + hh],
    ];
    NurbsRegion {
        outer: vec![make_closed_polygon_nurbs(&verts)],
        holes: vec![],
    }
}

/// Compute polygon area with the shoelace formula.
fn polygon_area(pts: &[[f64; 2]]) -> f64 {
    let n = pts.len();
    if n < 3 {
        return 0.0;
    }
    let mut area = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        area += pts[i][0] * pts[j][1];
        area -= pts[j][0] * pts[i][1];
    }
    area.abs() * 0.5
}

/// Approximate a `NurbsRegion` area from a polyline sample.
fn region_area(region: &NurbsRegion) -> f64 {
    let pts = region.outer_adaptive_sample(0.01);
    polygon_area(&pts)
}

fn region_set_area(regions: &[NurbsRegion]) -> f64 {
    regions.iter().map(region_area).sum()
}

#[test]
fn union_overlapping_rectangles() {
    let a = make_rect_region(0.0, 0.0, 2.0, 2.0);
    let b = make_rect_region(1.0, 0.5, 2.0, 2.0);

    let result = boolean_2d(&a, &b, BooleanOp::Union).expect("Union should succeed");
    let area = region_area(&result);
    assert_relative_eq!(area, 6.5, epsilon = 0.3);
}

#[test]
fn intersect_overlapping_rectangles() {
    let a = make_rect_region(0.0, 0.0, 2.0, 2.0);
    let b = make_rect_region(1.0, 0.5, 2.0, 2.0);

    let result = boolean_2d(&a, &b, BooleanOp::Intersect).expect("Intersect should succeed");
    let area = region_area(&result);
    assert_relative_eq!(area, 1.5, epsilon = 0.3);
}

#[test]
fn subtract_contained_rect_creates_hole() {
    let large = make_rect_region(0.0, 0.0, 4.0, 4.0);
    let small = make_rect_region(0.0, 0.0, 2.0, 2.0);

    let result = boolean_2d(&large, &small, BooleanOp::Subtract).expect("Subtract should succeed");
    assert_eq!(result.holes.len(), 1, "expected one hole");
}

#[test]
fn disjoint_union_error() {
    let a = make_rect_region(-5.0, 0.0, 2.0, 2.0);
    let b = make_rect_region(5.0, 0.0, 2.0, 2.0);

    let result = boolean_2d(&a, &b, BooleanOp::Union);
    assert!(result.is_err(), "union of disjoint regions should fail");
}

#[test]
fn disjoint_union_returns_two_regions_in_all_api() {
    let a = make_rect_region(-5.0, 0.0, 2.0, 2.0);
    let b = make_rect_region(5.0, 0.0, 2.0, 2.0);

    let result = boolean_2d_all(&a, &b, BooleanOp::Union).expect("all-api union should succeed");
    assert_eq!(result.len(), 2, "expected two disjoint regions");
    assert_relative_eq!(region_set_area(result.as_slice()), 8.0, epsilon = 0.1);
}

#[test]
fn disjoint_intersect_error() {
    let a = make_rect_region(-5.0, 0.0, 2.0, 2.0);
    let b = make_rect_region(5.0, 0.0, 2.0, 2.0);

    let result = boolean_2d(&a, &b, BooleanOp::Intersect);
    assert!(
        result.is_err(),
        "intersection of disjoint regions should fail"
    );
}

#[test]
fn disjoint_intersect_returns_empty_in_all_api() {
    let a = make_rect_region(-5.0, 0.0, 2.0, 2.0);
    let b = make_rect_region(5.0, 0.0, 2.0, 2.0);

    let result =
        boolean_2d_all(&a, &b, BooleanOp::Intersect).expect("all-api intersect should succeed");
    assert!(result.is_empty(), "expected empty result");
}

#[test]
fn full_containment_union_returns_outer() {
    let large = make_rect_region(0.0, 0.0, 4.0, 4.0);
    let small = make_rect_region(0.0, 0.0, 2.0, 2.0);

    let result = boolean_2d(&large, &small, BooleanOp::Union).expect("Union should succeed");
    let area = region_area(&result);
    assert_relative_eq!(area, 16.0, epsilon = 0.5);
}

#[test]
fn full_containment_intersect_returns_inner() {
    let large = make_rect_region(0.0, 0.0, 4.0, 4.0);
    let small = make_rect_region(0.0, 0.0, 2.0, 2.0);

    let result =
        boolean_2d(&large, &small, BooleanOp::Intersect).expect("Intersect should succeed");
    let area = region_area(&result);
    assert_relative_eq!(area, 4.0, epsilon = 0.5);
}

#[test]
fn union_edge_sharing_l_shape() {
    let a = make_rect_region(0.0, 0.0, 2.0, 1.0);
    let b = make_rect_region(0.0, 0.0, 1.0, 2.0);

    let result =
        boolean_2d(&a, &b, BooleanOp::Union).expect("union across collinear edges should succeed");
    let area = region_area(&result);
    assert_relative_eq!(area, 3.0, epsilon = 0.1);
}

#[test]
fn union_fully_shared_edge() {
    let a = make_rect_region(1.0, 0.5, 2.0, 1.0);
    let b = make_rect_region(1.0, 1.5, 2.0, 1.0);
    let result = boolean_2d(&a, &b, BooleanOp::Union).expect("operation should succeed");
    assert_relative_eq!(region_area(&result), 4.0, epsilon = 0.1);
}

#[test]
fn union_t_junction() {
    let a = make_rect_region(1.5, 0.5, 3.0, 1.0);
    let b = make_rect_region(1.5, 1.5, 1.0, 1.0);
    let result = boolean_2d(&a, &b, BooleanOp::Union).expect("operation should succeed");
    assert_relative_eq!(region_area(&result), 4.0, epsilon = 0.1);
}

#[test]
fn subtract_edge_sharing() {
    let a = make_rect_region(0.0, 0.0, 2.0, 2.0);
    let b = make_rect_region(1.0, 0.0, 2.0, 2.0);
    let result = boolean_2d(&a, &b, BooleanOp::Subtract).expect("operation should succeed");
    assert_relative_eq!(region_area(&result), 2.0, epsilon = 0.1);
}

#[test]
fn intersect_edge_sharing() {
    let a = make_rect_region(0.0, 0.0, 2.0, 1.0);
    let b = make_rect_region(0.0, 0.0, 1.0, 2.0);
    let result = boolean_2d(&a, &b, BooleanOp::Intersect).expect("operation should succeed");
    assert_relative_eq!(region_area(&result), 1.0, epsilon = 0.1);
}

#[test]
fn union_adjacent_rects() {
    let a = make_rect_region(1.0, 0.5, 2.0, 1.0);
    let b = make_rect_region(3.0, 0.5, 2.0, 1.0);
    let result = boolean_2d(&a, &b, BooleanOp::Union).expect("operation should succeed");
    assert_relative_eq!(region_area(&result), 4.0, epsilon = 0.1);
}

#[test]
fn triple_union_chain() {
    let a = make_rect_region(0.0, 0.0, 2.0, 1.0);
    let b = make_rect_region(0.0, 0.0, 1.0, 2.0);
    let ab = boolean_2d(&a, &b, BooleanOp::Union).expect("A union B should succeed");
    assert_relative_eq!(region_area(&ab), 3.0, epsilon = 0.1);

    let c = make_rect_region(1.0, 0.0, 1.0, 2.0);
    let abc = boolean_2d(&ab, &c, BooleanOp::Union).expect("A union B union C should succeed");
    let area = region_area(&abc);
    assert!(
        area > 3.5 && area < 5.5,
        "area should stay in [3.5, 5.5], got {}",
        area
    );
}

#[test]
fn subtract_circle_from_rect() {
    let rect = make_rect_region(0.0, 0.0, 2.0, 2.0);
    let circle_curve = NurbsCurve2D::circle([0.0, 0.0], 0.5);
    let circle = NurbsRegion {
        outer: vec![circle_curve],
        holes: vec![],
    };

    let result = boolean_2d(&rect, &circle, BooleanOp::Subtract).expect("subtract should succeed");
    assert_eq!(result.holes.len(), 1, "expected one hole");
    for seg in &result.holes[0] {
        assert!(
            seg.degree >= 2,
            "circle hole segments should keep degree >= 2, got {}",
            seg.degree
        );
    }
}

#[test]
fn intersect_circle_and_rect() {
    let rect = make_rect_region(1.0, 1.0, 2.0, 2.0);
    let circle_curve = NurbsCurve2D::circle([0.0, 0.0], 1.0);
    let circle = NurbsRegion {
        outer: vec![circle_curve],
        holes: vec![],
    };

    let result =
        boolean_2d(&rect, &circle, BooleanOp::Intersect).expect("intersect should succeed");
    let area = region_area(&result);
    let expected = std::f64::consts::PI / 4.0;
    assert_relative_eq!(area, expected, epsilon = 0.1);
    let has_degree2 = result.outer.iter().any(|seg| seg.degree >= 2);
    assert!(
        has_degree2,
        "intersection outer loop should contain a segment with degree >= 2"
    );
}

#[test]
fn intersect_preserves_degree2_arc() {
    let rect = make_rect_region(0.0, 0.0, 3.0, 3.0);
    let circle_curve = NurbsCurve2D::circle([0.0, 0.0], 1.0);
    let circle = NurbsRegion {
        outer: vec![circle_curve],
        holes: vec![],
    };

    let result =
        boolean_2d(&rect, &circle, BooleanOp::Intersect).expect("intersect should succeed");
    let area = region_area(&result);
    let expected = std::f64::consts::PI;
    assert_relative_eq!(area, expected, epsilon = 0.2);
    let has_degree2 = result.outer.iter().any(|seg| seg.degree >= 2);
    assert!(
        has_degree2,
        "circle intersection should retain at least one degree >= 2 segment"
    );
}

#[test]
fn boolean_preserves_degree3_curve() {
    let cp = vec![
        [1.0, 0.0],
        [1.5, 0.8],
        [0.5, 1.5],
        [-0.5, 1.5],
        [-1.5, 0.8],
        [-1.0, 0.0],
        [-1.5, -0.8],
        [-0.5, -1.5],
        [0.5, -1.5],
        [1.5, -0.8],
        [1.0, 0.0],
        [1.5, 0.8],
        [0.5, 1.5],
    ];
    let knots = vec![
        0.0, 0.0, 0.0, 0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.0, 1.0, 1.0,
    ];
    let curve = NurbsCurve2D::new(3, cp, knots);
    assert_eq!(curve.degree, 3, "input curve should be degree 3");

    let free_region = NurbsRegion {
        outer: vec![curve],
        holes: vec![],
    };

    let rect = make_rect_region(0.0, 0.0, 6.0, 6.0);
    let result =
        boolean_2d(&rect, &free_region, BooleanOp::Intersect).expect("intersect should succeed");

    let has_degree3 = result.outer.iter().any(|seg| seg.degree >= 3);
    assert!(
        has_degree3,
        "intersection with a degree-3 curve should retain a segment with degree >= 3"
    );
}

#[test]
fn boolean_chain_piecewise_input() {
    let a = make_rect_region(0.0, 0.0, 2.0, 2.0);
    let b = make_rect_region(1.0, 0.5, 2.0, 2.0);
    let ab = boolean_2d(&a, &b, BooleanOp::Union).expect("A union B should succeed");
    assert!(
        ab.outer.len() >= 2,
        "union result should keep a piecewise outer loop, got {} segments",
        ab.outer.len()
    );

    let c = make_rect_region(0.0, 0.0, 3.0, 1.0);
    let result =
        boolean_2d(&ab, &c, BooleanOp::Intersect).expect("(A union B) intersect C should succeed");

    let area = region_area(&result);
    assert!(
        area > 0.5,
        "intersection area should remain meaningfully positive, got {}",
        area
    );
    let ab_area = region_area(&ab);
    assert!(
        area < ab_area,
        "intersection area should be smaller than the original union: ab={}, result={}",
        ab_area,
        area
    );
}
