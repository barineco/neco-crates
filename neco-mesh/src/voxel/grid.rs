/// A structured voxel grid stored as a flat `(i, j, k)`-indexed buffer.
#[derive(Debug, Clone, PartialEq)]
pub struct VoxelGrid<T> {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub values: Vec<T>,
}

impl<T> VoxelGrid<T> {
    #[inline]
    pub fn index(&self, i: usize, j: usize, k: usize) -> usize {
        (i * self.ny + j) * self.nz + k
    }

    #[inline]
    pub fn get_ref(&self, i: usize, j: usize, k: usize) -> &T {
        &self.values[self.index(i, j, k)]
    }
}

impl<T: Copy> VoxelGrid<T> {
    #[inline]
    pub fn get(&self, i: usize, j: usize, k: usize) -> T {
        self.values[self.index(i, j, k)]
    }
}

/// Fill-fraction field on a structured voxel grid.
pub type FillFractionGrid = VoxelGrid<f64>;

/// Binary occupancy field on a structured voxel grid.
pub type OccupancyGrid = VoxelGrid<bool>;

/// Spatial metadata for a uniform structured grid.
///
/// `origin + [i, j, k] * spacing` is the world-coordinate grid point
/// represented by the corresponding voxel-grid index.
#[derive(Debug, Clone, PartialEq)]
pub struct UniformGrid3 {
    pub origin: [f64; 3],
    pub spacing: [f64; 3],
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
}

impl UniformGrid3 {
    /// Return the world-coordinate grid point represented by `(i, j, k)`.
    #[inline]
    pub fn point(&self, i: usize, j: usize, k: usize) -> [f64; 3] {
        [
            self.origin[0] + i as f64 * self.spacing[0],
            self.origin[1] + j as f64 * self.spacing[1],
            self.origin[2] + k as f64 * self.spacing[2],
        ]
    }
}

/// A voxel grid together with the world-coordinate grid points used for sampling.
#[derive(Debug, Clone, PartialEq)]
pub struct SpatialVoxelGrid<T> {
    pub grid: VoxelGrid<T>,
    pub layout: UniformGrid3,
}
