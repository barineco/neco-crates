//! 2D/3D rational NURBS curves, surfaces, and region representation.

mod deboor;
mod nurbs;
pub mod polynomial;

pub use nurbs::{dedup_piecewise_sample, NurbsCurve2D, NurbsRegion};
pub use polynomial::quartic::{solve_cubic, solve_quadratic, solve_quartic};
pub use polynomial::{eval_poly, newton_refine, solve_polynomial, PolynomialError};

mod curve3d;
mod surface3d;

pub use curve3d::NurbsCurve3D;
pub use surface3d::NurbsSurface3D;

#[cfg(feature = "fitting")]
mod fitting;
#[cfg(feature = "fitting")]
pub use fitting::{fit_nurbs_curve, NurbsFitError};
