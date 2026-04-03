//! B-Rep core data structures: Shell, Surface, Edge, Face, Curve3D.

use crate::bezier::{de_casteljau_rational_2d, de_casteljau_rational_3d, interpolate_frame};
use crate::vec3;

pub type VertexId = usize;
pub type EdgeId = usize;
pub type FaceId = usize;

const VERTEX_TOL: f64 = 1e-10;

/// Surface type.
#[derive(Debug, Clone)]
pub enum Surface {
    Plane {
        origin: [f64; 3],
        normal: [f64; 3],
    },
    Cylinder {
        origin: [f64; 3],
        axis: [f64; 3],
        radius: f64,
    },
    Cone {
        origin: [f64; 3],
        axis: [f64; 3],
        half_angle: f64,
    },
    Sphere {
        center: [f64; 3],
        radius: f64,
    },
    Ellipsoid {
        center: [f64; 3],
        rx: f64,
        ry: f64,
        rz: f64,
    },
    Torus {
        center: [f64; 3],
        axis: [f64; 3],
        major_radius: f64,
        minor_radius: f64,
    },
    SurfaceOfRevolution {
        center: [f64; 3],
        axis: [f64; 3],
        /// Unit vector for theta=0 direction.
        frame_u: [f64; 3],
        /// Unit vector for theta=pi/2 direction.
        frame_v: [f64; 3],
        /// Profile curve: 2D rational Bezier in (r, z) on the meridional plane.
        profile_control_points: Vec<[f64; 2]>,
        profile_weights: Vec<f64>,
        profile_degree: u32,
        /// Number of Bezier spans in the profile.
        n_profile_spans: u32,
        /// Start angle (rad).
        theta_start: f64,
        /// Angular sweep (rad).
        theta_range: f64,
    },
    SurfaceOfSweep {
        /// Bezier-decomposed spine control points.
        spine_control_points: Vec<[f64; 3]>,
        spine_weights: Vec<f64>,
        spine_degree: u32,
        /// Cross-section profile (x, y) Bezier spans.
        profile_control_points: Vec<[f64; 2]>,
        profile_weights: Vec<f64>,
        profile_degree: u32,
        n_profile_spans: u32,
        /// RMF frames at span endpoints: (normal, binormal, tangent).
        frames: Vec<[[f64; 3]; 3]>,
    },
    NurbsSurface {
        data: Box<neco_nurbs::NurbsSurface3D>,
    },
}

impl Surface {
    /// Evaluate surface point at parameter (u, v).
    pub fn evaluate(&self, u: f64, v: f64) -> [f64; 3] {
        match self {
            Surface::Plane { origin, normal } => {
                let n = vec3::normalized(*normal);
                let up = if n[0].abs() < 0.9 {
                    [1.0, 0.0, 0.0]
                } else {
                    [0.0, 1.0, 0.0]
                };
                let u_vec = vec3::normalized(vec3::cross(n, up));
                let v_vec = vec3::cross(n, u_vec);
                vec3::add(
                    vec3::add(*origin, vec3::scale(u_vec, u)),
                    vec3::scale(v_vec, v),
                )
            }
            Surface::Cylinder {
                origin,
                axis,
                radius,
            } => {
                let axis_len = vec3::length(*axis);
                let axis_n = vec3::normalized(*axis);
                let (bu, bv) = vec3::orthonormal_basis(*axis);
                let h = v.clamp(0.0, axis_len);
                vec3::add(
                    vec3::add(*origin, vec3::scale(axis_n, h)),
                    vec3::add(
                        vec3::scale(bu, radius * u.cos()),
                        vec3::scale(bv, radius * u.sin()),
                    ),
                )
            }
            Surface::Cone {
                origin,
                axis,
                half_angle,
            } => {
                let axis_len = vec3::length(*axis);
                let axis_n = vec3::normalized(*axis);
                let (bu, bv) = vec3::orthonormal_basis(*axis);
                let t = v.clamp(0.0, axis_len);
                let r = t * half_angle.tan();
                vec3::add(
                    vec3::add(*origin, vec3::scale(axis_n, t)),
                    vec3::add(vec3::scale(bu, r * u.cos()), vec3::scale(bv, r * u.sin())),
                )
            }
            Surface::Sphere { center, radius } => {
                let sin_v = v.sin();
                let cos_v = v.cos();
                [
                    center[0] + radius * sin_v * u.cos(),
                    center[1] + radius * sin_v * u.sin(),
                    center[2] + radius * cos_v,
                ]
            }
            Surface::Ellipsoid { center, rx, ry, rz } => {
                let sin_v = v.sin();
                let cos_v = v.cos();
                [
                    center[0] + rx * sin_v * u.cos(),
                    center[1] + ry * sin_v * u.sin(),
                    center[2] + rz * cos_v,
                ]
            }
            Surface::Torus {
                center,
                axis,
                major_radius,
                minor_radius,
            } => {
                let axis_n = vec3::normalized(*axis);
                let (bu, bv) = vec3::orthonormal_basis(*axis);
                let r = major_radius + minor_radius * v.cos();
                let dir = vec3::add(vec3::scale(bu, u.cos()), vec3::scale(bv, u.sin()));
                vec3::add(
                    vec3::add(*center, vec3::scale(dir, r)),
                    vec3::scale(axis_n, minor_radius * v.sin()),
                )
            }
            Surface::SurfaceOfRevolution {
                center,
                axis,
                frame_u,
                frame_v,
                profile_control_points,
                profile_weights,
                profile_degree,
                n_profile_spans,
                ..
            } => {
                let (r, z) = eval_revolution_profile(
                    profile_control_points,
                    profile_weights,
                    *profile_degree,
                    *n_profile_spans,
                    v,
                );
                let axis_n = vec3::normalized(*axis);
                let bu = vec3::normalized(*frame_u);
                let bv = vec3::normalized(*frame_v);
                let theta = u;
                vec3::add(
                    vec3::add(
                        *center,
                        vec3::add(
                            vec3::scale(bu, r * theta.cos()),
                            vec3::scale(bv, r * theta.sin()),
                        ),
                    ),
                    vec3::scale(axis_n, z),
                )
            }
            Surface::SurfaceOfSweep {
                spine_control_points,
                spine_weights,
                spine_degree,
                profile_control_points,
                profile_weights,
                profile_degree,
                n_profile_spans,
                frames,
            } => {
                let deg = *spine_degree as usize;
                let n_spine_spans = if deg > 0 {
                    (spine_control_points.len() - 1) / deg
                } else {
                    1
                };

                let (spine_pt, normal, binormal) = if n_spine_spans <= 1 {
                    let pt = de_casteljau_rational_3d(spine_control_points, spine_weights, u);
                    let (n, b) = interpolate_frame(frames, u);
                    (pt, n, b)
                } else {
                    let u_clamped = u.clamp(0.0, 1.0);
                    let span_f = u_clamped * n_spine_spans as f64;
                    let span_idx = (span_f as usize).min(n_spine_spans - 1);
                    let local_u = span_f - span_idx as f64;
                    let start_cp = span_idx * deg;
                    let end_cp = start_cp + deg + 1;
                    let pt = de_casteljau_rational_3d(
                        &spine_control_points[start_cp..end_cp],
                        &spine_weights[start_cp..end_cp],
                        local_u,
                    );
                    let (n, b) = interpolate_frame(&frames[span_idx..span_idx + 2], local_u);
                    (pt, n, b)
                };

                let n_prof = *n_profile_spans as usize;
                let p_deg = *profile_degree as usize;
                let cps_per_span = p_deg + 1;
                let (px, py) = if n_prof <= 1 {
                    de_casteljau_rational_2d(profile_control_points, profile_weights, v)
                } else {
                    let v_clamped = v.clamp(0.0, 1.0);
                    let span_f = v_clamped * n_prof as f64;
                    let span_idx = (span_f as usize).min(n_prof - 1);
                    let local_v = span_f - span_idx as f64;
                    let start = span_idx * cps_per_span;
                    let end = start + cps_per_span;
                    de_casteljau_rational_2d(
                        &profile_control_points[start..end],
                        &profile_weights[start..end],
                        local_v,
                    )
                };

                [
                    spine_pt[0] + px * normal[0] + py * binormal[0],
                    spine_pt[1] + px * normal[1] + py * binormal[1],
                    spine_pt[2] + px * normal[2] + py * binormal[2],
                ]
            }
            Surface::NurbsSurface { data } => data.evaluate(u, v),
        }
    }

    /// Outward unit normal at parameter (u, v).
    pub fn normal_at(&self, u: f64, v: f64) -> [f64; 3] {
        match self {
            Surface::Plane { normal, .. } => vec3::normalized(*normal),
            Surface::Cylinder { axis, .. } => {
                let (bu, bv) = vec3::orthonormal_basis(*axis);
                vec3::normalized(vec3::add(
                    vec3::scale(bu, u.cos()),
                    vec3::scale(bv, u.sin()),
                ))
            }
            Surface::Cone {
                axis, half_angle, ..
            } => {
                let (bu, bv) = vec3::orthonormal_basis(*axis);
                let cos_ha = half_angle.cos();
                let sin_ha = half_angle.sin();
                let a = vec3::normalized(*axis);
                let radial = vec3::add(vec3::scale(bu, u.cos()), vec3::scale(bv, u.sin()));
                vec3::normalized(vec3::add(
                    vec3::scale(radial, cos_ha),
                    vec3::scale(a, -sin_ha),
                ))
            }
            Surface::Sphere { center, radius } => {
                let p = self.evaluate(u, v);
                [
                    (p[0] - center[0]) / radius,
                    (p[1] - center[1]) / radius,
                    (p[2] - center[2]) / radius,
                ]
            }
            Surface::Ellipsoid { center, rx, ry, rz } => {
                let p = self.evaluate(u, v);
                let nx = (p[0] - center[0]) / (rx * rx);
                let ny = (p[1] - center[1]) / (ry * ry);
                let nz = (p[2] - center[2]) / (rz * rz);
                let len = (nx * nx + ny * ny + nz * nz).sqrt();
                if len < 1e-30 {
                    [0.0, 0.0, 1.0]
                } else {
                    [nx / len, ny / len, nz / len]
                }
            }
            Surface::Torus {
                center,
                axis,
                major_radius,
                ..
            } => {
                let (bu, bv) = vec3::orthonormal_basis(*axis);
                let p = self.evaluate(u, v);
                let dir = vec3::add(vec3::scale(bu, u.cos()), vec3::scale(bv, u.sin()));
                let ring_center = vec3::add(*center, vec3::scale(dir, *major_radius));
                vec3::normalized(vec3::sub(p, ring_center))
            }
            Surface::SurfaceOfRevolution {
                axis,
                frame_u,
                frame_v,
                profile_control_points,
                profile_weights,
                profile_degree,
                n_profile_spans,
                ..
            } => {
                let (r, _z) = eval_revolution_profile(
                    profile_control_points,
                    profile_weights,
                    *profile_degree,
                    *n_profile_spans,
                    v,
                );
                let axis_n = vec3::normalized(*axis);
                let bu = vec3::normalized(*frame_u);
                let bv = vec3::normalized(*frame_v);
                let theta = u;
                let cos_t = theta.cos();
                let sin_t = theta.sin();

                let ds_du = vec3::add(vec3::scale(bu, -r * sin_t), vec3::scale(bv, r * cos_t));

                let eps = 1e-6;
                let v_lo = (v - eps).max(0.0);
                let v_hi = (v + eps).min(1.0);
                let (r_lo, z_lo) = eval_revolution_profile(
                    profile_control_points,
                    profile_weights,
                    *profile_degree,
                    *n_profile_spans,
                    v_lo,
                );
                let (r_hi, z_hi) = eval_revolution_profile(
                    profile_control_points,
                    profile_weights,
                    *profile_degree,
                    *n_profile_spans,
                    v_hi,
                );
                let dt = v_hi - v_lo;
                let dr = (r_hi - r_lo) / dt;
                let dz = (z_hi - z_lo) / dt;
                let ds_dv = vec3::add(
                    vec3::add(vec3::scale(bu, dr * cos_t), vec3::scale(bv, dr * sin_t)),
                    vec3::scale(axis_n, dz),
                );

                let normal = vec3::cross(ds_du, ds_dv);
                let len = vec3::length(normal);
                if len < 1e-30 {
                    axis_n
                } else {
                    vec3::scale(normal, 1.0 / len)
                }
            }
            Surface::SurfaceOfSweep { .. } => {
                let eps = 1e-6;
                let u_lo = (u - eps).max(0.0);
                let u_hi = (u + eps).min(1.0);
                let v_lo = (v - eps).max(0.0);
                let v_hi = (v + eps).min(1.0);
                let du = vec3::sub(self.evaluate(u_hi, v), self.evaluate(u_lo, v));
                let dv = vec3::sub(self.evaluate(u, v_hi), self.evaluate(u, v_lo));
                let n = vec3::cross(du, dv);
                let len = vec3::length(n);
                if len < 1e-12 {
                    [0.0, 0.0, 1.0]
                } else {
                    vec3::scale(n, 1.0 / len)
                }
            }
            Surface::NurbsSurface { data } => data.normal(u, v),
        }
    }

    /// Convert SurfaceOfSweep/Revolution to NurbsSurface3D.
    pub fn to_nurbs_surface(&self) -> Option<neco_nurbs::NurbsSurface3D> {
        match self {
            Surface::SurfaceOfSweep {
                spine_control_points,
                spine_weights,
                spine_degree,
                profile_control_points,
                profile_weights,
                profile_degree,
                n_profile_spans,
                frames,
            } => {
                let s_deg = *spine_degree as usize;
                let n_spine_spans = if s_deg > 0 {
                    (spine_control_points.len() - 1) / s_deg
                } else {
                    1
                };
                let n_prof = *n_profile_spans as usize;

                let (v_degree, prof_cps, prof_ws) = if n_prof <= 1 {
                    let deg = profile_control_points.len() - 1;
                    (deg, profile_control_points.clone(), profile_weights.clone())
                } else {
                    let p_deg = *profile_degree as usize;
                    let cps_per_span = p_deg + 1;
                    let mut cps = Vec::new();
                    let mut ws = Vec::new();
                    for span_j in 0..n_prof {
                        let base = span_j * cps_per_span;
                        let start_k = if span_j == 0 { 0 } else { 1 };
                        for k in start_k..cps_per_span {
                            cps.push(profile_control_points[base + k]);
                            ws.push(profile_weights[base + k]);
                        }
                    }
                    (p_deg, cps, ws)
                };

                let n_spine_pts = n_spine_spans + 1;
                let mut spine_pts = Vec::with_capacity(n_spine_pts);
                let mut spine_wts = Vec::with_capacity(n_spine_pts);
                let mut normals = Vec::with_capacity(n_spine_pts);
                let mut binormals = Vec::with_capacity(n_spine_pts);

                for si in 0..n_spine_pts {
                    let pt = if n_spine_spans <= 1 {
                        let t = si as f64 / n_spine_spans.max(1) as f64;
                        de_casteljau_rational_3d(spine_control_points, spine_weights, t)
                    } else if si < n_spine_spans {
                        spine_control_points[si * s_deg]
                    } else {
                        spine_control_points[spine_control_points.len() - 1]
                    };
                    let w = if n_spine_spans <= 1 {
                        if si == 0 {
                            spine_weights[0]
                        } else {
                            spine_weights[spine_weights.len() - 1]
                        }
                    } else if si < n_spine_spans {
                        spine_weights[si * s_deg]
                    } else {
                        spine_weights[spine_weights.len() - 1]
                    };

                    let n = frames[si][0];
                    let b = frames[si][1];

                    spine_pts.push(pt);
                    spine_wts.push(w);
                    normals.push(n);
                    binormals.push(b);
                }

                let n_v_cps = prof_cps.len();
                let mut rows_cp: Vec<Vec<[f64; 3]>> = Vec::with_capacity(n_spine_pts);
                let mut rows_w: Vec<Vec<f64>> = Vec::with_capacity(n_spine_pts);

                for i in 0..n_spine_pts {
                    let sp = spine_pts[i];
                    let sw = spine_wts[i];
                    let n = &normals[i];
                    let b = &binormals[i];

                    let mut row_cp = Vec::with_capacity(n_v_cps);
                    let mut row_w = Vec::with_capacity(n_v_cps);

                    for j in 0..n_v_cps {
                        let [px, py] = prof_cps[j];
                        let pw = prof_ws[j];
                        let cp = [
                            sp[0] + px * n[0] + py * b[0],
                            sp[1] + px * n[1] + py * b[1],
                            sp[2] + px * n[2] + py * b[2],
                        ];
                        row_cp.push(cp);
                        row_w.push(sw * pw);
                    }

                    rows_cp.push(row_cp);
                    rows_w.push(row_w);
                }

                let mut knots_u = vec![0.0; 2];
                for i in 1..n_spine_spans {
                    let t = i as f64 / n_spine_spans as f64;
                    knots_u.push(t);
                }
                knots_u.extend(vec![1.0; 2]);

                let knots_v = if n_prof <= 1 {
                    let mut kv = vec![0.0; v_degree + 1];
                    kv.extend(vec![1.0; v_degree + 1]);
                    kv
                } else {
                    let p_deg = *profile_degree as usize;
                    let mut kv = vec![0.0; p_deg + 1];
                    for i in 1..n_prof {
                        let t = i as f64 / n_prof as f64;
                        for _ in 0..p_deg {
                            kv.push(t);
                        }
                    }
                    kv.extend(vec![1.0; p_deg + 1]);
                    kv
                };

                Some(neco_nurbs::NurbsSurface3D {
                    degree_u: 1,
                    degree_v: v_degree,
                    control_points: rows_cp,
                    weights: rows_w,
                    knots_u,
                    knots_v,
                })
            }
            Surface::SurfaceOfRevolution {
                center,
                axis,
                frame_u,
                frame_v,
                profile_control_points,
                profile_weights,
                profile_degree,
                n_profile_spans,
                theta_start,
                theta_range,
            } => {
                use std::f64::consts::FRAC_PI_2;

                let axis_n = vec3::normalized(*axis);
                let bu = vec3::normalized(*frame_u);
                let bv = vec3::normalized(*frame_v);

                let n_u_spans = (*theta_range / FRAC_PI_2).ceil().max(1.0) as usize;
                let span_angle = *theta_range / n_u_spans as f64;
                let half_angle = span_angle / 2.0;
                let w_mid = half_angle.cos();

                let n_u_cps = 2 * n_u_spans + 1;
                let mut u_cos = Vec::with_capacity(n_u_cps);
                let mut u_sin = Vec::with_capacity(n_u_cps);
                let mut u_weights = Vec::with_capacity(n_u_cps);

                for span_i in 0..n_u_spans {
                    let theta0 = *theta_start + span_i as f64 * span_angle;
                    let theta_mid = theta0 + half_angle;
                    let theta1 = theta0 + span_angle;

                    if span_i == 0 {
                        u_cos.push(theta0.cos());
                        u_sin.push(theta0.sin());
                        u_weights.push(1.0);
                    }

                    u_cos.push(theta_mid.cos() / w_mid);
                    u_sin.push(theta_mid.sin() / w_mid);
                    u_weights.push(w_mid);

                    u_cos.push(theta1.cos());
                    u_sin.push(theta1.sin());
                    u_weights.push(1.0);
                }

                let mut knots_u = vec![*theta_start; 3];
                for i in 1..n_u_spans {
                    let t = *theta_start + i as f64 * span_angle;
                    knots_u.push(t);
                    knots_u.push(t);
                }
                let theta_end = *theta_start + *theta_range;
                knots_u.extend(vec![theta_end; 3]);

                let n_prof = *n_profile_spans as usize;
                let (v_degree, prof_cps, prof_ws) = if n_prof <= 1 {
                    let deg = profile_control_points.len() - 1;
                    (deg, profile_control_points.clone(), profile_weights.clone())
                } else {
                    let p_deg = *profile_degree as usize;
                    let cps_per_span = p_deg + 1;
                    let mut cps = Vec::new();
                    let mut ws = Vec::new();
                    for span_j in 0..n_prof {
                        let base = span_j * cps_per_span;
                        let start_k = if span_j == 0 { 0 } else { 1 };
                        for k in start_k..cps_per_span {
                            cps.push(profile_control_points[base + k]);
                            ws.push(profile_weights[base + k]);
                        }
                    }
                    (p_deg, cps, ws)
                };

                let knots_v = if n_prof <= 1 {
                    let mut kv = vec![0.0; v_degree + 1];
                    kv.extend(vec![1.0; v_degree + 1]);
                    kv
                } else {
                    let p_deg = *profile_degree as usize;
                    let mut kv = vec![0.0; p_deg + 1];
                    for i in 1..n_prof {
                        let t = i as f64 / n_prof as f64;
                        for _ in 0..p_deg {
                            kv.push(t);
                        }
                    }
                    kv.extend(vec![1.0; p_deg + 1]);
                    kv
                };

                let n_v_cps = prof_cps.len();
                let mut rows_cp: Vec<Vec<[f64; 3]>> = Vec::with_capacity(n_u_cps);
                let mut rows_w: Vec<Vec<f64>> = Vec::with_capacity(n_u_cps);

                for ui in 0..n_u_cps {
                    let c = u_cos[ui];
                    let s = u_sin[ui];
                    let uw = u_weights[ui];

                    let mut row_cp = Vec::with_capacity(n_v_cps);
                    let mut row_w = Vec::with_capacity(n_v_cps);

                    for vj in 0..n_v_cps {
                        let [r, z] = prof_cps[vj];
                        let pw = prof_ws[vj];
                        let cp = [
                            center[0] + r * (c * bu[0] + s * bv[0]) + z * axis_n[0],
                            center[1] + r * (c * bu[1] + s * bv[1]) + z * axis_n[1],
                            center[2] + r * (c * bu[2] + s * bv[2]) + z * axis_n[2],
                        ];
                        row_cp.push(cp);
                        row_w.push(uw * pw);
                    }

                    rows_cp.push(row_cp);
                    rows_w.push(row_w);
                }

                Some(neco_nurbs::NurbsSurface3D {
                    degree_u: 2,
                    degree_v: v_degree,
                    control_points: rows_cp,
                    weights: rows_w,
                    knots_u,
                    knots_v,
                })
            }
            _ => None,
        }
    }

    /// Whether the surface supports analytical impostor rendering.
    pub fn is_analytical(&self) -> bool {
        !matches!(self, Surface::NurbsSurface { .. })
    }

    /// Default parameter range (u_min, u_max, v_min, v_max).
    pub fn param_range(&self) -> (f64, f64, f64, f64) {
        use std::f64::consts::{PI, TAU};
        match self {
            Surface::Plane { .. } => (0.0, 1.0, 0.0, 1.0),
            Surface::Cylinder { axis, .. } => (0.0, TAU, 0.0, vec3::length(*axis)),
            Surface::Cone { axis, .. } => (0.0, TAU, 0.0, vec3::length(*axis)),
            Surface::Sphere { .. } => (0.0, TAU, 0.0, PI),
            Surface::Ellipsoid { .. } => (0.0, TAU, 0.0, PI),
            Surface::Torus { .. } => (0.0, TAU, 0.0, TAU),
            Surface::SurfaceOfRevolution {
                theta_start,
                theta_range,
                ..
            } => (*theta_start, *theta_start + *theta_range, 0.0, 1.0),
            Surface::SurfaceOfSweep { .. } => (0.0, 1.0, 0.0, 1.0),
            Surface::NurbsSurface { data } => {
                let (u0, u1) = data.u_range();
                let (v0, v1) = data.v_range();
                (u0, u1, v0, v1)
            }
        }
    }

    /// Inverse-project a 3D point to surface parameters.
    pub fn inverse_project(&self, p: &[f64; 3]) -> Option<(f64, f64)> {
        match self {
            Surface::Plane { origin, normal } => {
                let n = vec3::normalized(*normal);
                let up = if n[0].abs() < 0.9 {
                    [1.0, 0.0, 0.0]
                } else {
                    [0.0, 1.0, 0.0]
                };
                let u_vec = vec3::normalized(vec3::cross(n, up));
                let v_vec = vec3::cross(n, u_vec);
                let d = vec3::sub(*p, *origin);
                Some((vec3::dot(d, u_vec), vec3::dot(d, v_vec)))
            }
            Surface::Cylinder { origin, axis, .. } => {
                let axis_len = vec3::length(*axis);
                if axis_len < 1e-30 {
                    return None;
                }
                let axis_n = vec3::normalized(*axis);
                let (bu, bv) = vec3::orthonormal_basis(*axis);
                let q = vec3::sub(*p, *origin);
                let h = vec3::dot(q, axis_n).clamp(0.0, axis_len);
                let q_perp = vec3::sub(q, vec3::scale(axis_n, h));
                let u = vec3::dot(q_perp, bv)
                    .atan2(vec3::dot(q_perp, bu))
                    .rem_euclid(std::f64::consts::TAU);
                Some((u, h))
            }
            Surface::Cone { origin, axis, .. } => {
                let axis_len = vec3::length(*axis);
                if axis_len < 1e-30 {
                    return None;
                }
                let axis_n = vec3::normalized(*axis);
                let (bu, bv) = vec3::orthonormal_basis(*axis);
                let q = vec3::sub(*p, *origin);
                let h = vec3::dot(q, axis_n).clamp(0.0, axis_len);
                let q_perp = vec3::sub(q, vec3::scale(axis_n, h));
                let u = vec3::dot(q_perp, bv)
                    .atan2(vec3::dot(q_perp, bu))
                    .rem_euclid(std::f64::consts::TAU);
                Some((u, h))
            }
            Surface::Sphere { center, radius } => {
                let d = vec3::sub(*p, *center);
                let len = vec3::length(d);
                if len < 1e-30 || radius.abs() < 1e-30 {
                    return None;
                }
                let u = d[1].atan2(d[0]).rem_euclid(std::f64::consts::TAU);
                let cos_v = (d[2] / radius).clamp(-1.0, 1.0);
                let v = cos_v.acos();
                Some((u, v))
            }
            Surface::Ellipsoid { center, rx, ry, rz } => {
                if rx.abs() < 1e-30 || ry.abs() < 1e-30 || rz.abs() < 1e-30 {
                    return None;
                }
                let x = (p[0] - center[0]) / rx;
                let y = (p[1] - center[1]) / ry;
                let z = (p[2] - center[2]) / rz;
                let u = y.atan2(x).rem_euclid(std::f64::consts::TAU);
                let v = z.clamp(-1.0, 1.0).acos();
                Some((u, v))
            }
            Surface::Torus {
                center,
                axis,
                major_radius,
                ..
            } => {
                let axis_n = vec3::normalized(*axis);
                let (bu, bv) = vec3::orthonormal_basis(*axis);
                let q = vec3::sub(*p, *center);
                let axial = vec3::dot(q, axis_n);
                let q_perp = vec3::sub(q, vec3::scale(axis_n, axial));
                let r_perp = vec3::length(q_perp);
                if r_perp < 1e-30 {
                    return None;
                }
                let u = vec3::dot(q_perp, bv)
                    .atan2(vec3::dot(q_perp, bu))
                    .rem_euclid(std::f64::consts::TAU);
                let v = axial
                    .atan2(r_perp - major_radius)
                    .rem_euclid(std::f64::consts::TAU);
                Some((u, v))
            }
            Surface::SurfaceOfRevolution {
                center,
                axis,
                frame_u,
                frame_v,
                profile_control_points,
                profile_weights,
                profile_degree,
                n_profile_spans,
                theta_start,
                theta_range,
            } => {
                let axis_n = vec3::normalized(*axis);
                let bu = vec3::normalized(*frame_u);
                let bv = vec3::normalized(*frame_v);
                let q = vec3::sub(*p, *center);
                let theta_raw = vec3::dot(q, bv).atan2(vec3::dot(q, bu));
                let theta = if *theta_range >= std::f64::consts::TAU - 1e-12 {
                    let offset = (theta_raw - theta_start).rem_euclid(std::f64::consts::TAU);
                    theta_start + offset
                } else {
                    let offset = (theta_raw - theta_start).rem_euclid(std::f64::consts::TAU);
                    if offset <= *theta_range {
                        theta_start + offset
                    } else if offset.min(std::f64::consts::TAU - offset)
                        <= (offset - theta_range).min(std::f64::consts::TAU - offset + theta_range)
                    {
                        *theta_start
                    } else {
                        *theta_start + *theta_range
                    }
                };
                let radial_dir =
                    vec3::add(vec3::scale(bu, theta.cos()), vec3::scale(bv, theta.sin()));
                let rho = vec3::dot(q, radial_dir);
                let z = vec3::dot(q, axis_n);
                let v = find_closest_v_on_profile(
                    rho,
                    z,
                    profile_control_points,
                    profile_weights,
                    *profile_degree,
                    *n_profile_spans,
                );
                Some((theta, v))
            }
            Surface::SurfaceOfSweep { .. } => {
                let (u0, u1, v0, v1) = self.param_range();
                Some(grid_project_surface(self, p, u0, u1, v0, v1))
            }
            Surface::NurbsSurface { data } => {
                let (u0, u1) = data.u_range();
                let (v0, v1) = data.v_range();
                Some(grid_project_surface(self, p, u0, u1, v0, v1))
            }
        }
    }
}

fn grid_project_surface(
    surface: &Surface,
    p: &[f64; 3],
    u0: f64,
    u1: f64,
    v0: f64,
    v1: f64,
) -> (f64, f64) {
    let nu = 16;
    let nv = 16;
    let mut best_u = u0;
    let mut best_v = v0;
    let mut best_dist = f64::INFINITY;

    for iu in 0..=nu {
        for iv in 0..=nv {
            let u = u0 + (u1 - u0) * (iu as f64 / nu as f64);
            let v = v0 + (v1 - v0) * (iv as f64 / nv as f64);
            let pt = surface.evaluate(u, v);
            let d = vec3::distance(pt, *p);
            if d < best_dist {
                best_dist = d;
                best_u = u;
                best_v = v;
            }
        }
    }

    let eps = 1e-8;
    for _ in 0..20 {
        let pt = surface.evaluate(best_u, best_v);
        let diff = vec3::sub(pt, *p);
        if vec3::length(diff) < 1e-12 {
            break;
        }
        let du = vec3::scale(
            vec3::sub(
                surface.evaluate(best_u + eps, best_v),
                surface.evaluate(best_u - eps, best_v),
            ),
            0.5 / eps,
        );
        let dv = vec3::scale(
            vec3::sub(
                surface.evaluate(best_u, best_v + eps),
                surface.evaluate(best_u, best_v - eps),
            ),
            0.5 / eps,
        );

        let a11 = vec3::dot(du, du);
        let a12 = vec3::dot(du, dv);
        let a22 = vec3::dot(dv, dv);
        let b1 = -vec3::dot(du, diff);
        let b2 = -vec3::dot(dv, diff);

        let det = a11 * a22 - a12 * a12;
        if det.abs() < 1e-30 {
            break;
        }

        let delta_u = (a22 * b1 - a12 * b2) / det;
        let delta_v = (a11 * b2 - a12 * b1) / det;
        best_u = (best_u + delta_u).clamp(u0, u1);
        best_v = (best_v + delta_v).clamp(v0, v1);
    }

    (best_u, best_v)
}

/// Evaluate multi-span rational Bezier profile.
pub fn eval_revolution_profile(
    profile_control_points: &[[f64; 2]],
    profile_weights: &[f64],
    profile_degree: u32,
    n_profile_spans: u32,
    v: f64,
) -> (f64, f64) {
    let n_prof = n_profile_spans as usize;
    let p_deg = profile_degree as usize;
    let cps_per_span = p_deg + 1;
    if n_prof <= 1 {
        de_casteljau_rational_2d(profile_control_points, profile_weights, v)
    } else {
        let v_clamped = v.clamp(0.0, 1.0);
        let span_f = v_clamped * n_prof as f64;
        let span_idx = (span_f as usize).min(n_prof - 1);
        let local_v = span_f - span_idx as f64;
        let start = span_idx * cps_per_span;
        let end = start + cps_per_span;
        de_casteljau_rational_2d(
            &profile_control_points[start..end],
            &profile_weights[start..end],
            local_v,
        )
    }
}

/// Find the closest v parameter on the profile to (rho, z).
pub fn find_closest_v_on_profile(
    rho: f64,
    z: f64,
    profile_control_points: &[[f64; 2]],
    profile_weights: &[f64],
    profile_degree: u32,
    n_profile_spans: u32,
) -> f64 {
    let n_prof = n_profile_spans as usize;
    let n_search = if n_prof <= 1 { 32 } else { n_prof * 16 };
    let mut best_v = 0.0;
    let mut best_dist_sq = f64::INFINITY;

    for i in 0..=n_search {
        let v = i as f64 / n_search as f64;
        let (r, zz) = eval_revolution_profile(
            profile_control_points,
            profile_weights,
            profile_degree,
            n_profile_spans,
            v,
        );
        let dr = rho - r;
        let dz = z - zz;
        let d2 = dr * dr + dz * dz;
        if d2 < best_dist_sq {
            best_dist_sq = d2;
            best_v = v;
        }
    }

    let eps = 1e-8;
    for _ in 0..20 {
        let (r, zz) = eval_revolution_profile(
            profile_control_points,
            profile_weights,
            profile_degree,
            n_profile_spans,
            best_v,
        );
        let (r_p, zz_p) = eval_revolution_profile(
            profile_control_points,
            profile_weights,
            profile_degree,
            n_profile_spans,
            (best_v + eps).min(1.0),
        );
        let (r_m, zz_m) = eval_revolution_profile(
            profile_control_points,
            profile_weights,
            profile_degree,
            n_profile_spans,
            (best_v - eps).max(0.0),
        );

        let dr_dv = (r_p - r_m) / (2.0 * eps);
        let dz_dv = (zz_p - zz_m) / (2.0 * eps);

        let residual_r = rho - r;
        let residual_z = z - zz;
        let f_val = residual_r * (-dr_dv) + residual_z * (-dz_dv);
        let df_val = dr_dv * dr_dv + dz_dv * dz_dv;

        if df_val.abs() < 1e-20 {
            break;
        }

        let dv = f_val / df_val;
        best_v = (best_v - dv).clamp(0.0, 1.0);

        if dv.abs() < 1e-12 {
            break;
        }
    }

    best_v
}

/// 3D curve.
#[derive(Debug, Clone)]
pub enum Curve3D {
    Line {
        start: [f64; 3],
        end: [f64; 3],
    },
    Arc {
        center: [f64; 3],
        axis: [f64; 3],
        start: [f64; 3],
        end: [f64; 3],
        radius: f64,
    },
    Ellipse {
        center: [f64; 3],
        axis_u: [f64; 3],
        axis_v: [f64; 3],
        t_start: f64,
        t_end: f64,
    },
    NurbsCurve3D {
        degree: usize,
        control_points: Vec<[f64; 3]>,
        weights: Vec<f64>,
        knots: Vec<f64>,
    },
}

impl Curve3D {
    pub fn evaluate(&self, t: f64) -> [f64; 3] {
        match self {
            Curve3D::Line { start, end } => {
                vec3::add(*start, vec3::scale(vec3::sub(*end, *start), t))
            }
            Curve3D::Arc {
                center,
                axis,
                start,
                radius,
                ..
            } => {
                let r_vec = vec3::sub(*start, *center);
                let tangent = vec3::scale(vec3::normalized(vec3::cross(*axis, r_vec)), *radius);
                let cos_t = t.cos();
                let sin_t = t.sin();
                vec3::add(
                    vec3::add(*center, vec3::scale(r_vec, cos_t)),
                    vec3::scale(tangent, sin_t),
                )
            }
            Curve3D::Ellipse {
                center,
                axis_u,
                axis_v,
                ..
            } => vec3::add(
                vec3::add(*center, vec3::scale(*axis_u, t.cos())),
                vec3::scale(*axis_v, t.sin()),
            ),
            Curve3D::NurbsCurve3D {
                degree,
                control_points,
                weights,
                knots,
            } => {
                let n = control_points.len();
                let p = *degree;
                let t_min = knots[p];
                let t_max = knots[n];
                let t = t.clamp(t_min, t_max);

                let k = {
                    if t >= knots[n] {
                        n - 1
                    } else {
                        let mut lo = p;
                        let mut hi = n;
                        while lo < hi {
                            let mid = (lo + hi) / 2;
                            if t < knots[mid] {
                                hi = mid;
                            } else {
                                lo = mid + 1;
                            }
                        }
                        lo - 1
                    }
                };

                let mut hx = vec![0.0; p + 1];
                let mut hy = vec![0.0; p + 1];
                let mut hz = vec![0.0; p + 1];
                let mut hw = vec![0.0; p + 1];

                for j in 0..=p {
                    let idx = k - p + j;
                    let w = weights[idx];
                    hx[j] = control_points[idx][0] * w;
                    hy[j] = control_points[idx][1] * w;
                    hz[j] = control_points[idx][2] * w;
                    hw[j] = w;
                }

                for r in 1..=p {
                    for j in (r..=p).rev() {
                        let left = k + j - p;
                        let right = k + 1 + j - r;
                        let denom = knots[right] - knots[left];
                        if denom.abs() < 1e-30 {
                            continue;
                        }
                        let alpha = (t - knots[left]) / denom;
                        hx[j] = (1.0 - alpha) * hx[j - 1] + alpha * hx[j];
                        hy[j] = (1.0 - alpha) * hy[j - 1] + alpha * hy[j];
                        hz[j] = (1.0 - alpha) * hz[j - 1] + alpha * hz[j];
                        hw[j] = (1.0 - alpha) * hw[j - 1] + alpha * hw[j];
                    }
                }

                let w = hw[p];
                if w.abs() < 1e-30 {
                    control_points.last().copied().unwrap_or([0.0; 3])
                } else {
                    [hx[p] / w, hy[p] / w, hz[p] / w]
                }
            }
        }
    }

    pub fn midpoint(&self) -> [f64; 3] {
        match self {
            Curve3D::Line { start, end } => vec3::scale(vec3::add(*start, *end), 0.5),
            Curve3D::Arc { .. } => self.evaluate(0.5),
            Curve3D::Ellipse { t_start, t_end, .. } => self.evaluate((t_start + t_end) * 0.5),
            Curve3D::NurbsCurve3D { degree, knots, .. } => {
                let p = *degree;
                let n = knots.len() - p - 1;
                let mid = (knots[p] + knots[n]) * 0.5;
                self.evaluate(mid)
            }
        }
    }

    /// Parameter range of the curve.
    pub fn param_range(&self) -> (f64, f64) {
        match self {
            Curve3D::Line { .. } => (0.0, 1.0),
            Curve3D::Arc {
                start,
                end,
                center,
                axis,
                radius,
            } => {
                let r_start = vec3::sub(*start, *center);
                let tangent = vec3::scale(vec3::normalized(vec3::cross(*axis, r_start)), *radius);
                let r_end = vec3::sub(*end, *center);
                let r_start_len = vec3::length(r_start);
                let r_end_len = vec3::length(r_end);
                let cos_angle = vec3::dot(r_start, r_end) / (r_start_len * r_end_len);
                let sin_angle = vec3::dot(tangent, r_end) / (vec3::length(tangent) * r_end_len);
                let angle = sin_angle.atan2(cos_angle);
                let angle = if angle.abs() < 1e-10 && vec3::length(vec3::sub(*start, *end)) < 1e-10
                {
                    std::f64::consts::TAU
                } else {
                    angle.max(0.0)
                };
                (0.0, angle)
            }
            Curve3D::Ellipse { t_start, t_end, .. } => (*t_start, *t_end),
            Curve3D::NurbsCurve3D { degree, knots, .. } => {
                let p = *degree;
                let n = knots.len() - p - 1;
                (knots[p], knots[n])
            }
        }
    }

    /// Convert to polyline via adaptive sampling.
    pub fn to_polyline(&self, chord_tol: f64) -> Vec<[f64; 3]> {
        match self {
            Curve3D::Line { start, end } => vec![*start, *end],
            _ => {
                let (t0, t1) = self.param_range();
                let mut points = vec![self.evaluate(t0)];
                self.adaptive_sample(t0, t1, chord_tol, &mut points);
                points
            }
        }
    }

    fn adaptive_sample(&self, t0: f64, t1: f64, tol: f64, points: &mut Vec<[f64; 3]>) {
        if (t1 - t0).abs() < 1e-10 {
            points.push(self.evaluate(t1));
            return;
        }
        let mid_t = (t0 + t1) * 0.5;
        let p0 = self.evaluate(t0);
        let p1 = self.evaluate(t1);
        let mid_curve = self.evaluate(mid_t);
        let mid_chord = vec3::scale(vec3::add(p0, p1), 0.5);
        let chord_height = vec3::length(vec3::sub(mid_curve, mid_chord));
        if chord_height > tol {
            self.adaptive_sample(t0, mid_t, tol, points);
            self.adaptive_sample(mid_t, t1, tol, points);
        } else {
            points.push(self.evaluate(t1));
        }
    }
}

/// Oriented edge reference.
#[derive(Debug, Clone, Copy)]
pub struct EdgeRef {
    pub edge_id: EdgeId,
    pub forward: bool,
}

/// Topological edge.
#[derive(Debug, Clone)]
pub struct Edge {
    pub v_start: VertexId,
    pub v_end: VertexId,
    pub curve: Curve3D,
}

/// Face (loop of edge references + surface).
#[derive(Debug, Clone)]
pub struct Face {
    pub loop_edges: Vec<EdgeRef>,
    pub surface: Surface,
    pub orientation_reversed: bool,
}

/// Sub-face: intermediate representation for boolean operations.
#[derive(Clone, Debug)]
pub struct SubFace {
    pub surface: Surface,
    pub polygon: Vec<[f64; 3]>,
    pub candidate_curves: Vec<Curve3D>,
    pub flipped: bool,
    pub source_shell: usize,
    pub source_face: usize,
}

impl SubFace {
    pub fn centroid(&self) -> [f64; 3] {
        let n = self.polygon.len() as f64;
        let (sx, sy, sz) = self.polygon.iter().fold((0.0, 0.0, 0.0), |(x, y, z), p| {
            (x + p[0], y + p[1], z + p[2])
        });
        [sx / n, sy / n, sz / n]
    }
}

/// B-Rep shell.
#[derive(Debug, Clone, Default)]
pub struct Shell {
    pub vertices: Vec<[f64; 3]>,
    pub edges: Vec<Edge>,
    pub faces: Vec<Face>,
}

impl Shell {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            edges: Vec::new(),
            faces: Vec::new(),
        }
    }

    /// Add vertex, reusing an existing one within tolerance.
    pub fn add_vertex(&mut self, p: [f64; 3]) -> VertexId {
        for (i, v) in self.vertices.iter().enumerate() {
            if vec3::distance(*v, p) < VERTEX_TOL {
                return i;
            }
        }
        let id = self.vertices.len();
        self.vertices.push(p);
        id
    }

    /// Add an edge.
    pub fn add_edge(&mut self, v_start: VertexId, v_end: VertexId, curve: Curve3D) -> EdgeId {
        let id = self.edges.len();
        self.edges.push(Edge {
            v_start,
            v_end,
            curve,
        });
        id
    }

    /// Bounding box enclosing all vertices and sampled curve edges.
    pub fn bounding_box(&self) -> ([f64; 3], [f64; 3]) {
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];

        let mut extend = |p: [f64; 3]| {
            for i in 0..3 {
                min[i] = min[i].min(p[i]);
                max[i] = max[i].max(p[i]);
            }
        };

        for v in &self.vertices {
            extend(*v);
        }

        for edge in &self.edges {
            match &edge.curve {
                Curve3D::Line { .. } => {}
                curve => {
                    let polyline = curve.to_polyline(0.01);
                    for p in &polyline {
                        extend(*p);
                    }
                }
            }
        }

        (min, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sphere_evaluate_poles() {
        let s = Surface::Sphere {
            center: [0.0, 0.0, 0.0],
            radius: 1.0,
        };
        let north = s.evaluate(0.0, 0.0);
        assert!((north[2] - 1.0).abs() < 1e-10);
        let south = s.evaluate(0.0, std::f64::consts::PI);
        assert!((south[2] + 1.0).abs() < 1e-10);
    }

    #[test]
    fn sphere_normal_outward() {
        let s = Surface::Sphere {
            center: [1.0, 2.0, 3.0],
            radius: 2.0,
        };
        let n = s.normal_at(0.0, std::f64::consts::FRAC_PI_2);
        let p = s.evaluate(0.0, std::f64::consts::FRAC_PI_2);
        let expected_x = (p[0] - 1.0) / 2.0;
        assert!((n[0] - expected_x).abs() < 1e-10);
    }

    #[test]
    fn cylinder_evaluate() {
        let s = Surface::Cylinder {
            origin: [0.0, 0.0, 0.0],
            axis: [0.0, 0.0, 2.0],
            radius: 1.0,
        };
        let p = s.evaluate(0.0, 1.0);
        assert!((p[2] - 1.0).abs() < 1e-10);
        let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
        assert!((r - 1.0).abs() < 1e-10);
    }

    fn make_multispan_revolution() -> Surface {
        let profile_control_points = vec![
            [1.0, 0.0],
            [1.0, 1.0],
            [1.0, 2.0],
            [2.0, 3.0],
            [2.0, 3.0],
            [2.0, 4.0],
            [2.0, 5.0],
            [1.0, 6.0],
        ];
        let profile_weights = vec![1.0; 8];
        Surface::SurfaceOfRevolution {
            center: [0.0, 0.0, 0.0],
            axis: [0.0, 0.0, 1.0],
            frame_u: [1.0, 0.0, 0.0],
            frame_v: [0.0, 1.0, 0.0],
            profile_control_points,
            profile_weights,
            profile_degree: 3,
            n_profile_spans: 2,
            theta_start: 0.0,
            theta_range: std::f64::consts::TAU,
        }
    }

    #[test]
    fn revolution_multispan_evaluate_endpoints() {
        let s = make_multispan_revolution();
        let p0 = s.evaluate(0.0, 0.0);
        assert!((p0[0] - 1.0).abs() < 1e-10);
        assert!(p0[2].abs() < 1e-10);

        let p1 = s.evaluate(0.0, 1.0);
        assert!((p1[0] - 1.0).abs() < 1e-10);
        assert!((p1[2] - 6.0).abs() < 1e-10);
    }

    #[test]
    fn revolution_multispan_evaluate_midpoint() {
        let s = make_multispan_revolution();
        let pm = s.evaluate(0.0, 0.5);
        assert!((pm[0] - 2.0).abs() < 1e-10);
        assert!((pm[2] - 3.0).abs() < 1e-10);
    }

    #[test]
    fn revolution_multispan_normal_at() {
        let s = make_multispan_revolution();
        let n = s.normal_at(0.0, 0.5);
        let len = vec3::length(n);
        assert!((len - 1.0).abs() < 1e-6);
    }

    #[test]
    fn param_range_sphere() {
        let s = Surface::Sphere {
            center: [0.0, 0.0, 0.0],
            radius: 1.0,
        };
        let (u0, u1, v0, v1) = s.param_range();
        assert!(u0.abs() < 1e-10);
        assert!((u1 - std::f64::consts::TAU).abs() < 1e-10);
        assert!(v0.abs() < 1e-10);
        assert!((v1 - std::f64::consts::PI).abs() < 1e-10);
    }

    #[test]
    fn sphere_inverse_project_round_trip() {
        let s = Surface::Sphere {
            center: [1.0, 2.0, 3.0],
            radius: 2.0,
        };
        let p = s.evaluate(0.7, 1.2);
        let (u, v) = s.inverse_project(&p).unwrap();
        let q = s.evaluate(u, v);
        assert!(vec3::distance(p, q) < 1e-10);
    }

    #[test]
    fn torus_inverse_project_round_trip() {
        let s = Surface::Torus {
            center: [0.0, 0.0, 0.0],
            axis: [0.0, 0.0, 1.0],
            major_radius: 3.0,
            minor_radius: 1.0,
        };
        let p = s.evaluate(1.1, 2.2);
        let (u, v) = s.inverse_project(&p).unwrap();
        let q = s.evaluate(u, v);
        assert!(vec3::distance(p, q) < 1e-10);
    }

    #[test]
    fn revolution_inverse_project_round_trip() {
        let s = make_multispan_revolution();
        let p = s.evaluate(1.3, 0.4);
        let (u, v) = s.inverse_project(&p).unwrap();
        let q = s.evaluate(u, v);
        assert!(vec3::distance(p, q) < 1e-6);
    }
}
