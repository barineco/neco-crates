#![cfg(any(feature = "spline", feature = "nurbs"))]

use neco_edge_routing::{route, PathKind, RouteRequest, RouteStyle};

fn request(style: RouteStyle) -> RouteRequest {
    RouteRequest {
        from: (0.0, 0.0),
        to: (10.0, 4.0),
        from_tangent: (1.0, 0.0),
        to_tangent: (-1.0, 0.0),
        style,
    }
}

#[cfg(feature = "spline")]
#[test]
fn spline_style_routes_when_feature_is_enabled() {
    let path = route(&request(RouteStyle::Spline)).expect("spline route");
    assert_eq!(path.kind, PathKind::Cubic);
}

#[cfg(feature = "nurbs")]
#[test]
fn nurbs_style_routes_when_feature_is_enabled() {
    let path = route(&request(RouteStyle::Nurbs { degree: 3 })).expect("nurbs route");
    assert!(matches!(path.kind, PathKind::Nurbs { .. }));
}
