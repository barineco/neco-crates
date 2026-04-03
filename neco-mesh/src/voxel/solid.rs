use std::collections::BTreeMap;

use super::grid::SpatialVoxelGrid;
use super::surface::surface_occupancy;

/// Errors returned by `solid_occupancy(...)` when the input is not a valid
/// closed triangle surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SolidOccupancyError {
    EmptySurface,
    TriangleIndexOutOfBounds {
        triangle: usize,
        vertex: usize,
        node_count: usize,
    },
    BoundaryEdge {
        edge: [usize; 2],
        count: usize,
    },
    NonManifoldEdge {
        edge: [usize; 2],
        count: usize,
    },
}

/// Rasterize a closed triangle surface into a structured binary occupancy grid.
///
/// Each `(i, j, k)` entry stores the occupancy result evaluated at the
/// world-coordinate grid point `layout.point(i, j, k)`.
///
/// This entry is intended for closed triangle surfaces that represent a solid
/// boundary. The current implementation shares the same sampling path as
/// `surface_occupancy(...)`.
///
/// Returns an error when the input is empty, references out-of-bounds node
/// indices, or has edges that are not used exactly twice.
pub fn solid_occupancy(
    surface_nodes: &[[f64; 3]],
    surface_triangles: &[[usize; 3]],
    max_edge: f64,
) -> Result<SpatialVoxelGrid<bool>, SolidOccupancyError> {
    validate_closed_triangle_surface(surface_nodes, surface_triangles)?;
    Ok(surface_occupancy(
        surface_nodes,
        surface_triangles,
        max_edge,
    ))
}

fn validate_closed_triangle_surface(
    surface_nodes: &[[f64; 3]],
    surface_triangles: &[[usize; 3]],
) -> Result<(), SolidOccupancyError> {
    if surface_nodes.is_empty() || surface_triangles.is_empty() {
        return Err(SolidOccupancyError::EmptySurface);
    }

    let mut edge_counts = BTreeMap::<(usize, usize), usize>::new();
    for (triangle_index, triangle) in surface_triangles.iter().enumerate() {
        for &vertex in triangle {
            if vertex >= surface_nodes.len() {
                return Err(SolidOccupancyError::TriangleIndexOutOfBounds {
                    triangle: triangle_index,
                    vertex,
                    node_count: surface_nodes.len(),
                });
            }
        }

        for edge in [
            sorted_edge(triangle[0], triangle[1]),
            sorted_edge(triangle[1], triangle[2]),
            sorted_edge(triangle[2], triangle[0]),
        ] {
            *edge_counts.entry(edge).or_insert(0) += 1;
        }
    }

    for ((a, b), count) in edge_counts {
        if count == 2 {
            continue;
        }
        let edge = [a, b];
        if count < 2 {
            return Err(SolidOccupancyError::BoundaryEdge { edge, count });
        }
        return Err(SolidOccupancyError::NonManifoldEdge { edge, count });
    }

    Ok(())
}

fn sorted_edge(a: usize, b: usize) -> (usize, usize) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}
