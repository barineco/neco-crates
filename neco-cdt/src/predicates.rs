//! Shewchuk adaptive-precision geometric predicates.
//!
//! Prevents topological errors caused by floating-point rounding.
//! Coordinates remain f64; only predicate sign evaluation is made exact.

use crate::robust_impl::{self, Coord, Coord3D};

/// 2D orientation test. Positive = CCW, negative = CW, zero = collinear.
#[inline]
pub fn orient2d(pa: [f64; 2], pb: [f64; 2], pc: [f64; 2]) -> f64 {
    robust_impl::orient2d(
        Coord { x: pa[0], y: pa[1] },
        Coord { x: pb[0], y: pb[1] },
        Coord { x: pc[0], y: pc[1] },
    )
}

/// 3D orientation test. Positive = pd is below the abc plane.
#[inline]
pub fn orient3d(pa: [f64; 3], pb: [f64; 3], pc: [f64; 3], pd: [f64; 3]) -> f64 {
    robust_impl::orient3d(
        Coord3D {
            x: pa[0],
            y: pa[1],
            z: pa[2],
        },
        Coord3D {
            x: pb[0],
            y: pb[1],
            z: pb[2],
        },
        Coord3D {
            x: pc[0],
            y: pc[1],
            z: pc[2],
        },
        Coord3D {
            x: pd[0],
            y: pd[1],
            z: pd[2],
        },
    )
}

/// Insphere test for 5 points. Positive = pe is inside the circumsphere of (pa,pb,pc,pd).
/// pa,pb,pc,pd must have positive orientation (orient3d > 0).
#[inline]
pub fn insphere(pa: [f64; 3], pb: [f64; 3], pc: [f64; 3], pd: [f64; 3], pe: [f64; 3]) -> f64 {
    robust_impl::insphere(
        Coord3D {
            x: pa[0],
            y: pa[1],
            z: pa[2],
        },
        Coord3D {
            x: pb[0],
            y: pb[1],
            z: pb[2],
        },
        Coord3D {
            x: pc[0],
            y: pc[1],
            z: pc[2],
        },
        Coord3D {
            x: pd[0],
            y: pd[1],
            z: pd[2],
        },
        Coord3D {
            x: pe[0],
            y: pe[1],
            z: pe[2],
        },
    )
}

/// 2D incircle test. Positive = pd is inside the circumcircle of (pa,pb,pc).
/// pa,pb,pc must be in CCW order.
#[inline]
pub fn incircle(pa: [f64; 2], pb: [f64; 2], pc: [f64; 2], pd: [f64; 2]) -> f64 {
    robust_impl::incircle(
        Coord { x: pa[0], y: pa[1] },
        Coord { x: pb[0], y: pb[1] },
        Coord { x: pc[0], y: pc[1] },
        Coord { x: pd[0], y: pd[1] },
    )
}
