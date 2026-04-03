/// 2D view transform (pan & zoom).
///
/// `view_size` is the world-space height mapped to the canvas vertical extent.
/// Smaller values mean more zoom.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct View2d {
    pub center_x: f64,
    pub center_y: f64,
    /// World-space height visible on canvas. Always positive.
    pub view_size: f64,
}

impl Default for View2d {
    fn default() -> Self {
        Self {
            center_x: 0.0,
            center_y: 0.0,
            view_size: 1.0,
        }
    }
}

impl View2d {
    /// Set view parameters. `view_size` is clamped to positive.
    pub fn set(&mut self, center_x: f64, center_y: f64, view_size: f64) {
        self.center_x = center_x;
        self.center_y = center_y;
        self.view_size = if view_size < f64::EPSILON {
            f64::EPSILON
        } else {
            view_size
        };
    }

    /// Pan by pixel delta. Speed scales with `view_size / canvas_height`.
    pub fn pan(&mut self, dx: f64, dy: f64, canvas_height: f64) {
        let speed = self.view_size / canvas_height;
        self.center_x += dx * speed;
        self.center_y += dy * speed;
    }

    /// Zoom centered on a canvas position. `delta > 0` zooms in.
    pub fn zoom_at(
        &mut self,
        delta: f64,
        canvas_x: f64,
        canvas_y: f64,
        canvas_width: f64,
        canvas_height: f64,
    ) {
        let factor = 1.0 + delta * 0.001;

        // Record cursor world position before zoom
        let (wx, wy) = self.canvas_to_world(canvas_x, canvas_y, canvas_width, canvas_height);

        let new_view_size = self.view_size / factor;
        self.view_size = if new_view_size < f64::EPSILON {
            f64::EPSILON
        } else {
            new_view_size
        };

        // Adjust center so cursor world position stays invariant
        let (wx2, wy2) = self.canvas_to_world(canvas_x, canvas_y, canvas_width, canvas_height);
        self.center_x += wx - wx2;
        self.center_y += wy - wy2;
    }

    /// Convert canvas coordinates to world coordinates.
    pub fn canvas_to_world(
        &self,
        cx: f64,
        cy: f64,
        canvas_width: f64,
        canvas_height: f64,
    ) -> (f64, f64) {
        let aspect = canvas_width / canvas_height;
        let world_x = self.center_x + (cx / canvas_width - 0.5) * self.view_size * aspect;
        let world_y = self.center_y + (cy / canvas_height - 0.5) * self.view_size;
        (world_x, world_y)
    }

    /// Convert world coordinates to canvas coordinates.
    pub fn world_to_canvas(
        &self,
        wx: f64,
        wy: f64,
        canvas_width: f64,
        canvas_height: f64,
    ) -> (f64, f64) {
        let aspect = canvas_width / canvas_height;
        let canvas_x = ((wx - self.center_x) / (self.view_size * aspect) + 0.5) * canvas_width;
        let canvas_y = ((wy - self.center_y) / self.view_size + 0.5) * canvas_height;
        (canvas_x, canvas_y)
    }

    /// Fit the entire world region into the canvas.
    pub fn fit(
        &mut self,
        world_width: f64,
        world_height: f64,
        canvas_width: f64,
        canvas_height: f64,
    ) {
        self.center_x = world_width / 2.0;
        self.center_y = world_height / 2.0;

        let fit_by_height = world_height;
        let fit_by_width = world_width * canvas_height / canvas_width;
        let base = if fit_by_height > fit_by_width {
            fit_by_height
        } else {
            fit_by_width
        };

        // Add slight margin
        self.view_size = base * 1.05;
    }

    /// Current zoom factor relative to a reference `view_size`.
    pub fn zoom_factor(&self, reference_view_size: f64) -> f64 {
        reference_view_size / self.view_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-10;

    /// canvas_to_world and world_to_canvas roundtrip
    #[test]
    fn coordinate_roundtrip() {
        let cases = [
            // (center_x, center_y, view_size, canvas_w, canvas_h, cx, cy)
            (0.0, 0.0, 1.0, 800.0, 600.0, 400.0, 300.0),
            (100.0, 200.0, 50.0, 1920.0, 1080.0, 960.0, 540.0),
            (-10.0, 5.0, 0.5, 640.0, 480.0, 0.0, 0.0),
            (0.0, 0.0, 10.0, 800.0, 600.0, 800.0, 600.0),
            (50.0, 50.0, 100.0, 1024.0, 768.0, 123.0, 456.0),
        ];

        for (center_x, center_y, view_size, cw, ch, cx, cy) in cases {
            let v = View2d {
                center_x,
                center_y,
                view_size,
            };
            let (wx, wy) = v.canvas_to_world(cx, cy, cw, ch);
            let (cx2, cy2) = v.world_to_canvas(wx, wy, cw, ch);
            assert!(
                (cx - cx2).abs() < EPS && (cy - cy2).abs() < EPS,
                "roundtrip failed: ({cx}, {cy}) -> ({wx}, {wy}) -> ({cx2}, {cy2})"
            );
        }
    }

    /// Reverse roundtrip: world_to_canvas then canvas_to_world
    #[test]
    fn coordinate_roundtrip_reverse() {
        let v = View2d {
            center_x: 30.0,
            center_y: -20.0,
            view_size: 8.0,
        };
        let (cw, ch) = (1280.0, 720.0);
        let (wx, wy) = (35.0, -18.0);
        let (cx, cy) = v.world_to_canvas(wx, wy, cw, ch);
        let (wx2, wy2) = v.canvas_to_world(cx, cy, cw, ch);
        assert!((wx - wx2).abs() < EPS && (wy - wy2).abs() < EPS);
    }

    /// zoom_at preserves cursor world position
    #[test]
    fn zoom_at_cursor_invariance() {
        let deltas = [100.0, -100.0, 500.0, -500.0, 1.0];
        let (cw, ch) = (800.0, 600.0);

        for delta in deltas {
            let mut v = View2d {
                center_x: 50.0,
                center_y: 30.0,
                view_size: 10.0,
            };
            let (cx, cy) = (200.0, 150.0);
            let (wx_before, wy_before) = v.canvas_to_world(cx, cy, cw, ch);
            v.zoom_at(delta, cx, cy, cw, ch);
            let (wx_after, wy_after) = v.canvas_to_world(cx, cy, cw, ch);
            assert!(
                (wx_before - wx_after).abs() < 1e-6 && (wy_before - wy_after).abs() < 1e-6,
                "zoom_at cursor invariance violated: delta={delta}, before=({wx_before},{wy_before}), after=({wx_after},{wy_after})"
            );
        }
    }

    /// Pan distance scales proportionally with view_size
    #[test]
    fn pan_proportional_to_view_size() {
        let ch = 600.0;
        let dx = 10.0;
        let dy = 20.0;

        let mut v1 = View2d {
            center_x: 0.0,
            center_y: 0.0,
            view_size: 5.0,
        };
        v1.pan(dx, dy, ch);
        let move1_x = v1.center_x;
        let move1_y = v1.center_y;

        let mut v2 = View2d {
            center_x: 0.0,
            center_y: 0.0,
            view_size: 10.0,
        };
        v2.pan(dx, dy, ch);
        let move2_x = v2.center_x;
        let move2_y = v2.center_y;

        // 2x view_size should yield 2x displacement
        assert!(
            (move2_x - move1_x * 2.0).abs() < EPS,
            "pan X proportionality violated: {move1_x} * 2 != {move2_x}"
        );
        assert!(
            (move2_y - move1_y * 2.0).abs() < EPS,
            "pan Y proportionality violated: {move1_y} * 2 != {move2_y}"
        );
    }

    /// fit ensures all four world corners are within canvas bounds
    #[test]
    fn fit_contains_world_region() {
        let cases = [
            // (world_w, world_h, canvas_w, canvas_h)
            (100.0, 80.0, 800.0, 600.0),
            (1920.0, 1080.0, 640.0, 480.0),
            (50.0, 200.0, 1024.0, 768.0), // tall world
            (300.0, 10.0, 800.0, 600.0),  // wide world
        ];

        for (ww, wh, cw, ch) in cases {
            let mut v = View2d::default();
            v.fit(ww, wh, cw, ch);

            // Verify all 4 corners
            let corners = [(0.0, 0.0), (ww, 0.0), (0.0, wh), (ww, wh)];
            for (wx, wy) in corners {
                let (cx, cy) = v.world_to_canvas(wx, wy, cw, ch);
                assert!(
                    cx >= -EPS && cx <= cw + EPS && cy >= -EPS && cy <= ch + EPS,
                    "fit out of bounds: world=({wx},{wy}) -> canvas=({cx},{cy}), canvas_size=({cw},{ch})"
                );
            }
        }
    }

    /// zoom_factor calculation
    #[test]
    fn zoom_factor_calculation() {
        let v = View2d {
            center_x: 0.0,
            center_y: 0.0,
            view_size: 5.0,
        };
        assert!((v.zoom_factor(10.0) - 2.0).abs() < EPS);
        assert!((v.zoom_factor(5.0) - 1.0).abs() < EPS);
        assert!((v.zoom_factor(2.5) - 0.5).abs() < EPS);
    }

    /// set clamps non-positive view_size to epsilon
    #[test]
    fn set_clamps_view_size() {
        let mut v = View2d::default();

        v.set(1.0, 2.0, 0.0);
        assert_eq!(v.view_size, f64::EPSILON);

        v.set(1.0, 2.0, -100.0);
        assert_eq!(v.view_size, f64::EPSILON);

        v.set(1.0, 2.0, f64::EPSILON * 0.5);
        assert_eq!(v.view_size, f64::EPSILON);

        // Positive value passes through
        v.set(1.0, 2.0, 42.0);
        assert_eq!(v.view_size, 42.0);
    }

    /// Default values
    #[test]
    fn default_values() {
        let v = View2d::default();
        assert_eq!(v.center_x, 0.0);
        assert_eq!(v.center_y, 0.0);
        assert_eq!(v.view_size, 1.0);
    }
}
