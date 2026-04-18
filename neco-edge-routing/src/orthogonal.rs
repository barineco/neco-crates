use alloc::vec;
use alloc::vec::Vec;

use crate::{is_degenerate_segment, is_zero_tangent, linear, PathData, PathKind, RouteRequest};

pub(crate) fn route(req: &RouteRequest, corner_radius: f64) -> PathData {
    if is_degenerate_segment(req.from, req.to) || is_zero_tangent(req.from_tangent) {
        return linear::route(req);
    }

    let bends = if req.from_tangent.0.abs() >= req.from_tangent.1.abs() {
        horizontal_first(req)
    } else {
        vertical_first(req)
    };

    if corner_radius <= 0.0 {
        return PathData {
            points: vec![req.from, bends[0], bends[1], req.to],
            kind: PathKind::Polyline,
        };
    }

    let mut points = Vec::with_capacity(8);
    points.push(req.from);

    let first = rounded_corner(req.from, bends[0], bends[1], corner_radius);
    points.push(first.0);
    points.push(bends[0]);
    points.push(first.1);

    let second = rounded_corner(bends[0], bends[1], req.to, corner_radius);
    points.push(second.0);
    points.push(bends[1]);
    points.push(second.1);
    points.push(req.to);

    PathData {
        points,
        kind: PathKind::Quadratic,
    }
}

fn horizontal_first(req: &RouteRequest) -> [(f64, f64); 2] {
    let mid_x = (req.from.0 + req.to.0) * 0.5;
    [(mid_x, req.from.1), (mid_x, req.to.1)]
}

fn vertical_first(req: &RouteRequest) -> [(f64, f64); 2] {
    let mid_y = (req.from.1 + req.to.1) * 0.5;
    [(req.from.0, mid_y), (req.to.0, mid_y)]
}

fn rounded_corner(
    prev: (f64, f64),
    corner: (f64, f64),
    next: (f64, f64),
    corner_radius: f64,
) -> ((f64, f64), (f64, f64)) {
    let prev_len = axis_distance(prev, corner);
    let next_len = axis_distance(corner, next);
    let radius = corner_radius.min(prev_len * 0.5).min(next_len * 0.5);
    let before = move_towards(corner, prev, radius);
    let after = move_towards(corner, next, radius);
    (before, after)
}

fn axis_distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).abs() + (a.1 - b.1).abs()
}

fn move_towards(from: (f64, f64), to: (f64, f64), amount: f64) -> (f64, f64) {
    if (from.0 - to.0).abs() > (from.1 - to.1).abs() {
        let sign = if to.0 >= from.0 { 1.0 } else { -1.0 };
        (from.0 + sign * amount, from.1)
    } else {
        let sign = if to.1 >= from.1 { 1.0 } else { -1.0 };
        (from.0, from.1 + sign * amount)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RouteStyle;

    fn request(from_tangent: (f64, f64), corner_radius: f64) -> RouteRequest {
        RouteRequest {
            from: (0.0, 0.0),
            to: (8.0, 6.0),
            from_tangent,
            to_tangent: (-1.0, 0.0),
            style: RouteStyle::Orthogonal { corner_radius },
        }
    }

    #[test]
    fn horizontal_priority_builds_hvh_polyline() {
        let path = route(&request((1.0, 0.0), 0.0), 0.0);
        assert_eq!(path.kind, PathKind::Polyline);
        assert_eq!(
            path.points,
            vec![(0.0, 0.0), (4.0, 0.0), (4.0, 6.0), (8.0, 6.0)]
        );
    }

    #[test]
    fn vertical_priority_builds_vhv_polyline() {
        let path = route(&request((0.0, 1.0), 0.0), 0.0);
        assert_eq!(path.kind, PathKind::Polyline);
        assert_eq!(
            path.points,
            vec![(0.0, 0.0), (0.0, 3.0), (8.0, 3.0), (8.0, 6.0)]
        );
    }

    #[test]
    fn rounded_corners_emit_quadratic_layout() {
        let path = route(&request((1.0, 0.0), 1.0), 1.0);
        assert_eq!(path.kind, PathKind::Quadratic);
        assert_eq!(path.points.first().copied(), Some((0.0, 0.0)));
        assert_eq!(path.points.last().copied(), Some((8.0, 6.0)));
        assert_eq!(path.points.len(), 8);
    }

    #[test]
    fn radius_is_clamped_to_half_segment_length() {
        let path = route(&request((1.0, 0.0), 10.0), 10.0);
        assert_eq!(path.points[1], (2.0, 0.0));
        assert_eq!(path.points[3], (4.0, 2.0));
    }

    #[test]
    fn zero_tangent_falls_back_to_linear() {
        let path = route(&request((0.0, 0.0), 0.0), 0.0);
        assert_eq!(path.kind, PathKind::Polyline);
        assert_eq!(path.points, vec![(0.0, 0.0), (8.0, 6.0)]);
    }
}
