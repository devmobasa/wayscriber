use log::debug;
use smithay_client_toolkit::seat::pointer::{AxisScroll, PointerEvent};

use crate::input::Tool;
use crate::input::state::COMMAND_PALETTE_MAX_VISIBLE;

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

        // Handle command palette scrolling
        if self.input_state.command_palette_open {
            if scroll_direction != 0 {
                let filtered_count = self.input_state.filtered_commands().len();
                let max_scroll = filtered_count.saturating_sub(COMMAND_PALETTE_MAX_VISIBLE);

                if scroll_direction > 0 {
                    // Scroll down
                    if self.input_state.command_palette_scroll < max_scroll {
                        self.input_state.command_palette_scroll += 1;
                        // Also move selection if it's above the visible area
                        if self.input_state.command_palette_selected
                            < self.input_state.command_palette_scroll
                        {
                            self.input_state.command_palette_selected =
                                self.input_state.command_palette_scroll;
                        }
                        self.input_state.needs_redraw = true;
                    }
                } else {
                    // Scroll up
                    if self.input_state.command_palette_scroll > 0 {
                        self.input_state.command_palette_scroll -= 1;
                        // Also move selection if it's below the visible area
                        if self.input_state.command_palette_selected
                            >= self.input_state.command_palette_scroll + COMMAND_PALETTE_MAX_VISIBLE
                        {
                            self.input_state.command_palette_selected =
                                self.input_state.command_palette_scroll
                                    + COMMAND_PALETTE_MAX_VISIBLE
                                    - 1;
                        }
                        self.input_state.needs_redraw = true;
                    }
                }
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
        if on_toolbar || self.pointer_over_toolbar() {
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
                let prev_font_size = self.input_state.current_font_size;
                self.input_state.adjust_font_size(-2.0);
                debug!(
                    "Font size decreased: {:.1}px",
                    self.input_state.current_font_size
                );
                if (self.input_state.current_font_size - prev_font_size).abs() > f64::EPSILON {
                    self.save_drawing_preferences();
                }
            }
            std::cmp::Ordering::Less if self.input_state.modifiers.shift => {
                let prev_font_size = self.input_state.current_font_size;
                self.input_state.adjust_font_size(2.0);
                debug!(
                    "Font size increased: {:.1}px",
                    self.input_state.current_font_size
                );
                if (self.input_state.current_font_size - prev_font_size).abs() > f64::EPSILON {
                    self.save_drawing_preferences();
                }
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
        #[cfg(tablet)]
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
                self.save_drawing_preferences();
            }
        }

        #[cfg(tablet)]
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
