use alloc::{string::String, vec::Vec};
use neco_view2d::View2d;

use crate::{fmt::format_f64, svg_coord::world_to_svg};

/// Return an SVG `points` string for finite world inputs.
/// Non-finite values may be emitted as `NaN` or `inf` strings.
pub fn world_points_to_polyline(
    view: &View2d,
    points: &[(f64, f64)],
    canvas_w: f64,
    canvas_h: f64,
) -> String {
    if points.is_empty() {
        return String::new();
    }

    let segments: Vec<String> = points
        .iter()
        .map(|&(wx, wy)| {
            let (sx, sy) = world_to_svg(view, wx, wy, canvas_w, canvas_h);
            let mut segment = format_f64(sx);
            segment.push(',');
            segment.push_str(&format_f64(sy));
            segment
        })
        .collect();

    segments.join(" ")
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::world_points_to_polyline;
    use neco_view2d::View2d;

    const EPS: f64 = 1e-5;

    fn parse_points(input: &str) -> Vec<(f64, f64)> {
        if input.is_empty() {
            return Vec::new();
        }
        input
            .split(' ')
            .map(|pair| {
                let (x, y) = pair.split_once(',').expect("point pair");
                (x.parse().expect("x"), y.parse().expect("y"))
            })
            .collect()
    }

    #[test]
    fn empty_input_returns_empty_string() {
        let out = world_points_to_polyline(&View2d::default(), &[], 800.0, 600.0);
        assert_eq!(out, "");
    }

    #[test]
    fn one_point_has_no_trailing_space() {
        let out = world_points_to_polyline(&View2d::default(), &[(0.0, 0.0)], 800.0, 600.0);
        assert!(!out.ends_with(' '));
        assert!(out.contains(','));
    }

    #[test]
    fn two_points_use_single_space_separator() {
        let out =
            world_points_to_polyline(&View2d::default(), &[(0.0, 0.0), (1.0, 1.0)], 800.0, 600.0);
        assert_eq!(out.matches(' ').count(), 1);
        assert!(!out.ends_with(' '));
    }

    #[test]
    fn parsed_points_match_world_to_canvas() {
        let view = View2d {
            center_x: 50.0,
            center_y: -20.0,
            view_size: 8.0,
        };
        let points = [(0.0, 0.0), (10.0, 5.0), (-3.0, 7.0)];
        let (cw, ch) = (1280.0, 720.0);
        let out = world_points_to_polyline(&view, &points, cw, ch);
        let parsed = parse_points(&out);
        assert_eq!(parsed.len(), points.len());
        for (parsed_point, world_point) in parsed.iter().zip(points.iter()) {
            let expected = view.world_to_canvas(world_point.0, world_point.1, cw, ch);
            assert!((parsed_point.0 - expected.0).abs() < EPS);
            assert!((parsed_point.1 - expected.1).abs() < EPS);
        }
    }

    #[test]
    fn locale_uses_dot_for_decimals() {
        let out = world_points_to_polyline(&View2d::default(), &[(0.5, 0.25)], 800.0, 600.0);
        let (x, y) = out.split_once(',').expect("point pair");
        assert_eq!(out.matches(',').count(), 1);
        assert!(!x.contains(','));
        assert!(!y.contains(','));
    }
}
