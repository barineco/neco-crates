use super::grid::{OccupancyGrid, SpatialVoxelGrid, UniformGrid3};

/// Rasterize a triangle surface into a structured binary occupancy grid.
///
/// Each `(i, j, k)` entry stores the occupancy result evaluated at the
/// world-coordinate grid point `layout.point(i, j, k)`.
pub fn surface_occupancy(
    surface_nodes: &[[f64; 3]],
    surface_triangles: &[[usize; 3]],
    max_edge: f64,
) -> SpatialVoxelGrid<bool> {
    let pad = max_edge;
    let (mut min, mut max) = bounding_box(surface_nodes);
    min[0] -= pad;
    min[1] -= pad;
    min[2] -= pad;
    max[0] += pad;
    max[1] += pad;
    max[2] += pad;

    let lx = max[0] - min[0];
    let ly = max[1] - min[1];
    let lz = max[2] - min[2];

    let nx = (lx / max_edge).ceil() as usize + 1;
    let ny = (ly / max_edge).ceil() as usize + 1;
    let nz = (lz / max_edge).ceil() as usize + 1;
    let dx = lx / (nx - 1).max(1) as f64;
    let dy = ly / (ny - 1).max(1) as f64;
    let dz = lz / (nz - 1).max(1) as f64;

    let mut values = Vec::with_capacity(nx * ny * nz);
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let point = [
                    min[0] + i as f64 * dx,
                    min[1] + j as f64 * dy,
                    min[2] + k as f64 * dz,
                ];
                values.push(is_inside_raycast(point, surface_nodes, surface_triangles));
            }
        }
    }

    SpatialVoxelGrid {
        grid: OccupancyGrid { nx, ny, nz, values },
        layout: UniformGrid3 {
            origin: min,
            spacing: [dx, dy, dz],
            nx,
            ny,
            nz,
        },
    }
}

fn bounding_box(nodes: &[[f64; 3]]) -> ([f64; 3], [f64; 3]) {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for point in nodes {
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
    }
    (min, max)
}

fn is_inside_raycast(
    point: [f64; 3],
    surface_nodes: &[[f64; 3]],
    surface_triangles: &[[usize; 3]],
) -> bool {
    let mut votes = 0u32;
    if raycast_axis(point, surface_nodes, surface_triangles, 2) {
        votes += 1;
    }
    if raycast_axis(point, surface_nodes, surface_triangles, 1) {
        votes += 1;
    }
    if raycast_axis(point, surface_nodes, surface_triangles, 0) {
        votes += 1;
    }
    votes >= 2
}

fn raycast_axis(
    point: [f64; 3],
    surface_nodes: &[[f64; 3]],
    surface_triangles: &[[usize; 3]],
    axis: usize,
) -> bool {
    let u_axis = (axis + 1) % 3;
    let v_axis = (axis + 2) % 3;
    let mut crossings = 0u32;

    for tri in surface_triangles {
        let va = surface_nodes[tri[0]];
        let vb = surface_nodes[tri[1]];
        let vc = surface_nodes[tri[2]];

        let d1u = vb[u_axis] - va[u_axis];
        let d1v = vb[v_axis] - va[v_axis];
        let d2u = vc[u_axis] - va[u_axis];
        let d2v = vc[v_axis] - va[v_axis];
        let det = d1u * d2v - d2u * d1v;
        if det.abs() < 1e-20 {
            continue;
        }

        let dpu = point[u_axis] - va[u_axis];
        let dpv = point[v_axis] - va[v_axis];
        let u_num = dpu * d2v - d2u * dpv;
        let v_num = d1u * dpv - dpu * d1v;

        if det > 0.0 {
            if u_num < 0.0 || v_num <= 0.0 || u_num + v_num >= det {
                continue;
            }
        } else if u_num > 0.0 || v_num >= 0.0 || u_num + v_num <= det {
            continue;
        }

        let hit = va[axis]
            + (u_num / det) * (vb[axis] - va[axis])
            + (v_num / det) * (vc[axis] - va[axis]);
        if hit > point[axis] {
            crossings += 1;
        }
    }

    crossings % 2 == 1
}
