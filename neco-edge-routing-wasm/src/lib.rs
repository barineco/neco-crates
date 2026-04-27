//! WebAssembly bindings for `neco-edge-routing` via `wasm-bindgen`.

use js_sys::{Array, Object, Reflect};
use neco_edge_routing::{route, PathKind, RouteRequest, RouteStyle};
use wasm_bindgen::prelude::*;

/// Route an edge between two points using the given style name.
#[wasm_bindgen]
pub fn route_edge(
    style: &str,
    from_x: f64,
    from_y: f64,
    to_x: f64,
    to_y: f64,
) -> Result<JsValue, JsValue> {
    let request = RouteRequest {
        from: (from_x, from_y),
        to: (to_x, to_y),
        from_tangent: (1.0, 0.0),
        to_tangent: (-1.0, 0.0),
        style: parse_style(style)?,
    };

    let path = route(&request).map_err(|error| JsValue::from_str(&error.to_string()))?;
    path_to_js(style, path)
}

fn parse_style(style: &str) -> Result<RouteStyle, JsValue> {
    match style {
        "bezier" => Ok(RouteStyle::Bezier { curvature: 0.25 }),
        "orthogonal" => Ok(RouteStyle::Orthogonal {
            corner_radius: 16.0,
        }),
        "spline" => Ok(RouteStyle::Spline),
        "nurbs" => Ok(RouteStyle::Nurbs { degree: 3 }),
        other => Err(JsValue::from_str(&format!(
            "unsupported route style: {other}"
        ))),
    }
}

fn path_to_js(style: &str, path: neco_edge_routing::PathData) -> Result<JsValue, JsValue> {
    let object = Object::new();
    Reflect::set(
        &object,
        &JsValue::from_str("style"),
        &JsValue::from_str(style),
    )?;
    Reflect::set(
        &object,
        &JsValue::from_str("kind"),
        &JsValue::from_str(kind_name(&path.kind)),
    )?;

    let points = Array::new();
    for (x, y) in path.points {
        let point = Object::new();
        Reflect::set(&point, &JsValue::from_str("x"), &JsValue::from_f64(x))?;
        Reflect::set(&point, &JsValue::from_str("y"), &JsValue::from_f64(y))?;
        points.push(&point);
    }
    Reflect::set(&object, &JsValue::from_str("points"), &points)?;

    if let PathKind::Nurbs { knots, weights } = path.kind {
        Reflect::set(&object, &JsValue::from_str("knots"), &vec_to_js(knots))?;
        Reflect::set(&object, &JsValue::from_str("weights"), &vec_to_js(weights))?;
    }

    Ok(JsValue::from(object))
}

fn vec_to_js(values: Vec<f64>) -> JsValue {
    let array = Array::new();
    for value in values {
        array.push(&JsValue::from_f64(value));
    }
    JsValue::from(array)
}

fn kind_name(kind: &PathKind) -> &'static str {
    match kind {
        PathKind::Polyline => "polyline",
        PathKind::Cubic => "cubic",
        PathKind::Quadratic => "quadratic",
        PathKind::Nurbs { .. } => "nurbs",
    }
}
