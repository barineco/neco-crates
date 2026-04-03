//! Segment selection and loop construction

use super::classify::{Location, OverlapClass};
use super::dist2;
use crate::types::BooleanOp;
use neco_nurbs::NurbsCurve2D;

/// Select segments according to the boolean operation type.
pub fn select_segments(
    op: BooleanOp,
    segs_a: &[NurbsCurve2D],
    locs_a: &[Location],
    segs_b: &[NurbsCurve2D],
    locs_b: &[Location],
) -> Vec<NurbsCurve2D> {
    let mut selected = Vec::new();

    match op {
        BooleanOp::Union => {
            for (seg, loc) in segs_a.iter().zip(locs_a.iter()) {
                match loc {
                    Location::Outside => selected.push(seg.clone()),
                    Location::Boundary(OverlapClass::SameDirection) => selected.push(seg.clone()),
                    _ => {}
                }
            }
            for (seg, loc) in segs_b.iter().zip(locs_b.iter()) {
                if *loc == Location::Outside {
                    selected.push(seg.clone());
                }
            }
        }
        BooleanOp::Subtract => {
            for (seg, loc) in segs_a.iter().zip(locs_a.iter()) {
                match loc {
                    Location::Outside => selected.push(seg.clone()),
                    Location::Boundary(OverlapClass::OppositeDirection) => {
                        selected.push(seg.clone())
                    }
                    _ => {}
                }
            }
            for (seg, loc) in segs_b.iter().zip(locs_b.iter()) {
                if *loc == Location::Inside {
                    selected.push(seg.reverse());
                }
            }
        }
        BooleanOp::Intersect => {
            for (seg, loc) in segs_a.iter().zip(locs_a.iter()) {
                match loc {
                    Location::Inside => selected.push(seg.clone()),
                    Location::Boundary(OverlapClass::OppositeDirection) => {
                        selected.push(seg.clone())
                    }
                    _ => {}
                }
            }
            for (seg, loc) in segs_b.iter().zip(locs_b.iter()) {
                if *loc == Location::Inside {
                    selected.push(seg.clone());
                }
            }
        }
    }

    selected
}

/// Build closed loops from selected segments via endpoint matching.
///
/// Gaps between endpoints are bridged with degree=1 linear segments.
pub fn build_loops(
    segments: Vec<NurbsCurve2D>,
    tol: f64,
) -> Result<Vec<Vec<NurbsCurve2D>>, String> {
    if segments.is_empty() {
        return Err("no segments selected".into());
    }

    let mut used = vec![false; segments.len()];
    let mut loops: Vec<Vec<NurbsCurve2D>> = Vec::new();

    while let Some(start_idx) = used.iter().position(|&u| !u) {
        used[start_idx] = true;
        let mut chain: Vec<NurbsCurve2D> = vec![segments[start_idx].clone()];

        // Connect via endpoint matching until closed
        let max_iter = segments.len();
        for _ in 0..max_iter {
            let chain_end = curve_end(chain.last().expect("chain is non-empty"));
            let chain_start = curve_start(&chain[0]);

            // Check if loop is closed
            if chain.len() > 1 && dist2(chain_end, chain_start) < tol {
                break;
            }

            // Find the unused segment whose start/end is closest to chain_end
            let mut best_idx = None;
            let mut best_dist = f64::INFINITY;
            let mut best_reversed = false;

            for (i, seg) in segments.iter().enumerate() {
                if used[i] {
                    continue;
                }
                let start = curve_start(seg);
                let end = curve_end(seg);

                let d_start = dist2(chain_end, start);
                let d_end = dist2(chain_end, end);

                if d_start < best_dist {
                    best_dist = d_start;
                    best_idx = Some(i);
                    best_reversed = false;
                }
                if d_end < best_dist {
                    best_dist = d_end;
                    best_idx = Some(i);
                    best_reversed = true;
                }
            }

            if let Some(idx) = best_idx {
                if best_dist > tol * 100.0 {
                    // Nearest segment too far away -> end loop
                    break;
                }
                used[idx] = true;

                let next_curve = if best_reversed {
                    segments[idx].reverse()
                } else {
                    segments[idx].clone()
                };

                // Insert linear bridging segment if gap exists
                let gap = dist2(chain_end, curve_start(&next_curve));
                if gap > tol {
                    chain.push(make_linear_segment(chain_end, curve_start(&next_curve)));
                }

                chain.push(next_curve);
            } else {
                break;
            }
        }

        if chain.len() < 2 {
            // A single segment is valid if it forms a closed curve
            let start = curve_start(&chain[0]);
            let end = curve_end(&chain[0]);
            if dist2(start, end) >= tol {
                continue;
            }
        }

        // Bridge gap between last and first endpoints
        let chain_end = curve_end(chain.last().expect("chain is non-empty"));
        let chain_start = curve_start(&chain[0]);
        if dist2(chain_end, chain_start) > tol {
            chain.push(make_linear_segment(chain_end, chain_start));
        }

        loops.push(chain);
    }

    if loops.is_empty() {
        return Err("failed to build closed loop".into());
    }

    Ok(loops)
}

/// Evaluate point at curve start parameter.
fn curve_start(c: &NurbsCurve2D) -> [f64; 2] {
    let t = c.knots[c.degree];
    c.evaluate(t)
}

/// Evaluate point at curve end parameter.
fn curve_end(c: &NurbsCurve2D) -> [f64; 2] {
    let n = c.control_points.len();
    let t = c.knots[n];
    c.evaluate(t)
}

/// Create a degree=1 linear segment between two points.
fn make_linear_segment(start: [f64; 2], end: [f64; 2]) -> NurbsCurve2D {
    NurbsCurve2D::new(1, vec![start, end], vec![0.0, 0.0, 1.0, 1.0])
}
