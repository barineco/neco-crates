//! 2D Constrained Delaunay Triangulation and adaptive-precision geometric predicates.

mod cdt;
mod predicates;
mod robust_impl;

pub use cdt::{Cdt, CdtError};
pub use predicates::{incircle, insphere, orient2d, orient3d};
