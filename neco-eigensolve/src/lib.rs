//! Lightweight solvers for generalized eigenvalue problems.

pub mod c64;
pub mod chefsi;
pub mod cms;
mod dense;
mod dense_block;
pub mod feast;
pub mod ic0;
mod lobpcg;
pub mod rng;
pub(crate) mod spmv;

pub use c64::C64;
pub use dense_block::DenseMatrix;
pub use feast::contour::{ContourPoint, FeastInterval};
pub use feast::{feast_solve_interval, FeastConfig, FeastIntervalResult, FeastIterationInfo};
pub use ic0::Ic0Preconditioner;
pub use lobpcg::{
    lobpcg, lobpcg_configured, lobpcg_with_progress, JacobiPreconditioner, LobpcgConfig,
    LobpcgResult, Preconditioner,
};
