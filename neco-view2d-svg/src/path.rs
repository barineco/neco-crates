use alloc::{format, string::String, vec::Vec};
use neco_view2d::View2d;

use crate::{fmt::format_f64, svg_coord::world_to_svg};

/// Return an SVG `d` string for finite world inputs.
/// Non-finite values may be emitted as `NaN` or `inf` strings.
pub fn world_points_to_svg_d(
    view: &View2d,
    points: &[(f64, f64)],
    canvas_w: f64,
    canvas_h: f64,
) -> String {
    if points.is_empty() {
        return String::new();
    }

    let commands: Vec<String> = points
        .iter()
        .enumerate()
        .map(|(index, &(wx, wy))| {
            let (sx, sy) = world_to_svg(view, wx, wy, canvas_w, canvas_h);
            let prefix = if index == 0 { "M" } else { "L" };
            format!("{prefix} {},{}", format_f64(sx), format_f64(sy))
        })
        .collect();

    commands.join(" ")
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::world_points_to_svg_d;
    use neco_view2d::View2d;

    const EPS: f64 = 1e-5;

    fn parse_path(input: &str) -> Vec<(f64, f64)> {
        if input.is_empty() {
            return Vec::new();
        }
        input
            .split(" L ")
            .enumerate()
            .map(|(index, segment)| {
                let point = if index == 0 {
                    segment.strip_prefix("M ").expect("move")
                } else {
                    segment
                };
                let (x, y) = point.split_once(',').expect("point pair");
                (x.parse().expect("x"), y.parse().expect("y"))
            })
            .collect()
    }

    #[test]
    fn empty_input_returns_empty_string() {
        let out = world_points_to_svg_d(&View2d::default(), &[], 800.0, 600.0);
        assert_eq!(out, "");
    }

    #[test]
    fn single_point_uses_only_move() {
        let out = world_points_to_svg_d(&View2d::default(), &[(0.0, 0.0)], 800.0, 600.0);
        assert!(out.starts_with("M "));
        assert!(!out.contains(" L "));
    }

    #[test]
    fn multiple_points_use_line_segments() {
        let out = world_points_to_svg_d(
            &View2d::default(),
            &[(0.0, 0.0), (1.0, 1.0), (2.0, 3.0)],
            800.0,
            600.0,
        );
        assert_eq!(out.matches(" L ").count(), 2);
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
        let out = world_points_to_svg_d(&view, &points, cw, ch);
        let parsed = parse_path(&out);
        assert_eq!(parsed.len(), points.len());
        for (parsed_point, world_point) in parsed.iter().zip(points.iter()) {
            let expected = view.world_to_canvas(world_point.0, world_point.1, cw, ch);
            assert!((parsed_point.0 - expected.0).abs() < EPS);
            assert!((parsed_point.1 - expected.1).abs() < EPS);
        }
    }

    #[test]
    fn single_point_does_not_emit_line_segment() {
        let out = world_points_to_svg_d(&View2d::default(), &[(1.0, 2.0)], 800.0, 600.0);
        assert!(!out.contains(" L "));
    }
}
