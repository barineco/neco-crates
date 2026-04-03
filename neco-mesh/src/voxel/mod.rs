//! Structured voxel-grid utilities and analytical voxelization helpers.

pub mod grid;
pub mod solid;
pub mod surface;
pub mod wire;

pub use grid::{FillFractionGrid, OccupancyGrid, SpatialVoxelGrid, UniformGrid3, VoxelGrid};
pub use solid::{solid_occupancy, SolidOccupancyError};
pub use surface::surface_occupancy;
pub use wire::{GeometryConfig, RodGeometry, TriangleGeometry};
