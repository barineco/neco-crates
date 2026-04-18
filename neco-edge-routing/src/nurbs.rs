use alloc::vec;
use alloc::vec::Vec;

use neco_nurbs::NurbsCurve2D;

use crate::{feature_control_points, PathData, PathKind, RouteRequest, RoutingError};

pub(crate) fn route(req: &RouteRequest, degree: u32) -> Result<PathData, RoutingError> {
    let degree = clamp_degree(degree);
    let control_points = feature_control_points(req);
    let points_2d = control_points.map(|(x, y)| [x, y]);
    let knots = clamped_uniform_knots(degree, points_2d.len());
    let curve = NurbsCurve2D::new(degree, points_2d.to_vec(), knots);

    curve.validate().map_err(|_| RoutingError::InvalidInput {
        reason: "nurbs control points, weights, or knots are inconsistent",
    })?;

    Ok(PathData {
        points: curve
            .control_points
            .iter()
            .map(|point| (point[0], point[1]))
            .collect(),
        kind: PathKind::Nurbs {
            knots: curve.knots.clone(),
            weights: curve.weights.clone(),
        },
    })
}

fn clamp_degree(degree: u32) -> usize {
    let requested = usize::try_from(degree).unwrap_or(usize::MAX);
    requested.clamp(1, 3)
}

fn clamped_uniform_knots(degree: usize, control_point_count: usize) -> Vec<f64> {
    let knot_count = control_point_count + degree + 1;
    let interior_count = knot_count.saturating_sub((degree + 1) * 2);
    let mut knots = vec![0.0; degree + 1];
    for index in 1..=interior_count {
        knots.push(index as f64 / (interior_count + 1) as f64);
    }
    knots.extend(vec![1.0; degree + 1]);
    knots
}

#[cfg(test)]
mod tests {
    use super::route;
    use crate::{PathKind, RouteRequest, RouteStyle};

    fn request(degree: u32) -> RouteRequest {
        RouteRequest {
            from: (0.0, 0.0),
            to: (10.0, 4.0),
            from_tangent: (1.0, 0.0),
            to_tangent: (-1.0, 0.0),
            style: RouteStyle::Nurbs { degree },
        }
    }

    #[test]
    fn degree_three_emits_expected_control_data() {
        let path = route(&request(3), 3).expect("nurbs route");
        assert_eq!(path.points.len(), 4);
        match path.kind {
            PathKind::Nurbs { knots, weights } => {
                assert_eq!(knots.len(), 8);
                assert!(weights
                    .iter()
                    .all(|weight| (*weight - 1.0).abs() < f64::EPSILON));
            }
            other => panic!("unexpected kind: {other:?}"),
        }
    }

    #[test]
    fn tuple_array_tuple_conversion_is_lossless() {
        let path = route(&request(3), 3).expect("nurbs route");
        assert_eq!(path.points[0], (0.0, 0.0));
        assert_eq!(path.points[3], (10.0, 4.0));
    }

    #[test]
    fn degree_two_adjusts_knot_count() {
        let path = route(&request(2), 2).expect("quadratic nurbs route");
        match path.kind {
            PathKind::Nurbs { knots, .. } => assert_eq!(knots.len(), 7),
            other => panic!("unexpected kind: {other:?}"),
        }
    }

    #[test]
    fn curve_build_does_not_panic() {
        let path = route(&request(1), 1).expect("linear nurbs route");
        assert!(matches!(path.kind, PathKind::Nurbs { .. }));
    }
}
