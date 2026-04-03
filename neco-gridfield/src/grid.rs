use crate::Array2;
use core::fmt;

#[derive(Debug, Clone)]
pub enum GridError {
    InvalidSpacing {
        dx: f64,
    },
    InvalidExtent {
        axis: &'static str,
        value: f64,
    },
    ResolutionOverflow {
        axis: &'static str,
        value: f64,
        dx: f64,
    },
    InvalidMaskShape {
        expected: (usize, usize),
        actual: (usize, usize),
    },
}

impl PartialEq for GridError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::InvalidSpacing { dx: a }, Self::InvalidSpacing { dx: b }) => {
                (a.is_nan() && b.is_nan()) || a == b
            }
            (
                Self::InvalidExtent {
                    axis: axis_a,
                    value: value_a,
                },
                Self::InvalidExtent {
                    axis: axis_b,
                    value: value_b,
                },
            ) => axis_a == axis_b && ((value_a.is_nan() && value_b.is_nan()) || value_a == value_b),
            (
                Self::ResolutionOverflow {
                    axis: axis_a,
                    value: value_a,
                    dx: dx_a,
                },
                Self::ResolutionOverflow {
                    axis: axis_b,
                    value: value_b,
                    dx: dx_b,
                },
            ) => {
                axis_a == axis_b
                    && ((value_a.is_nan() && value_b.is_nan()) || value_a == value_b)
                    && ((dx_a.is_nan() && dx_b.is_nan()) || dx_a == dx_b)
            }
            (
                Self::InvalidMaskShape {
                    expected: expected_a,
                    actual: actual_a,
                },
                Self::InvalidMaskShape {
                    expected: expected_b,
                    actual: actual_b,
                },
            ) => expected_a == expected_b && actual_a == actual_b,
            _ => false,
        }
    }
}

impl fmt::Display for GridError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSpacing { dx } => {
                write!(f, "grid spacing dx must be finite and positive, got {dx}")
            }
            Self::InvalidExtent { axis, value } => {
                write!(
                    f,
                    "grid extent {axis} must be finite and non-negative, got {value}"
                )
            }
            Self::ResolutionOverflow { axis, value, dx } => write!(
                f,
                "grid extent {axis}={value} with dx={dx} produces too many cells"
            ),
            Self::InvalidMaskShape { expected, actual } => write!(
                f,
                "mask shape must match grid shape {:?}, got {:?}",
                expected, actual
            ),
        }
    }
}

impl std::error::Error for GridError {}

/// 2D uniform square grid.
#[derive(Debug)]
pub struct Grid2D {
    lx: f64,
    ly: f64,
    dx: f64,
    nx: usize,
    ny: usize,
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", rename_all = "snake_case"))]
#[derive(Debug, Clone)]
pub enum BoundaryGeometry {
    Circular {
        r_outer: f64,
        r_hole: f64,
    },
    Rectangular,
    #[cfg_attr(feature = "serde", serde(skip))]
    Mask(Array2<bool>),
}

impl Grid2D {
    pub fn new(lx: f64, ly: f64, dx: f64) -> Result<Self, GridError> {
        if !dx.is_finite() || dx <= 0.0 {
            return Err(GridError::InvalidSpacing { dx });
        }
        validate_extent("lx", lx)?;
        validate_extent("ly", ly)?;
        let nx = cells_for_extent("lx", lx, dx)?;
        let ny = cells_for_extent("ly", ly, dx)?;
        Ok(Self { lx, ly, dx, nx, ny })
    }

    pub fn nx(&self) -> usize {
        self.nx
    }

    pub fn ny(&self) -> usize {
        self.ny
    }

    pub fn dx(&self) -> f64 {
        self.dx
    }

    pub fn lx(&self) -> f64 {
        self.lx
    }

    pub fn ly(&self) -> f64 {
        self.ly
    }

    /// Coordinate arrays (X, Y) with origin at grid center.
    pub fn coords(&self) -> (Array2<f64>, Array2<f64>) {
        let cx = (self.nx / 2) as f64 * self.dx;
        let cy = (self.ny / 2) as f64 * self.dx;
        let mut x = Array2::zeros((self.nx, self.ny));
        let mut y = Array2::zeros((self.nx, self.ny));
        for i in 0..self.nx {
            for j in 0..self.ny {
                x[[i, j]] = i as f64 * self.dx - cx;
                y[[i, j]] = j as f64 * self.dx - cy;
            }
        }
        (x, y)
    }

    /// Radius map from grid center.
    pub fn radius_map(&self) -> Array2<f64> {
        let (x, y) = self.coords();
        let mut r = Array2::zeros((self.nx, self.ny));
        for i in 0..self.nx {
            for j in 0..self.ny {
                r[[i, j]] = (x[[i, j]].powi(2) + y[[i, j]].powi(2)).sqrt();
            }
        }
        r
    }

    /// Interior mask (true = active computational cell).
    pub fn interior_mask(&self, geom: &BoundaryGeometry) -> Result<Array2<bool>, GridError> {
        match geom {
            BoundaryGeometry::Circular { r_outer, r_hole } => {
                let r = self.radius_map();
                let mut mask = Array2::from_elem((self.nx, self.ny), false);
                for i in 0..self.nx {
                    for j in 0..self.ny {
                        let rv = r[[i, j]];
                        mask[[i, j]] = rv < *r_outer && rv > *r_hole;
                    }
                }
                Ok(mask)
            }
            BoundaryGeometry::Mask(mask) => {
                let actual = (mask.nrows(), mask.ncols());
                let expected = (self.nx, self.ny);
                if actual != expected {
                    return Err(GridError::InvalidMaskShape { expected, actual });
                }
                Ok(mask.clone())
            }
            BoundaryGeometry::Rectangular => {
                let mut mask = Array2::from_elem((self.nx, self.ny), true);
                let x_border = self.nx.saturating_sub(2);
                let y_border = self.ny.saturating_sub(2);
                for i in 0..self.nx {
                    for j in 0..self.ny {
                        if i < 2 || i >= x_border || j < 2 || j >= y_border {
                            mask[[i, j]] = false;
                        }
                    }
                }
                Ok(mask)
            }
        }
    }
}

fn validate_extent(axis: &'static str, value: f64) -> Result<(), GridError> {
    if !value.is_finite() || value < 0.0 {
        return Err(GridError::InvalidExtent { axis, value });
    }
    Ok(())
}

fn cells_for_extent(axis: &'static str, value: f64, dx: f64) -> Result<usize, GridError> {
    let cells = (value / dx).round();
    if !cells.is_finite() || cells < 0.0 || cells > (usize::MAX - 1) as f64 {
        return Err(GridError::ResolutionOverflow { axis, value, dx });
    }
    Ok(cells as usize + 1)
}
