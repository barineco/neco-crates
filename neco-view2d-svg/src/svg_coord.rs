use neco_view2d::View2d;

pub(crate) fn world_to_svg(
    view: &View2d,
    wx: f64,
    wy: f64,
    canvas_w: f64,
    canvas_h: f64,
) -> (f64, f64) {
    view.world_to_canvas(wx, wy, canvas_w, canvas_h)
}

#[cfg(test)]
mod tests {
    use super::world_to_svg;
    use neco_view2d::View2d;

    #[test]
    fn matches_world_to_canvas() {
        let view = View2d {
            center_x: 50.0,
            center_y: -20.0,
            view_size: 8.0,
        };
        let (cw, ch) = (1280.0, 720.0);
        let expected = view.world_to_canvas(10.0, 5.0, cw, ch);
        let actual = world_to_svg(&view, 10.0, 5.0, cw, ch);
        assert_eq!(actual, expected);
    }
}
