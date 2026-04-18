use alloc::vec;

use neco_spline::CubicSpline;

use crate::{
    feature_control_points, is_degenerate_segment, linear, PathData, PathKind, RouteRequest,
    RoutingError,
};

/// Route with cubic spline-backed Bezier segments.
///
/// The implementation narrows intermediate control coordinates to `f32` because
/// `neco-spline` currently exposes `CubicSpline` in `f32`. The returned points are
/// widened back to `f64` after `to_bezier_segments()` conversion.
pub(crate) fn route(req: &RouteRequest) -> Result<PathData, RoutingError> {
    if is_degenerate_segment(req.from, req.to) {
        return Ok(linear::route(req));
    }

    let [p0, p1, p2, p3] = feature_control_points(req);
    let params = [0.0_f32, 1.0_f32 / 3.0_f32, 2.0_f32 / 3.0_f32, 1.0_f32];
    let xs = [p0.0, p1.0, p2.0, p3.0];
    let ys = [p0.1, p1.1, p2.1, p3.1];

    let x_points = control_series(&params, &xs);
    let y_points = control_series(&params, &ys);

    let spline_x = CubicSpline::new(&x_points).map_err(|_| RoutingError::InvalidInput {
        reason: "spline x control points must be strictly ascending in parameter space",
    })?;
    let spline_y = CubicSpline::new(&y_points).map_err(|_| RoutingError::InvalidInput {
        reason: "spline y control points must be strictly ascending in parameter space",
    })?;

    let x_segments = spline_x.to_bezier_segments();
    let y_segments = spline_y.to_bezier_segments();
    if x_segments.len() != y_segments.len() {
        return Err(RoutingError::InvalidInput {
            reason: "spline axis conversion produced mismatched segment counts",
        });
    }

    let mut points = vec![];
    for (x_segment, y_segment) in x_segments.iter().zip(y_segments.iter()) {
        points.push((f64::from(x_segment.p0.1), f64::from(y_segment.p0.1)));
        points.push((f64::from(x_segment.p1.1), f64::from(y_segment.p1.1)));
        points.push((f64::from(x_segment.p2.1), f64::from(y_segment.p2.1)));
        points.push((f64::from(x_segment.p3.1), f64::from(y_segment.p3.1)));
    }

    Ok(PathData {
        points,
        kind: PathKind::Cubic,
    })
}

fn control_series(params: &[f32; 4], values: &[f64; 4]) -> [(f32, f32); 4] {
    [
        (params[0], values[0] as f32),
        (params[1], values[1] as f32),
        (params[2], values[2] as f32),
        (params[3], values[3] as f32),
    ]
}

#[cfg(test)]
mod tests {
    use super::route;
    use crate::{PathKind, RouteRequest, RouteStyle};
    use alloc::vec;

    const EPS: f64 = 1e-3;

    fn request(from: (f64, f64), to: (f64, f64), from_tangent: (f64, f64)) -> RouteRequest {
        RouteRequest {
            from,
            to,
            from_tangent,
            to_tangent: (-1.0, 0.0),
            style: RouteStyle::Spline,
        }
    }

    #[test]
    fn general_case_emits_cubic_segments() {
        let path = route(&request((0.0, 0.0), (10.0, 4.0), (1.0, 0.0))).expect("spline route");
        assert_eq!(path.kind, PathKind::Cubic);
        assert_eq!(path.points.first().copied(), Some((0.0, 0.0)));
        let end = path.points.last().copied().expect("last point");
        assert!((end.0 - 10.0).abs() < EPS);
        assert!((end.1 - 4.0).abs() < EPS);
    }

    #[test]
    fn degenerate_segment_falls_back_to_linear_shape() {
        let path = route(&request((2.0, -1.0), (2.0, -1.0), (1.0, 0.0))).expect("degenerate");
        assert_eq!(path.kind, PathKind::Polyline);
        assert_eq!(path.points, vec![(2.0, -1.0), (2.0, -1.0)]);
    }

    #[test]
    fn zero_tangent_still_builds_curve() {
        let path = route(&request((0.0, 0.0), (8.0, 3.0), (0.0, 0.0))).expect("zero tangent");
        assert_eq!(path.kind, PathKind::Cubic);
        assert_eq!(path.points.len(), 12);
        assert_eq!(path.points.first().copied(), Some((0.0, 0.0)));
        let end = path.points.last().copied().expect("last point");
        assert!((end.0 - 8.0).abs() < EPS);
        assert!((end.1 - 3.0).abs() < EPS);
    }
}
