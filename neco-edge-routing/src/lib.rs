#![no_std]

//! necosystems series 2D edge routing primitives for node graphs.

extern crate alloc;

mod bezier;
mod error;
mod linear;
#[cfg(feature = "nurbs")]
mod nurbs;
mod orthogonal;
#[cfg(feature = "spline")]
mod spline;

use alloc::vec::Vec;

pub use error::RoutingError;

const EPSILON: f64 = 1e-9;
#[cfg(any(feature = "spline", feature = "nurbs"))]
const FEATURE_HANDLE_RATIO: f64 = 0.25;

/// Edge routing strategy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RouteStyle {
    /// Direct line segment from `from` to `to`.
    Linear,
    /// Cubic Bezier route using tangent-scaled handles.
    Bezier {
        /// Multiplier applied to `distance(from, to)` for both handles.
        curvature: f64,
    },
    /// Axis-aligned route with optional rounded corners.
    Orthogonal {
        /// Corner radius. Values larger than half the local segment length are clamped.
        corner_radius: f64,
    },
    /// Natural cubic spline route. Requires the `spline` feature.
    Spline,
    /// NURBS control path. Requires the `nurbs` feature.
    Nurbs {
        /// Requested degree. The implementation clamps this into the valid range.
        degree: u32,
    },
}

/// Input for a routing pass.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteRequest {
    /// Route start point.
    pub from: (f64, f64),
    /// Route end point.
    pub to: (f64, f64),
    /// Tangent direction that leaves `from`.
    pub from_tangent: (f64, f64),
    /// Tangent direction that approaches `to` from the incoming side.
    pub to_tangent: (f64, f64),
    /// Requested route style.
    pub style: RouteStyle,
}

/// Routed path data.
#[derive(Debug, Clone, PartialEq)]
pub struct PathData {
    /// Control-point layout depends on `kind`.
    pub points: Vec<(f64, f64)>,
    /// Semantic interpretation of `points`.
    pub kind: PathKind,
}

/// Semantic shape of `PathData.points`.
#[derive(Debug, Clone, PartialEq)]
pub enum PathKind {
    /// Polyline vertices such as `from, bend..., to`.
    Polyline,
    /// Cubic segments flattened as `P0, C1, C2, P1` per segment.
    Cubic,
    /// Rounded orthogonal path flattened as `from, pre, corner, post, ..., to`.
    Quadratic,
    /// NURBS control points plus knot and weight data.
    Nurbs {
        /// Knot vector aligned with `points`.
        knots: Vec<f64>,
        /// Weight vector aligned with `points`.
        weights: Vec<f64>,
    },
}

/// Compute an edge route without rendering concerns.
pub fn route(req: &RouteRequest) -> Result<PathData, RoutingError> {
    validate_request(req)?;
    match req.style {
        RouteStyle::Linear => Ok(linear::route(req)),
        RouteStyle::Bezier { curvature } => Ok(bezier::route(req, curvature)),
        RouteStyle::Orthogonal { corner_radius } => Ok(orthogonal::route(req, corner_radius)),
        RouteStyle::Spline => {
            #[cfg(feature = "spline")]
            {
                spline::route(req)
            }
            #[cfg(not(feature = "spline"))]
            {
                Err(RoutingError::FeatureDisabled { style: "Spline" })
            }
        }
        RouteStyle::Nurbs { degree } => {
            #[cfg(feature = "nurbs")]
            {
                nurbs::route(req, degree)
            }
            #[cfg(not(feature = "nurbs"))]
            {
                let _ = degree;
                Err(RoutingError::FeatureDisabled { style: "Nurbs" })
            }
        }
    }
}

fn validate_request(req: &RouteRequest) -> Result<(), RoutingError> {
    for value in [
        req.from.0,
        req.from.1,
        req.to.0,
        req.to.1,
        req.from_tangent.0,
        req.from_tangent.1,
        req.to_tangent.0,
        req.to_tangent.1,
    ] {
        if !value.is_finite() {
            return Err(RoutingError::InvalidInput {
                reason: "route request contains non-finite coordinates or tangents",
            });
        }
    }

    match req.style {
        RouteStyle::Bezier { curvature } if !curvature.is_finite() => {
            Err(RoutingError::InvalidInput {
                reason: "bezier curvature must be finite",
            })
        }
        RouteStyle::Orthogonal { corner_radius } if !corner_radius.is_finite() => {
            Err(RoutingError::InvalidInput {
                reason: "orthogonal corner radius must be finite",
            })
        }
        _ => Ok(()),
    }
}

pub(crate) fn add(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 + b.0, a.1 + b.1)
}

pub(crate) fn sub(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 - b.0, a.1 - b.1)
}

pub(crate) fn scale(v: (f64, f64), factor: f64) -> (f64, f64) {
    (v.0 * factor, v.1 * factor)
}

pub(crate) fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    let d = sub(b, a);
    sqrt(d.0 * d.0 + d.1 * d.1)
}

pub(crate) fn length(v: (f64, f64)) -> f64 {
    sqrt(v.0 * v.0 + v.1 * v.1)
}

fn sqrt(value: f64) -> f64 {
    if value <= 0.0 {
        return 0.0;
    }

    let mut x = if value >= 1.0 { value } else { 1.0 };
    for _ in 0..16 {
        x = 0.5 * (x + value / x);
    }
    x
}

pub(crate) fn is_degenerate_segment(a: (f64, f64), b: (f64, f64)) -> bool {
    distance(a, b) <= EPSILON
}

pub(crate) fn is_zero_tangent(v: (f64, f64)) -> bool {
    length(v) <= EPSILON
}

pub(crate) fn cubic_control_points(req: &RouteRequest, handle_scale: f64) -> [(f64, f64); 4] {
    [
        req.from,
        add(req.from, scale(req.from_tangent, handle_scale)),
        add(req.to, scale(req.to_tangent, handle_scale)),
        req.to,
    ]
}

#[cfg(any(feature = "spline", feature = "nurbs"))]
pub(crate) fn feature_control_points(req: &RouteRequest) -> [(f64, f64); 4] {
    let handle_scale = distance(req.from, req.to) * FEATURE_HANDLE_RATIO;
    cubic_control_points(req, handle_scale)
}
