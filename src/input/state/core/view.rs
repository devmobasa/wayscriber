use crate::util::Rect;

#[derive(Debug, Clone)]
pub(in crate::input::state) struct ViewState {
    zoom_active: bool,
    zoom_locked: bool,
    zoom_scale: f64,
    zoom_view_offset: (f64, f64),
    frozen_active: bool,
    screen_width: u32,
    screen_height: u32,
    active_output_label: Option<String>,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            zoom_active: false,
            zoom_locked: false,
            zoom_scale: 1.0,
            zoom_view_offset: (0.0, 0.0),
            frozen_active: false,
            screen_width: 0,
            screen_height: 0,
            active_output_label: None,
        }
    }
}

impl ViewState {
    pub(in crate::input::state) fn set_zoom_status(
        &mut self,
        active: bool,
        locked: bool,
        scale: f64,
        view_offset: (f64, f64),
    ) -> bool {
        let changed = self.zoom_active != active
            || self.zoom_locked != locked
            || (self.zoom_scale - scale).abs() > f64::EPSILON
            || (self.zoom_view_offset.0 - view_offset.0).abs() > f64::EPSILON
            || (self.zoom_view_offset.1 - view_offset.1).abs() > f64::EPSILON;
        if changed {
            self.zoom_active = active;
            self.zoom_locked = locked;
            self.zoom_scale = scale;
            self.zoom_view_offset = view_offset;
        }
        changed
    }

    pub(in crate::input::state) fn zoom_active(&self) -> bool {
        self.zoom_active
    }

    pub(in crate::input::state) fn zoom_locked(&self) -> bool {
        self.zoom_locked
    }

    pub(in crate::input::state) fn zoom_scale(&self) -> f64 {
        self.zoom_scale
    }

    pub(in crate::input::state) fn set_frozen_active(&mut self, active: bool) -> bool {
        let changed = self.frozen_active != active;
        self.frozen_active = active;
        changed
    }

    pub(in crate::input::state) fn frozen_active(&self) -> bool {
        self.frozen_active
    }

    pub(in crate::input::state) fn set_screen_dimensions(&mut self, width: u32, height: u32) {
        self.screen_width = width;
        self.screen_height = height;
    }

    pub(in crate::input::state) fn screen_size(&self) -> (u32, u32) {
        (self.screen_width, self.screen_height)
    }

    pub(in crate::input::state) fn screen_width(&self) -> u32 {
        self.screen_width
    }

    pub(in crate::input::state) fn screen_height(&self) -> u32 {
        self.screen_height
    }

    pub(in crate::input::state) fn set_active_output_label(
        &mut self,
        label: Option<String>,
    ) -> bool {
        if self.active_output_label == label {
            return false;
        }
        self.active_output_label = label;
        true
    }

    pub(in crate::input::state) fn active_output_label(&self) -> Option<&str> {
        self.active_output_label.as_deref()
    }

    pub(in crate::input::state) fn canvas_scale(&self) -> f64 {
        if self.zoom_active {
            self.zoom_scale.max(f64::MIN_POSITIVE)
        } else {
            1.0
        }
    }

    pub(in crate::input::state) fn canvas_origin(&self, board_offset: (f64, f64)) -> (f64, f64) {
        if self.zoom_active {
            (
                board_offset.0 + self.zoom_view_offset.0,
                board_offset.1 + self.zoom_view_offset.1,
            )
        } else {
            board_offset
        }
    }

    pub(in crate::input::state) fn canvas_coords_for_screen(
        &self,
        board_offset: (f64, f64),
        screen_x: i32,
        screen_y: i32,
    ) -> (i32, i32) {
        let scale = self.canvas_scale();
        let (origin_x, origin_y) = self.canvas_origin(board_offset);
        (
            (origin_x + screen_x as f64 / scale).round() as i32,
            (origin_y + screen_y as f64 / scale).round() as i32,
        )
    }

    pub(in crate::input::state) fn screen_coords_for_canvas(
        &self,
        board_offset: (f64, f64),
        canvas_x: i32,
        canvas_y: i32,
    ) -> (i32, i32) {
        let scale = self.canvas_scale();
        let (origin_x, origin_y) = self.canvas_origin(board_offset);
        (
            ((canvas_x as f64 - origin_x) * scale).round() as i32,
            ((canvas_y as f64 - origin_y) * scale).round() as i32,
        )
    }

    pub(in crate::input::state) fn screen_rect_for_canvas(
        &self,
        board_offset: (f64, f64),
        rect: Rect,
    ) -> Option<Rect> {
        let scale = self.canvas_scale();
        let (origin_x, origin_y) = self.canvas_origin(board_offset);
        let min_x = ((rect.x as f64 - origin_x) * scale).floor() as i32;
        let min_y = ((rect.y as f64 - origin_y) * scale).floor() as i32;
        let max_x = (((rect.x + rect.width) as f64 - origin_x) * scale).ceil() as i32;
        let max_y = (((rect.y + rect.height) as f64 - origin_y) * scale).ceil() as i32;
        Rect::from_min_max(min_x, min_y, max_x, max_y)
    }

    /// Returns the visible canvas area, or a 1x1 fallback at its minimum corner
    /// when the transformed extent cannot be represented by [`Rect`].
    pub(in crate::input::state) fn visible_canvas_rect(&self, board_offset: (f64, f64)) -> Rect {
        let (x1, y1) = self.canvas_coords_for_screen(board_offset, 0, 0);
        let (x2, y2) = self.canvas_coords_for_screen(
            board_offset,
            self.screen_width.min(i32::MAX as u32) as i32,
            self.screen_height.min(i32::MAX as u32) as i32,
        );
        let min_x = x1.min(x2);
        let min_y = y1.min(y2);
        let fallback = Rect {
            x: min_x,
            y: min_y,
            width: 1,
            height: 1,
        };
        // Widen before adding the non-empty minimum. Persisted view offsets
        // can saturate both transformed corners at i32::MAX, where `min + 1`
        // would overflow in debug builds.
        let max_x = i64::from(x1.max(x2)).max(i64::from(min_x) + 1);
        let max_y = i64::from(y1.max(y2)).max(i64::from(min_y) + 1);
        let (Ok(max_x), Ok(max_y)) = (i32::try_from(max_x), i32::try_from(max_y)) else {
            return fallback;
        };
        Rect::from_min_max(min_x, min_y, max_x, max_y).unwrap_or(fallback)
    }

    pub(in crate::input::state) fn visible_canvas_center(
        &self,
        board_offset: (f64, f64),
    ) -> (i32, i32) {
        let rect = self.visible_canvas_rect(board_offset);
        (
            rect.x.saturating_add(rect.width / 2),
            rect.y.saturating_add(rect.height / 2),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::Rect;

    #[test]
    fn zoom_change_detection_uses_the_existing_epsilon_boundary() {
        let mut view = ViewState::default();

        assert!(!view.set_zoom_status(false, false, 1.0, (f64::EPSILON, 0.0)));
        assert!(view.set_zoom_status(false, false, 1.0, (f64::EPSILON * 2.0, 0.0)));
    }

    #[test]
    fn saturated_canvas_origin_returns_the_documented_one_pixel_visible_fallback() {
        let mut view = ViewState::default();
        view.set_screen_dimensions(1, 1);
        assert!(
            view.set_zoom_status(true, false, 1.0, (f64::from(i32::MAX), f64::from(i32::MAX)),)
        );

        assert_eq!(
            view.visible_canvas_rect((0.0, 0.0)),
            Rect {
                x: i32::MAX,
                y: i32::MAX,
                width: 1,
                height: 1,
            }
        );
    }

    #[test]
    fn screen_and_canvas_coordinates_round_trip_at_scale_one_and_two_point_five() {
        let mut view = ViewState::default();

        let canvas = view.canvas_coords_for_screen((100.0, -50.0), 30, 40);
        assert_eq!(canvas, (130, -10));
        assert_eq!(
            view.screen_coords_for_canvas((100.0, -50.0), canvas.0, canvas.1),
            (30, 40)
        );

        assert!(view.set_zoom_status(true, false, 2.5, (12.0, -8.0)));
        let canvas = view.canvas_coords_for_screen((100.0, -50.0), 25, 50);
        assert_eq!(canvas, (122, -38));
        assert_eq!(
            view.screen_coords_for_canvas((100.0, -50.0), canvas.0, canvas.1),
            (25, 50)
        );
    }

    #[test]
    fn setting_the_same_output_label_reports_no_change() {
        let mut view = ViewState::default();

        assert!(view.set_active_output_label(Some("DP-1".to_string())));
        assert!(!view.set_active_output_label(Some("DP-1".to_string())));
        assert_eq!(view.active_output_label(), Some("DP-1"));
    }
}
