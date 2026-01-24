use crate::config::ToolbarLayoutMode;
use crate::draw::{Color, FontDescriptor};
use crate::input::{DrawingState, EraserMode, InputState, Tool};

impl InputState {
    pub(super) fn apply_toolbar_select_tool(&mut self, tool: Tool) -> bool {
        if matches!(self.state, DrawingState::TextInput { .. }) {
            self.cancel_text_input();
        }
        let mut changed = self.set_tool_override(Some(tool));
        if self.toolbar_layout_mode == ToolbarLayoutMode::Simple && self.toolbar_shapes_expanded {
            self.toolbar_shapes_expanded = false;
            changed = true;
        }
        changed
    }

    pub(super) fn apply_toolbar_set_color(&mut self, color: Color) -> bool {
        self.set_color(color)
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
        true
    }

    pub(super) fn apply_toolbar_enter_sticky_note_mode(&mut self) -> bool {
        let _ = self.set_tool_override(None);
        self.toolbar_enter_sticky_note_mode();
        true
    }

    pub(super) fn apply_toolbar_toggle_all_highlight(&mut self, enable: bool) -> bool {
        // set_highlight_tool already handles both highlight tool and click highlight
        let currently_active = self.highlight_tool_active() || self.click_highlight_enabled();
        if currently_active != enable {
            self.set_highlight_tool(enable);
            self.needs_redraw = true;
            true
        } else {
            false
        }
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
}
