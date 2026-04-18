#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![no_std]

//! necosystems series SVG attribute emitters for neco-view2d world coordinates.

extern crate alloc;

mod fmt;
mod path;
mod polyline;
mod svg_coord;
mod transform;

pub use path::world_points_to_svg_d;
pub use polyline::world_points_to_polyline;
pub use transform::world_transform_attr;
