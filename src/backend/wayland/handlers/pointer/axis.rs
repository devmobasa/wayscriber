use log::debug;
use smithay_client_toolkit::seat::pointer::{AxisScroll, PointerEvent};

use crate::input::Tool;
use crate::input::state::InputState;

use super::*;

impl WaylandState {
    pub(super) fn handle_pointer_axis(
        &mut self,
        event: &PointerEvent,
        on_toolbar: bool,
        vertical: AxisScroll,
    ) {
        let scroll_direction = if vertical.discrete != 0 {
            vertical.discrete
        } else if vertical.absolute.abs() > 0.1 {
            if vertical.absolute > 0.0 { 1 } else { -1 }
        } else {
            0
        };
        // Handle radial menu scroll-to-thickness
        if self.input_state.is_radial_menu_open() {
            if scroll_direction != 0 {
                let delta = if scroll_direction > 0 { -1.0 } else { 1.0 };
                self.adjust_active_tool_thickness(delta, true);
            }
            return;
        }

        // Handle command palette scrolling (display-row space; selection is
        // kept inside the window, skipping group headers).
        if self.input_state.command_palette_open {
            if scroll_direction != 0 {
                self.input_state
                    .command_palette_wheel_scroll(scroll_direction);
            }
            return;
        }

        if self.input_state.show_help {
            if scroll_direction != 0 {
                let delta = if scroll_direction > 0 { 1.0 } else { -1.0 };
                let scroll_step = 48.0;
                let max_scroll = self.input_state.help_overlay_scroll_max;
                let mut next = self.input_state.help_overlay_scroll + delta * scroll_step;
                if max_scroll > 0.0 {
                    next = next.clamp(0.0, max_scroll);
                } else {
                    next = next.max(0.0);
                }
                if (next - self.input_state.help_overlay_scroll).abs() > f64::EPSILON {
                    self.input_state.help_overlay_scroll = next;
                    self.input_state.dirty_tracker.mark_full();
                    self.input_state.needs_redraw = true;
                }
            }
            return;
        }
        if try_handle_board_picker_page_panel_axis(
            &mut self.input_state,
            event.position,
            scroll_direction,
        ) {
            return;
        }
        if on_toolbar || self.pointer_over_toolbar() {
            if scroll_direction != 0 {
                if self.wheel_over_side_toolbar(&event.surface, event.position) {
                    self.scroll_side_pane_by_wheel(scroll_direction);
                } else if self.wheel_over_top_toolbar(&event.surface, event.position) {
                    // With a Canvas/Session/Settings popover open, the wheel scrolls
                    // its capped viewport; otherwise a top-strip wheel stays a
                    // no-op (it never falls through to thickness/zoom).
                    self.scroll_top_popover_by_wheel(scroll_direction);
                }
            }
            return;
        }
        if self.input_state.modifiers.ctrl && self.input_state.modifiers.alt {
            if scroll_direction != 0 {
                let zoom_in = scroll_direction < 0;
                self.handle_zoom_scroll(zoom_in, event.position.0, event.position.1);
            }
            return;
        }

        match scroll_direction.cmp(&0) {
            std::cmp::Ordering::Greater if self.input_state.modifiers.shift => {
                self.input_state.adjust_font_size(-2.0);
                debug!(
                    "Font size decreased: {:.1}px",
                    self.input_state.current_font_size
                );
            }
            std::cmp::Ordering::Less if self.input_state.modifiers.shift => {
                self.input_state.adjust_font_size(2.0);
                debug!(
                    "Font size increased: {:.1}px",
                    self.input_state.current_font_size
                );
            }
            std::cmp::Ordering::Greater | std::cmp::Ordering::Less => {
                let delta = if scroll_direction > 0 { -1.0 } else { 1.0 };
                self.adjust_active_tool_thickness(delta, false);
            }
            std::cmp::Ordering::Equal => {}
        }
    }

    fn adjust_active_tool_thickness(&mut self, delta: f64, radial_menu_path: bool) {
        let eraser_active = self.input_state.active_tool() == Tool::Eraser;
        #[cfg(feature = "tablet-input")]
        let prev_thickness = self.input_state.current_thickness;

        let changed = if radial_menu_path {
            self.input_state.radial_menu_adjust_thickness(delta)
        } else if self.input_state.nudge_thickness_for_active_tool(delta) {
            self.input_state.needs_redraw = true;
            true
        } else {
            false
        };

        if changed {
            if eraser_active {
                debug!(
                    "Eraser size adjusted: {:.0}px",
                    self.input_state.eraser_size
                );
            } else {
                debug!(
                    "Thickness adjusted: {:.0}px",
                    self.input_state.current_thickness
                );
            }
        }

        #[cfg(feature = "tablet-input")]
        if !eraser_active
            && (self.input_state.current_thickness - prev_thickness).abs() > f64::EPSILON
        {
            self.stylus_base_thickness = Some(self.input_state.current_thickness);
            if self.stylus_tip_down {
                self.stylus_pressure_thickness = Some(self.input_state.current_thickness);
                self.record_stylus_peak(self.input_state.current_thickness);
            } else {
                self.stylus_pressure_thickness = None;
                self.stylus_peak_thickness = None;
            }
        }
    }
}

fn try_handle_board_picker_page_panel_axis(
    input_state: &mut InputState,
    position: (f64, f64),
    scroll_direction: i32,
) -> bool {
    if !input_state.is_board_picker_open() || scroll_direction == 0 {
        return false;
    }
    let x = position.0.round() as i32;
    let y = position.1.round() as i32;
    if !input_state.board_picker_page_panel_content_at(x, y) {
        return false;
    }
    let delta = if scroll_direction > 0 { 1 } else { -1 };
    let _ = input_state.board_picker_scroll_page_panel_rows(delta);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::Frame;
    use crate::input::state::{BoardPickerFocus, test_support::make_test_input_state};

    fn update_picker_layout(input_state: &mut InputState) {
        let surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, 1280, 720).expect("image surface");
        let ctx = cairo::Context::new(&surface).expect("cairo context");
        input_state.update_board_picker_layout(&ctx, 1280, 720);
    }

    fn set_board_page_count(input_state: &mut InputState, board_index: usize, page_count: usize) {
        let pages = input_state.boards.board_states_mut()[board_index]
            .pages
            .pages_mut();
        pages.clear();
        pages.extend((0..page_count.max(1)).map(|_| Frame::new()));
    }

    #[test]
    fn board_picker_page_panel_axis_consumes_before_thickness_changes() {
        let mut input_state = make_test_input_state();
        input_state.open_board_picker();
        let board_index = input_state
            .board_picker_page_panel_board_index()
            .expect("page panel board index");
        set_board_page_count(&mut input_state, board_index, 80);
        update_picker_layout(&mut input_state);

        let layout = *input_state.board_picker_layout().expect("layout");
        let position = (layout.page_viewport_x + 1.0, layout.page_viewport_y + 1.0);
        let thickness = input_state.current_thickness;
        input_state.board_picker_set_focus(BoardPickerFocus::PagePanel);

        assert!(try_handle_board_picker_page_panel_axis(
            &mut input_state,
            position,
            1,
        ));
        assert_eq!(input_state.current_thickness, thickness);
        update_picker_layout(&mut input_state);

        let layout = *input_state.board_picker_layout().expect("layout");
        assert_eq!(layout.page_scroll_row, 1);
    }
}
