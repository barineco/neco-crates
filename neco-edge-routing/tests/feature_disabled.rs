#![cfg(any(not(feature = "spline"), not(feature = "nurbs")))]

use neco_edge_routing::{route, RouteRequest, RouteStyle, RoutingError};

fn request(style: RouteStyle) -> RouteRequest {
    RouteRequest {
        from: (0.0, 0.0),
        to: (10.0, 4.0),
        from_tangent: (1.0, 0.0),
        to_tangent: (-1.0, 0.0),
        style,
    }
}

#[cfg(not(feature = "spline"))]
#[test]
fn spline_style_errors_when_feature_is_disabled() {
    assert_eq!(
        route(&request(RouteStyle::Spline)),
        Err(RoutingError::FeatureDisabled { style: "Spline" })
    );
}

#[cfg(not(feature = "nurbs"))]
#[test]
fn nurbs_style_errors_when_feature_is_disabled() {
    assert_eq!(
        route(&request(RouteStyle::Nurbs { degree: 3 })),
        Err(RoutingError::FeatureDisabled { style: "Nurbs" })
    );
}
