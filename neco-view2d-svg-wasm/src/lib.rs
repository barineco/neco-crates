//! WebAssembly bindings for `neco-view2d-svg` via `wasm-bindgen`.

use neco_view2d::View2d;
use neco_view2d_svg::{world_points_to_polyline, world_points_to_svg_d, world_transform_attr};
use wasm_bindgen::prelude::*;

/// Emit an SVG `transform` attribute value for the given view and canvas size.
#[wasm_bindgen]
pub fn emit_transform(
    center_x: f64,
    center_y: f64,
    view_size: f64,
    canvas_w: f64,
    canvas_h: f64,
) -> String {
    let view = View2d {
        center_x,
        center_y,
        view_size,
    };
    world_transform_attr(&view, canvas_w, canvas_h)
}

/// Emit an SVG `polyline` `points` attribute from a flat `[x0, y0, x1, y1, ...]` array of world coordinates.
#[wasm_bindgen]
pub fn emit_polyline(
    center_x: f64,
    center_y: f64,
    view_size: f64,
    points: Vec<f64>,
    canvas_w: f64,
    canvas_h: f64,
) -> String {
    let view = View2d {
        center_x,
        center_y,
        view_size,
    };
    let pairs = decode_points(points);
    world_points_to_polyline(&view, &pairs, canvas_w, canvas_h)
}

/// Emit an SVG `path` `d` attribute from a flat `[x0, y0, x1, y1, ...]` array of world coordinates.
#[wasm_bindgen]
pub fn emit_path(
    center_x: f64,
    center_y: f64,
    view_size: f64,
    points: Vec<f64>,
    canvas_w: f64,
    canvas_h: f64,
) -> String {
    let view = View2d {
        center_x,
        center_y,
        view_size,
    };
    let pairs = decode_points(points);
    world_points_to_svg_d(&view, &pairs, canvas_w, canvas_h)
}

fn decode_points(points: Vec<f64>) -> Vec<(f64, f64)> {
    points
        .chunks(2)
        .filter_map(|chunk| chunk.first().zip(chunk.get(1)).map(|(x, y)| (*x, *y)))
        .collect()
}
