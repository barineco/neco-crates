use alloc::{format, string::String};
use neco_view2d::View2d;

use crate::fmt::format_f64;

/// Return `translate(tx,ty) scale(sx,sy)` for finite world inputs.
/// Non-finite values may be emitted as `NaN` or `inf` strings.
pub fn world_transform_attr(view: &View2d, canvas_w: f64, canvas_h: f64) -> String {
    let (ox, oy) = view.world_to_canvas(0.0, 0.0, canvas_w, canvas_h);
    let (ux, _) = view.world_to_canvas(1.0, 0.0, canvas_w, canvas_h);
    let (_, vy) = view.world_to_canvas(0.0, 1.0, canvas_w, canvas_h);
    let sx = ux - ox;
    let sy = vy - oy;

    format!(
        "translate({},{}) scale({},{})",
        format_f64(ox),
        format_f64(oy),
        format_f64(sx),
        format_f64(sy)
    )
}

#[cfg(test)]
mod tests {
    use super::world_transform_attr;
    use crate::svg_coord::world_to_svg;
    use neco_view2d::View2d;

    const EPS: f64 = 1e-5;

    fn parse_transform(input: &str) -> (f64, f64, f64, f64) {
        let translate_start = input.find("translate(").expect("translate") + "translate(".len();
        let translate_end =
            input[translate_start..].find(')').expect("translate end") + translate_start;
        let scale_start = input.find("scale(").expect("scale") + "scale(".len();
        let scale_end = input[scale_start..].find(')').expect("scale end") + scale_start;

        let translate = &input[translate_start..translate_end];
        let scale = &input[scale_start..scale_end];

        let (tx, ty) = translate.split_once(',').expect("translate pair");
        let (sx, sy) = scale.split_once(',').expect("scale pair");

        (
            tx.parse().expect("tx"),
            ty.parse().expect("ty"),
            sx.parse().expect("sx"),
            sy.parse().expect("sy"),
        )
    }

    #[test]
    fn origin_matches_translate() {
        let view = View2d::default();
        let out = world_transform_attr(&view, 800.0, 600.0);
        let (tx, ty, _, _) = parse_transform(&out);
        let (ox, oy) = world_to_svg(&view, 0.0, 0.0, 800.0, 600.0);
        assert!((tx - ox).abs() < EPS);
        assert!((ty - oy).abs() < EPS);
    }

    #[test]
    fn matches_world_to_canvas_for_arbitrary_point() {
        let view = View2d {
            center_x: 50.0,
            center_y: -20.0,
            view_size: 8.0,
        };
        let (cw, ch) = (1280.0, 720.0);
        let (tx, ty, sx, sy) = parse_transform(&world_transform_attr(&view, cw, ch));
        let (wx, wy) = (42.0, -15.0);
        let (expected_x, expected_y) = view.world_to_canvas(wx, wy, cw, ch);
        let actual_x = tx + sx * wx;
        let actual_y = ty + sy * wy;
        assert!((actual_x - expected_x).abs() < EPS);
        assert!((actual_y - expected_y).abs() < EPS);
    }

    #[test]
    fn scale_tracks_view_size() {
        let base = View2d::default();
        let zoomed_out = View2d {
            center_x: 0.0,
            center_y: 0.0,
            view_size: 10.0,
        };
        let (_, _, base_sx, base_sy) = parse_transform(&world_transform_attr(&base, 800.0, 600.0));
        let (_, _, zoom_sx, zoom_sy) =
            parse_transform(&world_transform_attr(&zoomed_out, 800.0, 600.0));
        assert!((zoom_sx - base_sx / 10.0).abs() < EPS);
        assert!((zoom_sy - base_sy / 10.0).abs() < EPS);
    }

    #[test]
    fn aspect_basis_matches_non_square_canvas() {
        let view = View2d::default();
        for (cw, ch) in [(1600.0, 800.0), (400.0, 800.0)] {
            let (_, _, sx, sy) = parse_transform(&world_transform_attr(&view, cw, ch));
            let (ox, oy) = view.world_to_canvas(0.0, 0.0, cw, ch);
            let (ux, uy) = view.world_to_canvas(1.0, 0.0, cw, ch);
            let (vx, vy) = view.world_to_canvas(0.0, 1.0, cw, ch);
            assert!((sx - (ux - ox)).abs() < EPS);
            assert!((sy - (vy - oy)).abs() < EPS);
            assert!((uy - oy).abs() < EPS);
            assert!((vx - ox).abs() < EPS);
        }
    }

    #[test]
    fn output_uses_translate_and_two_argument_scale() {
        let out = world_transform_attr(&View2d::default(), 800.0, 600.0);
        assert!(out.starts_with("translate("));
        assert!(out.contains(") scale("));
        assert!(out.ends_with(')'));
        let scale_start = out.find("scale(").expect("scale") + "scale(".len();
        let scale_end = out[scale_start..].find(')').expect("scale end") + scale_start;
        let scale_args = &out[scale_start..scale_end];
        assert!(scale_args.contains(','));
        let numeric_chars = out.chars().all(|ch| {
            ch.is_ascii_digit()
                || matches!(
                    ch,
                    't' | 'r'
                        | 'a'
                        | 'n'
                        | 's'
                        | 'l'
                        | 'e'
                        | 'c'
                        | '('
                        | ')'
                        | ','
                        | '.'
                        | '-'
                        | ' '
                )
        });
        assert!(numeric_chars);
    }
}
