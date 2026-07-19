use crate::draw::{Color, FontDescriptor};
use crate::input::{DrawingState, EraserMode, InputState, Tool};

use super::super::PrecisionEntryTarget;
use super::super::model::ToolbarSliderSpec;

impl InputState {
    /// Open the precise-entry popup for a pill numeral.
    pub(super) fn apply_toolbar_open_precision_entry(
        &mut self,
        target: PrecisionEntryTarget,
    ) -> bool {
        self.open_precision_entry(target);
        true
    }

    /// Commit a typed precise-entry value, clamped to the target's shared
    /// slider range, and close the popup if it is still open.
    pub(super) fn apply_toolbar_commit_precision_entry(
        &mut self,
        target: PrecisionEntryTarget,
        value: f64,
    ) -> bool {
        let _ = self.cancel_precision_entry();
        if !value.is_finite() {
            return false;
        }
        match target {
            PrecisionEntryTarget::Thickness => {
                let spec = ToolbarSliderSpec::THICKNESS;
                self.apply_toolbar_set_thickness(value.clamp(spec.min, spec.max))
            }
            PrecisionEntryTarget::FontSize => {
                let spec = ToolbarSliderSpec::FONT_SIZE;
                self.apply_toolbar_set_font_size(value.clamp(spec.min, spec.max))
            }
        }
    }

    pub(super) fn apply_toolbar_select_tool(&mut self, tool: Tool) -> bool {
        if matches!(self.state, DrawingState::TextInput { .. }) {
            self.cancel_text_input();
        }
        let mut changed = if tool == Tool::Highlight {
            let was_highlight_active = self.highlight_tool_active();
            let was_click_highlight_enabled = self.click_highlight_enabled();
            self.set_highlight_tool(true);
            let override_changed = self.set_tool_override(Some(tool));
            override_changed
                || was_highlight_active != self.highlight_tool_active()
                || was_click_highlight_enabled != self.click_highlight_enabled()
        } else {
            self.set_tool_override(Some(tool))
        };
        if self.toolbar_shapes_expanded {
            self.toolbar_shapes_expanded = false;
            changed = true;
        }
        if self.toolbar_top_overflow_open {
            self.toolbar_top_overflow_open = false;
            changed = true;
        }
        changed
    }

    pub(super) fn apply_toolbar_set_color(&mut self, color: Color) -> bool {
        self.apply_color_from_ui(color)
    }

    pub(super) fn apply_toolbar_set_color_hsv(&mut self, h: f64, s: f64, v: f64) -> bool {
        let changed = self.apply_color_from_ui(crate::draw::color::hsv_to_rgb(h, s, v));
        // Remember the picker position even when the color collapses to a
        // gray/black RGB value that cannot express hue or saturation.
        if self.toolbar_picker_hsv != Some((h, s, v)) {
            self.toolbar_picker_hsv = Some((h, s, v));
            self.needs_redraw = true;
            return true;
        }
        changed
    }

    pub(super) fn apply_toolbar_set_thickness(&mut self, value: f64) -> bool {
        self.set_thickness_for_active_tool(value)
    }

    pub(super) fn apply_toolbar_set_marker_opacity(&mut self, value: f64) -> bool {
        self.set_marker_opacity(value)
    }

    pub(super) fn apply_toolbar_set_eraser_mode(&mut self, mode: EraserMode) -> bool {
        self.set_eraser_mode(mode)
    }

    pub(super) fn apply_toolbar_set_font(&mut self, descriptor: FontDescriptor) -> bool {
        self.set_font_descriptor(descriptor)
    }

    pub(super) fn apply_toolbar_set_font_size(&mut self, size: f64) -> bool {
        self.set_font_size(size)
    }

    pub(super) fn apply_toolbar_toggle_fill(&mut self, enable: bool) -> bool {
        self.set_fill_enabled(enable)
    }

    pub(super) fn apply_toolbar_set_polygon_sides(&mut self, sides: u8) -> bool {
        self.set_polygon_sides(sides)
    }

    pub(super) fn apply_toolbar_nudge_polygon_sides(&mut self, delta: i8) -> bool {
        self.nudge_polygon_sides(delta)
    }

    pub(super) fn apply_toolbar_toggle_arrow_labels(&mut self, enable: bool) -> bool {
        self.set_arrow_label_enabled(enable)
    }

    pub(super) fn apply_toolbar_reset_arrow_label_counter(&mut self) -> bool {
        self.reset_arrow_label_counter()
    }

    pub(super) fn apply_toolbar_reset_step_marker_counter(&mut self) -> bool {
        self.reset_step_marker_counter()
    }

    pub(super) fn apply_toolbar_nudge_thickness(&mut self, delta: f64) -> bool {
        self.nudge_thickness_for_active_tool(delta)
    }

    pub(super) fn apply_toolbar_nudge_marker_opacity(&mut self, delta: f64) -> bool {
        self.set_marker_opacity(self.marker_opacity + delta)
    }

    pub(super) fn apply_toolbar_enter_text_mode(&mut self) -> bool {
        let _ = self.set_tool_override(None);
        self.toolbar_enter_text_mode();
        self.close_top_toolbar_menus();
        true
    }

    pub(super) fn apply_toolbar_enter_sticky_note_mode(&mut self) -> bool {
        let _ = self.set_tool_override(None);
        self.toolbar_enter_sticky_note_mode();
        self.close_top_toolbar_menus();
        true
    }

    pub(super) fn apply_toolbar_toggle_all_highlight(&mut self, enable: bool) -> bool {
        // set_highlight_tool already handles both highlight tool and click highlight
        let currently_active = self.highlight_tool_active() || self.click_highlight_enabled();
        let mut changed = false;
        if currently_active != enable {
            self.set_highlight_tool(enable);
            self.needs_redraw = true;
            changed = true;
        }
        self.close_top_toolbar_menus() || changed
    }

    pub(super) fn apply_toolbar_toggle_highlight_tool_ring(&mut self, enable: bool) -> bool {
        self.set_highlight_tool_ring_enabled(enable)
    }

    pub(super) fn apply_toolbar_apply_preset(&mut self, slot: usize) -> bool {
        self.apply_preset(slot)
    }

    pub(super) fn apply_toolbar_save_preset(&mut self, slot: usize) -> bool {
        self.save_preset(slot)
    }

    pub(super) fn apply_toolbar_clear_preset(&mut self, slot: usize) -> bool {
        self.clear_preset(slot)
    }

    pub(super) fn apply_toolbar_copy_hex_color(&mut self) -> bool {
        self.pending_copy_hex = true;
        true
    }

    pub(super) fn apply_toolbar_paste_hex_color(&mut self) -> bool {
        self.pending_paste_hex = true;
        true
    }

    pub(super) fn apply_toolbar_open_color_picker_popup(&mut self) -> bool {
        self.open_color_picker_popup();
        true
    }

    /// Open the color picker popup ready for typing: the hex field is
    /// focused and its content selected, so the first keystroke replaces it.
    pub(super) fn apply_toolbar_edit_hex_color(&mut self) -> bool {
        self.open_color_picker_popup();
        self.color_picker_popup_set_hex_editing(true);
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::input::state::test_support::make_test_input_state;
    use crate::ui::toolbar::ToolbarEvent;

    #[test]
    fn set_color_hsv_applies_color_and_remembers_picker_position() {
        let mut state = make_test_input_state();

        // A zero-saturation pick collapses to white in RGB; the remembered
        // HSV triple is what keeps the picker's hue from snapping to red.
        let changed = state.apply_toolbar_event(ToolbarEvent::SetColorHsv {
            h: 0.4,
            s: 0.0,
            v: 1.0,
        });

        assert!(changed);
        assert_eq!(state.toolbar_picker_hsv, Some((0.4, 0.0, 1.0)));
        let color = state.current_color;
        assert!((color.r - 1.0).abs() < 1e-9);
        assert!((color.g - 1.0).abs() < 1e-9);
        assert!((color.b - 1.0).abs() < 1e-9);
    }

    #[test]
    fn edit_hex_color_opens_popup_with_hex_focused() {
        let mut state = make_test_input_state();

        let changed = state.apply_toolbar_event(ToolbarEvent::EditHexColor);

        assert!(changed);
        assert!(state.is_color_picker_popup_open());
        assert!(state.color_picker_popup_is_hex_editing());
        assert!(state.color_picker_popup_hex_selected());
    }
}
