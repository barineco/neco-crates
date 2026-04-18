use neco_view2d::View2d;
use neco_view2d_svg::{world_points_to_polyline, world_points_to_svg_d, world_transform_attr};

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

fn parse_transform(input: &str) -> (f64, f64, f64, f64) {
    let translate_start = input.find("translate(").expect("translate") + "translate(".len();
    let translate_end =
        input[translate_start..].find(')').expect("translate end") + translate_start;
    let scale_start = input.find("scale(").expect("scale") + "scale(".len();
    let scale_end = input[scale_start..].find(')').expect("scale end") + scale_start;

    let (tx, ty) = input[translate_start..translate_end]
        .split_once(',')
        .expect("translate pair");
    let (sx, sy) = input[scale_start..scale_end]
        .split_once(',')
        .expect("scale pair");

    (
        tx.parse().expect("tx"),
        ty.parse().expect("ty"),
        sx.parse().expect("sx"),
        sy.parse().expect("sy"),
    )
}

#[test]
fn roundtrip_matches_world_to_canvas() {
    let view = View2d {
        center_x: 50.0,
        center_y: -20.0,
        view_size: 8.0,
    };
    let (cw, ch) = (1280.0, 720.0);
    let points = [(0.0, 0.0), (10.0, 5.0), (-3.0, 7.0), (42.0, -15.0)];

    let polyline = world_points_to_polyline(&view, &points, cw, ch);
    let path_d = world_points_to_svg_d(&view, &points, cw, ch);
    let transform = world_transform_attr(&view, cw, ch);

    let parsed_polyline = parse_points(&polyline);
    let parsed_path = parse_path(&path_d);
    let (tx, ty, sx, sy) = parse_transform(&transform);

    for (((wx, wy), poly), path) in points
        .iter()
        .zip(parsed_polyline.iter())
        .zip(parsed_path.iter())
    {
        let expected = view.world_to_canvas(*wx, *wy, cw, ch);
        assert!((poly.0 - expected.0).abs() < EPS);
        assert!((poly.1 - expected.1).abs() < EPS);
        assert!((path.0 - expected.0).abs() < EPS);
        assert!((path.1 - expected.1).abs() < EPS);

        let transform_x = tx + sx * wx;
        let transform_y = ty + sy * wy;
        assert!((transform_x - expected.0).abs() < EPS);
        assert!((transform_y - expected.1).abs() < EPS);
    }
}
