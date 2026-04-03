//! 2D boolean operations module

pub mod classify;
pub mod combine;
pub mod intersect;

use crate::types::BooleanOp;
use neco_nurbs::{dedup_piecewise_sample, NurbsCurve2D, NurbsRegion};

#[derive(Clone, Debug, Default)]
pub struct RegionSet {
    pub regions: Vec<NurbsRegion>,
}

impl RegionSet {
    pub fn new(regions: Vec<NurbsRegion>) -> Self {
        Self { regions }
    }

    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    pub fn len(&self) -> usize {
        self.regions.len()
    }

    pub fn as_slice(&self) -> &[NurbsRegion] {
        &self.regions
    }

    pub fn into_regions(self) -> Vec<NurbsRegion> {
        self.regions
    }
}

/// 2D point-to-point distance
#[inline]
fn dist2(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// Intersection result between two NURBS curves.
#[derive(Clone, Debug)]
pub enum Intersection {
    Point {
        point: [f64; 2],
        t_a: f64,
        t_b: f64,
    },
    /// Collinear overlap interval
    Overlap {
        t_a: (f64, f64),
        t_b: (f64, f64),
    },
}

/// 2D boolean operation on two NurbsRegion.
///
/// Only single-region results are supported; multiple regions return an error.
/// Collinear edges are handled via overlap detection + normal-based classification.
pub fn boolean_2d(a: &NurbsRegion, b: &NurbsRegion, op: BooleanOp) -> Result<NurbsRegion, String> {
    let result = boolean_2d_all(a, b, op)?;
    match result.len() {
        0 => Err("boolean result is empty; use boolean_2d_all".into()),
        1 => match result.into_regions().into_iter().next() {
            Some(region) => Ok(region),
            None => Err("boolean result lost its single region unexpectedly".into()),
        },
        n => Err(format!(
            "boolean result has {} regions; use boolean_2d_all",
            n
        )),
    }
}

/// 2D boolean operation returning zero or more regions.
pub fn boolean_2d_all(
    a: &NurbsRegion,
    b: &NurbsRegion,
    op: BooleanOp,
) -> Result<RegionSet, String> {
    boolean_2d_inner(a, b, op)
}

/// Kind of a split segment.
#[derive(Clone, Debug)]
pub enum SegmentKind {
    Normal,
    /// Overlap segment classified by normal direction
    Overlap {
        other_curve_t_mid: f64,
        other_curve_index: usize,
    },
}

/// Piecewise overlap info: (segment_index, local_t_start, local_t_end) pairs.
struct PiecewiseOverlap {
    my: (usize, f64, f64),
    other: (usize, f64, f64),
}

/// Classify each split segment as Normal or Overlap.
fn classify_segment_kinds(
    segs: &[NurbsCurve2D],
    overlaps: &[PiecewiseOverlap],
    seg_offsets: &[usize],
    is_curve_a: bool,
) -> Vec<SegmentKind> {
    segs.iter()
        .enumerate()
        .map(|(i, seg)| {
            let n = seg.control_points.len();
            let t_min = seg.knots[seg.degree];
            let t_max = seg.knots[n];
            let t_mid = 0.5 * (t_min + t_max);

            for ov in overlaps {
                let (my, other) = if is_curve_a {
                    (&ov.my, &ov.other)
                } else {
                    (&ov.other, &ov.my)
                };
                let (my_seg_idx, my_t0, my_t1) = *my;
                let (other_seg_idx, other_t0, other_t1) = *other;

                // Check if this segment belongs to the split range of the source segment
                let split_start = seg_offsets[my_seg_idx];
                let split_end = if my_seg_idx + 1 < seg_offsets.len() {
                    seg_offsets[my_seg_idx + 1]
                } else {
                    segs.len()
                };
                if i < split_start || i >= split_end {
                    continue;
                }

                // Normalize interval for reverse traversal
                let (my_lo, my_hi) = if my_t0 <= my_t1 {
                    (my_t0, my_t1)
                } else {
                    (my_t1, my_t0)
                };
                // Check if segment midpoint falls within the overlap interval
                if t_mid >= my_lo - 1e-8 && t_mid <= my_hi + 1e-8 {
                    let other_mid = 0.5 * (other_t0 + other_t1);
                    return SegmentKind::Overlap {
                        other_curve_t_mid: other_mid,
                        other_curve_index: other_seg_idx,
                    };
                }
            }
            SegmentKind::Normal
        })
        .collect()
}

fn boolean_2d_inner(a: &NurbsRegion, b: &NurbsRegion, op: BooleanOp) -> Result<RegionSet, String> {
    let outer_a = &a.outer;
    let outer_b = &b.outer;

    // Find intersections for all segment pairs, collecting local t per segment
    let mut params_a: Vec<Vec<f64>> = vec![Vec::new(); outer_a.len()];
    let mut params_b: Vec<Vec<f64>> = vec![Vec::new(); outer_b.len()];
    let mut pw_overlaps: Vec<PiecewiseOverlap> = Vec::new();
    let mut has_any_intersection = false;

    for (ia, seg_a) in outer_a.iter().enumerate() {
        for (ib, seg_b) in outer_b.iter().enumerate() {
            let ixns = intersect::find_intersections(seg_a, seg_b);
            for ix in &ixns {
                has_any_intersection = true;
                match ix {
                    Intersection::Point { t_a, t_b, .. } => {
                        params_a[ia].push(*t_a);
                        params_b[ib].push(*t_b);
                    }
                    Intersection::Overlap { t_a, t_b } => {
                        params_a[ia].push(t_a.0);
                        params_a[ia].push(t_a.1);
                        params_b[ib].push(t_b.0);
                        params_b[ib].push(t_b.1);
                        pw_overlaps.push(PiecewiseOverlap {
                            my: (ia, t_a.0, t_a.1),
                            other: (ib, t_b.0, t_b.1),
                        });
                    }
                }
            }
        }
    }

    if !has_any_intersection {
        return handle_no_intersection(a, b, op);
    }

    let eps = 1e-10;

    // Sort, dedup, and exclude endpoints from local t values, then split
    fn prepare_and_split(
        outer: &[NurbsCurve2D],
        params: &mut [Vec<f64>],
        eps: f64,
    ) -> (Vec<NurbsCurve2D>, Vec<usize>) {
        let mut all_segs: Vec<NurbsCurve2D> = Vec::new();
        let mut seg_offsets: Vec<usize> = Vec::new();

        for (i, curve) in outer.iter().enumerate() {
            seg_offsets.push(all_segs.len());

            let n = curve.control_points.len();
            let t_lo = curve.knots[curve.degree];
            let t_hi = curve.knots[n];

            let ts = &mut params[i];
            ts.sort_by(|x, y| x.total_cmp(y));
            ts.dedup_by(|a, b| (*a - *b).abs() < eps);
            ts.retain(|&t| t > t_lo + eps && t < t_hi - eps);

            let split = curve.split_at_params(ts);
            all_segs.extend(split);
        }
        (all_segs, seg_offsets)
    }

    let (segs_a, seg_offsets_a) = prepare_and_split(outer_a, &mut params_a, eps);
    let (segs_b, seg_offsets_b) = prepare_and_split(outer_b, &mut params_b, eps);

    // If all split params were excluded as endpoints and no overlaps, treat as no intersection
    let all_trivial_a = segs_a.len() == outer_a.len();
    let all_trivial_b = segs_b.len() == outer_b.len();
    if all_trivial_a && all_trivial_b && pw_overlaps.is_empty() {
        return handle_no_intersection(a, b, op);
    }

    let kinds_a = classify_segment_kinds(&segs_a, &pw_overlaps, &seg_offsets_a, true);
    let kinds_b = classify_segment_kinds(&segs_b, &pw_overlaps, &seg_offsets_b, false);

    let locs_a = classify::classify_segments_with_kinds(&segs_a, &kinds_a, outer_b, outer_a);
    let locs_b = classify::classify_segments_with_kinds(&segs_b, &kinds_b, outer_a, outer_b);

    let selected = combine::select_segments(op, &segs_a, &locs_a, &segs_b, &locs_b);
    let loops = if selected.is_empty() {
        Vec::new()
    } else {
        combine::build_loops(selected, 1e-6)?
    };
    Ok(region_set_from_loops(loops))
}

/// Sample start point from the first segment's start parameter.
fn sample_start_point(outer: &[NurbsCurve2D]) -> [f64; 2] {
    let seg = &outer[0];
    let t_start = seg.knots[seg.degree];
    seg.evaluate(t_start)
}

/// Handle case with no intersections (containment or disjoint).
fn handle_no_intersection(
    a: &NurbsRegion,
    b: &NurbsRegion,
    op: BooleanOp,
) -> Result<RegionSet, String> {
    let a_sample = sample_start_point(&a.outer);
    let b_sample = sample_start_point(&b.outer);
    let a_in_b = classify::point_in_nurbs_region(&a_sample, &b.outer);
    let b_in_a = classify::point_in_nurbs_region(&b_sample, &a.outer);

    match op {
        BooleanOp::Union => {
            if a_in_b {
                Ok(RegionSet::new(vec![b.clone()]))
            } else if b_in_a {
                Ok(RegionSet::new(vec![a.clone()]))
            } else {
                Ok(RegionSet::new(vec![a.clone(), b.clone()]))
            }
        }
        BooleanOp::Subtract => {
            if b_in_a {
                // B is inside A: return B as a hole
                Ok(RegionSet::new(vec![NurbsRegion {
                    outer: a.outer.clone(),
                    holes: vec![b.outer.clone()],
                }]))
            } else if a_in_b {
                Ok(RegionSet::default())
            } else {
                // Disjoint: A unchanged
                Ok(RegionSet::new(vec![a.clone()]))
            }
        }
        BooleanOp::Intersect => {
            if a_in_b {
                Ok(RegionSet::new(vec![a.clone()]))
            } else if b_in_a {
                Ok(RegionSet::new(vec![b.clone()]))
            } else {
                Ok(RegionSet::default())
            }
        }
    }
}

fn region_set_from_loops(loops: Vec<Vec<NurbsCurve2D>>) -> RegionSet {
    if loops.is_empty() {
        return RegionSet::default();
    }

    #[derive(Clone)]
    struct LoopInfo {
        curves: Vec<NurbsCurve2D>,
        sample: [f64; 2],
        area: f64,
        parent: Option<usize>,
        depth: usize,
    }

    let mut infos: Vec<LoopInfo> = loops
        .into_iter()
        .map(|curves| {
            let sample = sample_start_point(&curves);
            let area = sampled_loop_area(&curves).abs();
            LoopInfo {
                curves,
                sample,
                area,
                parent: None,
                depth: 0,
            }
        })
        .collect();

    for i in 0..infos.len() {
        let mut parent = None;
        let mut parent_area = f64::INFINITY;
        for j in 0..infos.len() {
            if i == j {
                continue;
            }
            if classify::point_in_nurbs_region(&infos[i].sample, &infos[j].curves)
                && infos[j].area < parent_area
            {
                parent = Some(j);
                parent_area = infos[j].area;
            }
        }
        infos[i].parent = parent;
    }

    for i in 0..infos.len() {
        let mut depth = 0usize;
        let mut cursor = infos[i].parent;
        while let Some(parent) = cursor {
            depth += 1;
            cursor = infos[parent].parent;
        }
        infos[i].depth = depth;
    }

    let mut regions: Vec<NurbsRegion> = infos
        .iter()
        .filter(|info| info.depth % 2 == 0)
        .map(|info| NurbsRegion {
            outer: info.curves.clone(),
            holes: vec![],
        })
        .collect();

    for info in infos.iter().filter(|info| info.depth % 2 == 1) {
        let mut cursor = info.parent;
        while let Some(parent) = cursor {
            if infos[parent].depth % 2 == 0 {
                if let Some(region) = regions
                    .iter_mut()
                    .find(|region| same_loop(region.outer_curves(), &infos[parent].curves))
                {
                    region.holes.push(info.curves.clone());
                }
                break;
            }
            cursor = infos[parent].parent;
        }
    }

    RegionSet::new(regions)
}

fn same_loop(a: &[NurbsCurve2D], b: &[NurbsCurve2D]) -> bool {
    let pa = sample_start_point(a);
    let pb = sample_start_point(b);
    dist2(pa, pb) < 1e-8 && (sampled_loop_area(a) - sampled_loop_area(b)).abs() < 1e-6
}

fn sampled_loop_area(curves: &[NurbsCurve2D]) -> f64 {
    let polygon = dedup_piecewise_sample(curves.iter(), 0.01);
    signed_polygon_area(&polygon)
}

fn signed_polygon_area(points: &[[f64; 2]]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    let mut area = 0.0;
    for i in 0..points.len() {
        let j = (i + 1) % points.len();
        area += points[i][0] * points[j][1] - points[j][0] * points[i][1];
    }
    area * 0.5
}
