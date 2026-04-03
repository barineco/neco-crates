//! 1D/2D/3D mesh generation library.

mod internal_mesh3d;
mod point3;
mod predicates;

pub mod immersed;
pub mod mesh1d;
pub mod mesh2d;
pub mod mesh3d;
pub mod types;
pub mod voxel;

pub use immersed::generate_immersed_mesh;
pub use mesh1d::{mesh_curve, mesh_line};
pub use mesh2d::{
    mesh_polygon, mesh_polygon_adaptive, mesh_rect, mesh_region, mesh_region_adaptive,
    point_in_polygon,
};
pub use mesh3d::generate_quality_mesh;
pub use mesh3d::refinement::RefinementParams;
pub use types::{EdgeMesh, ImmersedMesh, TetMesh3D, TriMesh2D};
pub use voxel::{
    solid_occupancy, surface_occupancy, FillFractionGrid, GeometryConfig, OccupancyGrid,
    RodGeometry, SolidOccupancyError, SpatialVoxelGrid, TriangleGeometry, UniformGrid3, VoxelGrid,
};
