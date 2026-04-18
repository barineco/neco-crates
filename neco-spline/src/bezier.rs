use crate::CubicSpline;

/// A single cubic Bezier segment in 2D `(x, y)` space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BezierCubic {
    pub p0: (f32, f32),
    pub p1: (f32, f32),
    pub p2: (f32, f32),
    pub p3: (f32, f32),
}

impl CubicSpline {
    /// Convert the spline into cubic Bezier segments.
    pub fn to_bezier_segments(&self) -> Vec<BezierCubic> {
        if self.points.len() < 2 {
            return Vec::new();
        }

        self.points
            .windows(2)
            .zip(self.coefficients.iter())
            .map(|(points, coefficients)| {
                let x0 = points[0].0;
                let x3 = points[1].0;
                let h = x3 - x0;
                let [a, b, c, d] = *coefficients;

                let y0 = a;
                let y1 = a + (b * h) / 3.0;
                let y2 = a + (2.0 * b * h) / 3.0 + (c * h * h) / 3.0;
                let y3 = a + b * h + c * h * h + d * h * h * h;

                BezierCubic {
                    p0: (x0, y0),
                    p1: (x0 + h / 3.0, y1),
                    p2: (x0 + (2.0 * h) / 3.0, y2),
                    p3: (x3, y3),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evaluate_bezier_y(segment: BezierCubic, t: f32) -> f32 {
        let u = 1.0 - t;
        let b0 = u * u * u;
        let b1 = 3.0 * u * u * t;
        let b2 = 3.0 * u * t * t;
        let b3 = t * t * t;
        b0 * segment.p0.1 + b1 * segment.p1.1 + b2 * segment.p2.1 + b3 * segment.p3.1
    }

    #[test]
    fn bezier_segments_roundtrip_three_points() {
        let points = [(0.0, 0.0), (0.5, 0.8), (1.0, 1.0)];
        let spline = CubicSpline::new(&points).unwrap();
        let segments = spline.to_bezier_segments();

        assert_eq!(segments.len(), points.len() - 1);

        for (segment_index, segment) in segments.iter().copied().enumerate() {
            let x0 = points[segment_index].0;
            let x1 = points[segment_index + 1].0;
            let h = x1 - x0;
            for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let x = x0 + h * t;
                let spline_y = spline.evaluate(x);
                let bezier_y = evaluate_bezier_y(segment, t);
                assert!(
                    (spline_y - bezier_y).abs() < 1e-5,
                    "segment {segment_index}, t={t}: spline_y={spline_y}, bezier_y={bezier_y}"
                );
            }
        }
    }

    #[test]
    fn bezier_segments_roundtrip_five_points_and_join_properties() {
        let points = [(0.0, 0.0), (0.3, 0.5), (0.7, 0.2), (1.2, 1.1), (1.5, 0.9)];
        let spline = CubicSpline::new(&points).unwrap();
        let segments = spline.to_bezier_segments();

        assert_eq!(segments.len(), points.len() - 1);

        for (segment_index, segment) in segments.iter().copied().enumerate() {
            assert!(segment.p0.0 < segment.p1.0);
            assert!(segment.p1.0 < segment.p2.0);
            assert!(segment.p2.0 < segment.p3.0);

            let x0 = points[segment_index].0;
            let x1 = points[segment_index + 1].0;
            let h = x1 - x0;
            for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let x = x0 + h * t;
                let spline_y = spline.evaluate(x);
                let bezier_y = evaluate_bezier_y(segment, t);
                assert!(
                    (spline_y - bezier_y).abs() < 1e-5,
                    "segment {segment_index}, t={t}: spline_y={spline_y}, bezier_y={bezier_y}"
                );
            }
        }

        for pair in segments.windows(2) {
            assert!((pair[0].p3.0 - pair[1].p0.0).abs() < 1e-6);
            assert!((pair[0].p3.1 - pair[1].p0.1).abs() < 1e-5);
        }
    }
}
