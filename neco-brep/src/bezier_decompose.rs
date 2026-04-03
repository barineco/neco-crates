//! Decompose a NURBS tensor-product surface into Bezier patches.

use neco_nurbs::NurbsSurface3D;

/// Rational tensor-product Bezier patch.
#[derive(Clone, Debug)]
pub struct BezierPatch {
    pub degree_u: usize,
    pub degree_v: usize,
    /// Control point grid \[degree_u+1\]\[degree_v+1\].
    pub control_points: Vec<Vec<[f64; 3]>>,
    /// Weight grid \[degree_u+1\]\[degree_v+1\].
    pub weights: Vec<Vec<f64>>,
    pub u_min: f64,
    pub u_max: f64,
    pub v_min: f64,
    pub v_max: f64,
}

impl BezierPatch {
    /// Evaluate a point on the patch by converting to NurbsSurface3D internally.
    pub fn evaluate(&self, u: f64, v: f64) -> [f64; 3] {
        let p = self.degree_u;
        let q = self.degree_v;
        let mut knots_u = vec![self.u_min; p + 1];
        knots_u.extend(vec![self.u_max; p + 1]);
        let mut knots_v = vec![self.v_min; q + 1];
        knots_v.extend(vec![self.v_max; q + 1]);

        let surf = NurbsSurface3D {
            degree_u: p,
            degree_v: q,
            control_points: self.control_points.clone(),
            weights: self.weights.clone(),
            knots_u,
            knots_v,
        };
        surf.evaluate(u, v)
    }

    /// Compute the axis-aligned bounding box from control points.
    pub fn aabb(&self) -> ([f64; 3], [f64; 3]) {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut min_z = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut max_z = f64::NEG_INFINITY;

        for row in &self.control_points {
            for pt in row {
                min_x = min_x.min(pt[0]);
                min_y = min_y.min(pt[1]);
                min_z = min_z.min(pt[2]);
                max_x = max_x.max(pt[0]);
                max_y = max_y.max(pt[1]);
                max_z = max_z.max(pt[2]);
            }
        }

        ([min_x, min_y, min_z], [max_x, max_y, max_z])
    }

    pub fn u_range(&self) -> (f64, f64) {
        (self.u_min, self.u_max)
    }

    pub fn v_range(&self) -> (f64, f64) {
        (self.v_min, self.v_max)
    }
}

/// Collect internal knots and their multiplicities from a knot vector.
fn collect_internal_knots(knots: &[f64], degree: usize, n_cp: usize) -> Vec<(f64, usize)> {
    let t_min = knots[degree];
    let t_max = knots[n_cp];

    let mut result: Vec<(f64, usize)> = Vec::new();
    let mut i = degree + 1;
    while i < knots.len() - degree - 1 {
        let t = knots[i];
        if t > t_min && t < t_max {
            let mut mult = 0;
            let mut j = i;
            while j < knots.len() && (knots[j] - t).abs() < 1e-14 {
                mult += 1;
                j += 1;
            }
            result.push((t, mult));
            i = j;
        } else {
            i += 1;
        }
    }
    result
}

/// Decompose a `NurbsSurface3D` into tensor-product Bezier patches.
///
/// Raises internal knot multiplicities to full degree in both u and v,
/// then extracts (degree+1) x (degree+1) control point blocks.
pub fn decompose_to_bezier_patches(surface: &NurbsSurface3D) -> Vec<BezierPatch> {
    let p = surface.degree_u;
    let q = surface.degree_v;
    let mut surf = surface.clone();

    // Raise u-direction internal knot multiplicities to degree_u
    let internal_u = collect_internal_knots(&surf.knots_u, p, surf.control_points.len());
    for (t, mult) in &internal_u {
        let insertions_needed = p - mult;
        for _ in 0..insertions_needed {
            surf = surf.insert_knot_u(*t);
        }
    }

    // Raise v-direction internal knot multiplicities to degree_v
    let internal_v = collect_internal_knots(&surf.knots_v, q, surf.control_points[0].len());
    for (t, mult) in &internal_v {
        let insertions_needed = q - mult;
        for _ in 0..insertions_needed {
            surf = surf.insert_knot_v(*t);
        }
    }

    // Split into Bezier patches
    let n_u = surf.control_points.len();
    let n_v = surf.control_points[0].len();
    let num_spans_u = (n_u - 1) / p;
    let num_spans_v = (n_v - 1) / q;

    // Zero spans: return the original surface as a single patch
    if num_spans_u == 0 || num_spans_v == 0 {
        return vec![BezierPatch {
            degree_u: p,
            degree_v: q,
            control_points: surf.control_points.clone(),
            weights: surf.weights.clone(),
            u_min: surface.knots_u[p],
            u_max: surface.knots_u[surface.control_points.len()],
            v_min: surface.knots_v[q],
            v_max: surface.knots_v[surface.control_points[0].len()],
        }];
    }

    let mut patches = Vec::with_capacity(num_spans_u * num_spans_v);

    let cp = &surf.control_points;
    let ws = &surf.weights;

    for su in 0..num_spans_u {
        let u_start_cp = su * p;
        let u_end_cp = u_start_cp + p + 1;
        if u_end_cp > n_u {
            break;
        }
        let u_min = surf.knots_u[u_start_cp + p];
        let u_max = surf.knots_u[u_end_cp];

        for sv in 0..num_spans_v {
            let v_start_cp = sv * q;
            let v_end_cp = v_start_cp + q + 1;
            if v_end_cp > n_v {
                break;
            }
            let v_min = surf.knots_v[v_start_cp + q];
            let v_max = surf.knots_v[v_end_cp];

            // Extract control point block
            let mut cp_block = Vec::with_capacity(p + 1);
            let mut w_block = Vec::with_capacity(p + 1);
            for i in u_start_cp..u_end_cp {
                cp_block.push(cp[i][v_start_cp..v_end_cp].to_vec());
                w_block.push(ws[i][v_start_cp..v_end_cp].to_vec());
            }

            patches.push(BezierPatch {
                degree_u: p,
                degree_v: q,
                control_points: cp_block,
                weights: w_block,
                u_min,
                u_max,
                v_min,
                v_max,
            });
        }
    }

    patches
}
