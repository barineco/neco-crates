pub use neco_cdt::{insphere, orient3d};

use crate::point3::Point3;

#[inline]
pub fn p3(p: &Point3) -> [f64; 3] {
    (*p).into()
}
