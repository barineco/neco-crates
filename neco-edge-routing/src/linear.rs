use alloc::vec;

use crate::{PathData, PathKind, RouteRequest};

pub(crate) fn route(req: &RouteRequest) -> PathData {
    PathData {
        points: vec![req.from, req.to],
        kind: PathKind::Polyline,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RouteStyle;

    fn request(from: (f64, f64), to: (f64, f64)) -> RouteRequest {
        RouteRequest {
            from,
            to,
            from_tangent: (1.0, 2.0),
            to_tangent: (-3.0, 4.0),
            style: RouteStyle::Linear,
        }
    }

    #[test]
    fn keeps_endpoints_for_general_case() {
        let path = route(&request((-2.0, 1.5), (3.0, -4.0)));
        assert_eq!(path.kind, PathKind::Polyline);
        assert_eq!(path.points, vec![(-2.0, 1.5), (3.0, -4.0)]);
    }

    #[test]
    fn keeps_duplicate_points_for_degenerate_case() {
        let path = route(&request((1.0, 1.0), (1.0, 1.0)));
        assert_eq!(path.points, vec![(1.0, 1.0), (1.0, 1.0)]);
    }

    #[test]
    fn ignores_tangents() {
        let mut req = request((0.0, 0.0), (5.0, 7.0));
        let first = route(&req);
        req.from_tangent = (0.0, 0.0);
        req.to_tangent = (100.0, -100.0);
        let second = route(&req);
        assert_eq!(first, second);
    }
}
