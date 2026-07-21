use super::{RadialMenuLayout, RadialMenuState};
use crate::input::state::InputState;

/// Center circle radius.
pub(super) const CENTER_RADIUS: f64 = 30.0;
/// Inner radius of the tool ring.
const TOOL_INNER: f64 = 40.0;
/// Outer radius of the tool ring.
const TOOL_OUTER: f64 = 100.0;
/// Inner radius of the sub-ring band (flush with tool outer, no gap).
const SUB_INNER: f64 = 100.0;
/// Outer radius of the sub-ring band.
const SUB_OUTER: f64 = 150.0;
/// Inner radius of the color ring (flush with sub outer, no gap).
const COLOR_INNER: f64 = 150.0;
/// Outer radius of the color ring (slightly shrunk from the pre-size-ring
/// 184 to make room for the gauge band).
const COLOR_OUTER: f64 = 176.0;
/// Inner radius of the size ring (flush with color outer, no gap).
const SIZE_INNER: f64 = 176.0;
/// Outer radius of the size ring, the outermost band.
const SIZE_OUTER: f64 = 190.0;

impl InputState {
    /// Compute and cache the radial menu layout, clamping the center to keep it on screen.
    pub fn update_radial_menu_layout(&mut self, width: u32, height: u32) {
        if let RadialMenuState::Open {
            center_x, center_y, ..
        } = &self.radial_menu_state
        {
            // Track the outermost band so the whole menu (size ring included)
            // still fits the screen edges.
            let margin = SIZE_OUTER + 4.0;
            let cx = clamp_center_coordinate(*center_x, width as f64, margin);
            let cy = clamp_center_coordinate(*center_y, height as f64, margin);

            self.radial_menu_layout = Some(RadialMenuLayout {
                center_x: cx,
                center_y: cy,
                center_radius: CENTER_RADIUS,
                tool_inner: TOOL_INNER,
                tool_outer: TOOL_OUTER,
                sub_inner: SUB_INNER,
                sub_outer: SUB_OUTER,
                color_inner: COLOR_INNER,
                color_outer: COLOR_OUTER,
                size_inner: SIZE_INNER,
                size_outer: SIZE_OUTER,
            });
        }
    }

    /// Clear the cached layout when the menu is hidden.
    pub fn clear_radial_menu_layout(&mut self) {
        self.radial_menu_layout = None;
    }
}

fn clamp_center_coordinate(coord: f64, extent: f64, margin: f64) -> f64 {
    let min = margin;
    let max = extent - margin;
    if min <= max {
        coord.clamp(min, max)
    } else {
        extent / 2.0
    }
}
