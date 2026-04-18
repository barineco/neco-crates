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

#[test]
fn linear_route_returns_endpoints_only() {
    let path = route(&request(RouteStyle::Linear)).expect("linear route");
    assert_eq!(path.kind, PathKind::Polyline);
    assert_eq!(path.points, vec![(0.0, 0.0), (10.0, 4.0)]);
}

#[test]
fn bezier_route_emits_dist_scaled_control_points() {
    let path = route(&request(RouteStyle::Bezier { curvature: 0.25 })).expect("bezier route");
    assert_eq!(path.kind, PathKind::Cubic);
    assert_eq!(path.points.len(), 4);
    assert_eq!(path.points[0], (0.0, 0.0));
    assert_eq!(path.points[3], (10.0, 4.0));
    assert!(path.points[1].0 > 0.0);
    assert!(path.points[2].0 < 10.0);
}

#[test]
fn orthogonal_route_uses_four_polyline_vertices_without_radius() {
    let path =
        route(&request(RouteStyle::Orthogonal { corner_radius: 0.0 })).expect("orthogonal route");
    assert_eq!(path.kind, PathKind::Polyline);
    assert_eq!(path.points.len(), 4);
    assert_eq!(path.points[0], (0.0, 0.0));
    assert_eq!(path.points[3], (10.0, 4.0));
}
