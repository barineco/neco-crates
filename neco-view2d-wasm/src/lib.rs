use neco_view2d::View2d;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmView2d {
    inner: View2d,
}

impl Default for WasmView2d {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl WasmView2d {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: View2d::default(),
        }
    }

    pub fn pan(&mut self, dx: f64, dy: f64, canvas_height: f64) {
        self.inner.pan(dx, dy, canvas_height);
    }

    pub fn zoom_at(&mut self, delta: f64, cx: f64, cy: f64, cw: f64, ch: f64) {
        self.inner.zoom_at(delta, cx, cy, cw, ch);
    }

    pub fn canvas_to_world(&self, cx: f64, cy: f64, cw: f64, ch: f64) -> Vec<f64> {
        let (wx, wy) = self.inner.canvas_to_world(cx, cy, cw, ch);
        vec![wx, wy]
    }

    pub fn world_to_canvas(&self, wx: f64, wy: f64, cw: f64, ch: f64) -> Vec<f64> {
        let (cx, cy) = self.inner.world_to_canvas(wx, wy, cw, ch);
        vec![cx, cy]
    }

    pub fn get_state(&self) -> Vec<f64> {
        vec![
            self.inner.center_x,
            self.inner.center_y,
            self.inner.view_size,
        ]
    }

    pub fn set_state(&mut self, cx: f64, cy: f64, vs: f64) {
        self.inner.set(cx, cy, vs);
    }

    pub fn fit(&mut self, ww: f64, wh: f64, cw: f64, ch: f64) {
        self.inner.fit(ww, wh, cw, ch);
    }

    pub fn zoom_factor(&self, reference_view_size: f64) -> f64 {
        self.inner.zoom_factor(reference_view_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neco_view2d::View2d;

    const EPS: f64 = 1e-10;

    fn assert_vec2_close(actual: Vec<f64>, expected: (f64, f64)) {
        assert_eq!(actual.len(), 2);
        assert!((actual[0] - expected.0).abs() < EPS);
        assert!((actual[1] - expected.1).abs() < EPS);
    }

    fn assert_vec3_close(actual: Vec<f64>, expected: (f64, f64, f64)) {
        assert_eq!(actual.len(), 3);
        assert!((actual[0] - expected.0).abs() < EPS);
        assert!((actual[1] - expected.1).abs() < EPS);
        assert!((actual[2] - expected.2).abs() < EPS);
    }

    #[test]
    fn new_and_get_state_match_view2d() {
        let wrapper = WasmView2d::new();
        let core = View2d::default();

        assert_vec3_close(
            wrapper.get_state(),
            (core.center_x, core.center_y, core.view_size),
        );
    }

    #[test]
    fn set_state_pan_zoom_fit_and_zoom_factor_delegate_to_view2d() {
        let mut wrapper = WasmView2d::new();
        let mut core = View2d::default();

        wrapper.set_state(30.0, -12.5, 8.0);
        core.set(30.0, -12.5, 8.0);
        assert_vec3_close(
            wrapper.get_state(),
            (core.center_x, core.center_y, core.view_size),
        );

        wrapper.pan(5.0, -3.0, 720.0);
        core.pan(5.0, -3.0, 720.0);
        assert_vec3_close(
            wrapper.get_state(),
            (core.center_x, core.center_y, core.view_size),
        );

        wrapper.zoom_at(120.0, 320.0, 240.0, 1280.0, 720.0);
        core.zoom_at(120.0, 320.0, 240.0, 1280.0, 720.0);
        assert_vec3_close(
            wrapper.get_state(),
            (core.center_x, core.center_y, core.view_size),
        );

        wrapper.fit(1920.0, 1080.0, 800.0, 600.0);
        core.fit(1920.0, 1080.0, 800.0, 600.0);
        assert_vec3_close(
            wrapper.get_state(),
            (core.center_x, core.center_y, core.view_size),
        );

        let wrapper_zoom = wrapper.zoom_factor(64.0);
        let core_zoom = core.zoom_factor(64.0);
        assert!((wrapper_zoom - core_zoom).abs() < EPS);
    }

    #[test]
    fn coordinate_conversions_return_fixed_vec_shapes_and_match_view2d() {
        let wrapper = WasmView2d::new();
        let core = View2d::default();

        let wrapper_world = wrapper.canvas_to_world(200.0, 150.0, 800.0, 600.0);
        let core_world = core.canvas_to_world(200.0, 150.0, 800.0, 600.0);
        assert_vec2_close(wrapper_world, core_world);

        let wrapper_canvas = wrapper.world_to_canvas(0.25, -0.5, 800.0, 600.0);
        let core_canvas = core.world_to_canvas(0.25, -0.5, 800.0, 600.0);
        assert_vec2_close(wrapper_canvas, core_canvas);
    }
}
