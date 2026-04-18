use alloc::vec;

use crate::{cubic_control_points, distance, PathData, PathKind, RouteRequest};

pub(crate) fn route(req: &RouteRequest, curvature: f64) -> PathData {
    let handle_scale = distance(req.from, req.to) * curvature;
    let [p0, p1, p2, p3] = cubic_control_points(req, handle_scale);
    PathData {
        points: vec![p0, p1, p2, p3],
        kind: PathKind::Cubic,
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::*;
    use crate::RouteStyle;

    fn request(from: (f64, f64), to: (f64, f64)) -> RouteRequest {
        RouteRequest {
            from,
            to,
            from_tangent: (1.0, 0.0),
            to_tangent: (-1.0, 0.0),
            style: RouteStyle::Bezier { curvature: 0.25 },
        }
    }

    #[test]
    fn zero_curvature_degenerates_to_line_shape() {
        let path = route(&request((0.0, 0.0), (8.0, 2.0)), 0.0);
        assert_eq!(
            path.points,
            vec![(0.0, 0.0), (0.0, 0.0), (8.0, 2.0), (8.0, 2.0)]
        );
    }

    #[test]
    fn forward_tangents_push_handles_outward() {
        let path = route(&request((0.0, 0.0), (10.0, 0.0)), 0.25);
        assert_eq!(path.kind, PathKind::Cubic);
        assert!(path.points[1].0 > path.points[0].0);
        assert!(path.points[2].0 < path.points[3].0);
    }

    #[test]
    fn swapping_endpoints_reverses_curve() {
        let forward = route(&request((0.0, 0.0), (10.0, 4.0)), 0.25);
        let mut reverse_req = request((10.0, 4.0), (0.0, 0.0));
        reverse_req.from_tangent = (-1.0, 0.0);
        reverse_req.to_tangent = (1.0, 0.0);
        let reverse = route(&reverse_req, 0.25);
        let reversed_points: Vec<_> = reverse.points.into_iter().rev().collect();
        assert_eq!(forward.points, reversed_points);
    }
}
