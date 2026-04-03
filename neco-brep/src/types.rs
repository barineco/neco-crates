//! Shared type definitions.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub fn direction(&self) -> [f64; 3] {
        match self {
            Axis::X => [1.0, 0.0, 0.0],
            Axis::Y => [0.0, 1.0, 0.0],
            Axis::Z => [0.0, 0.0, 1.0],
        }
    }
}

/// Loft cross-section.
pub struct LoftSection {
    pub profile: neco_nurbs::NurbsRegion,
    /// 4x4 transform matrix.
    pub transform: [[f64; 4]; 4],
}

/// Loft connection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoftMode {
    /// Ruled surface.
    Straight,
    /// Cubic spline interpolation.
    Smooth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanOp {
    Union,
    Subtract,
    Intersect,
}

pub fn identity_matrix() -> [[f64; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

pub fn translation_matrix(dx: f64, dy: f64, dz: f64) -> [[f64; 4]; 4] {
    [
        [1.0, 0.0, 0.0, dx],
        [0.0, 1.0, 0.0, dy],
        [0.0, 0.0, 1.0, dz],
        [0.0, 0.0, 0.0, 1.0],
    ]
}
