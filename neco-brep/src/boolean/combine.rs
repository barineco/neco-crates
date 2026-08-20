//! Boolean 演算で選択した線分から閉ループを構成する処理。

use super::classify::{Location, OverlapClass};
use super::dist2;
use crate::types::BooleanOp;
use neco_nurbs::NurbsCurve2D;

/// 演算種別と位置分類に従って結果境界の線分を選択します。減算では B の内側線分を逆向きにします。
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

/// 未使用の線分から近い端点を順に連結し、隙間には一次線分を追加して閉ループを構成します。次の線分までの距離が `tol * 100` を超える場合は、その線分を残したまま現在の鎖を閉じるため、孤立した線分を含まない別のループを返すことがあります。線分が空、またはループを一つも作れない場合は失敗します。
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

        let max_iter = segments.len();
        for _ in 0..max_iter {
            let chain_end = curve_end(chain.last().expect("chain is non-empty"));
            let chain_start = curve_start(&chain[0]);

            if chain.len() > 1 && dist2(chain_end, chain_start) < tol {
                break;
            }

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
                    break;
                }
                used[idx] = true;

                let next_curve = if best_reversed {
                    segments[idx].reverse()
                } else {
                    segments[idx].clone()
                };

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
            let start = curve_start(&chain[0]);
            let end = curve_end(&chain[0]);
            if dist2(start, end) >= tol {
                continue;
            }
        }

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

fn curve_start(c: &NurbsCurve2D) -> [f64; 2] {
    let t = c.knots[c.degree];
    c.evaluate(t)
}

fn curve_end(c: &NurbsCurve2D) -> [f64; 2] {
    let n = c.control_points.len();
    let t = c.knots[n];
    c.evaluate(t)
}

fn make_linear_segment(start: [f64; 2], end: [f64; 2]) -> NurbsCurve2D {
    NurbsCurve2D::new(1, vec![start, end], vec![0.0, 0.0, 1.0, 1.0])
}
