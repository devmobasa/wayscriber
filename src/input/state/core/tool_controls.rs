use super::base::{DrawingState, InputState, MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};
use crate::config::Action;
use crate::draw::{Color, FontDescriptor};
use crate::input::tool::{EraserMode, Tool};

impl InputState {
    /// Sets or clears an explicit tool override. Returns true if the tool changed.
    pub fn set_tool_override(&mut self, tool: Option<Tool>) -> bool {
        if self.tool_override == tool {
            return false;
        }

        self.tool_override = tool;

        // Ensure we are not mid-drawing with a stale tool
        if !matches!(
            self.state,
            DrawingState::Idle | DrawingState::TextInput { .. }
        ) {
            self.state = DrawingState::Idle;
        }

        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    /// Sets the marker opacity multiplier (0.05-0.9). Returns true if changed.
    pub fn set_marker_opacity(&mut self, opacity: f64) -> bool {
        let clamped = opacity.clamp(0.05, 0.9);
        if (clamped - self.marker_opacity).abs() < f64::EPSILON {
            return false;
        }
        self.marker_opacity = clamped;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    /// Returns the current explicit tool override (if any).
    pub fn tool_override(&self) -> Option<Tool> {
        self.tool_override
    }

    /// Sets thickness or eraser size depending on the active tool.
    pub fn set_thickness_for_active_tool(&mut self, value: f64) -> bool {
        match self.active_tool() {
            Tool::Eraser => self.set_eraser_size(value),
            _ => self.set_thickness(value),
        }
    }

    /// Nudges thickness or eraser size depending on the active tool.
    pub fn nudge_thickness_for_active_tool(&mut self, delta: f64) -> bool {
        match self.active_tool() {
            Tool::Eraser => self.set_eraser_size(self.eraser_size + delta),
            _ => self.set_thickness(self.current_thickness + delta),
        }
    }

    /// Updates the current drawing color to an arbitrary value. Returns true if changed.
    pub fn set_color(&mut self, color: Color) -> bool {
        if self.current_color == color {
            return false;
        }

        self.current_color = color;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.sync_highlight_color();
        true
    }

    /// Sets the absolute thickness (px), clamped to valid bounds. Returns true if changed.
    pub fn set_thickness(&mut self, thickness: f64) -> bool {
        let clamped = thickness.clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
        if (clamped - self.current_thickness).abs() < f64::EPSILON {
            return false;
        }

        self.current_thickness = clamped;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    /// Sets the absolute eraser size (px), clamped to valid bounds. Returns true if changed.
    pub fn set_eraser_size(&mut self, size: f64) -> bool {
        let clamped = size.clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
        if (clamped - self.eraser_size).abs() < f64::EPSILON {
            return false;
        }
        self.eraser_size = clamped;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    /// Sets the eraser behavior mode. Returns true if changed.
    pub fn set_eraser_mode(&mut self, mode: EraserMode) -> bool {
        if self.eraser_mode == mode {
            return false;
        }
        self.eraser_mode = mode;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    /// Toggles between brush and stroke eraser modes.
    pub fn toggle_eraser_mode(&mut self) -> bool {
        let next = match self.eraser_mode {
            EraserMode::Brush => EraserMode::Stroke,
            EraserMode::Stroke => EraserMode::Brush,
        };
        self.set_eraser_mode(next)
    }

    pub(crate) fn eraser_hit_radius(&self) -> f64 {
        (self.eraser_size / 2.0).max(1.0)
    }

    /// Sets the font descriptor used for text rendering. Returns true if changed.
    #[allow(dead_code)]
    pub fn set_font_descriptor(&mut self, descriptor: FontDescriptor) -> bool {
        if self.font_descriptor == descriptor {
            return false;
        }

        self.font_descriptor = descriptor;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    /// Sets the absolute font size (px), clamped to the same range as config validation.
    #[allow(dead_code)]
    pub fn set_font_size(&mut self, size: f64) -> bool {
        let clamped = size.clamp(8.0, 72.0);
        if (clamped - self.current_font_size).abs() < f64::EPSILON {
            return false;
        }

        self.current_font_size = clamped;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    /// Sets toolbar visibility flag (controls both top and side). Returns true if toggled.
    pub fn set_toolbar_visible(&mut self, visible: bool) -> bool {
        let any_change = self.toolbar_visible != visible
            || self.toolbar_top_visible != visible
            || self.toolbar_side_visible != visible;

        if !any_change {
            return false;
        }

        self.toolbar_visible = visible;
        self.toolbar_top_visible = visible;
        self.toolbar_side_visible = visible;
        self.needs_redraw = true;
        true
    }

    /// Returns whether any toolbar is marked visible.
    pub fn toolbar_visible(&self) -> bool {
        self.toolbar_visible || self.toolbar_top_visible || self.toolbar_side_visible
    }

    /// Returns whether the top toolbar is visible.
    pub fn toolbar_top_visible(&self) -> bool {
        self.toolbar_top_visible
    }

    /// Returns whether the side toolbar is visible.
    pub fn toolbar_side_visible(&self) -> bool {
        self.toolbar_side_visible
    }

    /// Enables or disables fill for fill-capable shapes.
    pub fn set_fill_enabled(&mut self, enabled: bool) -> bool {
        if self.fill_enabled == enabled {
            return false;
        }
        self.fill_enabled = enabled;
        self.needs_redraw = true;
        true
    }

    /// Initialize toolbar visibility from config (called at startup).
    #[allow(clippy::too_many_arguments)]
    pub fn init_toolbar_from_config(
        &mut self,
        top_pinned: bool,
        side_pinned: bool,
        use_icons: bool,
        show_more_colors: bool,
        show_actions_section: bool,
        show_delay_sliders: bool,
        show_marker_opacity_section: bool,
    ) {
        self.toolbar_top_pinned = top_pinned;
        self.toolbar_side_pinned = side_pinned;
        self.toolbar_top_visible = top_pinned;
        self.toolbar_side_visible = side_pinned;
        self.toolbar_visible = top_pinned || side_pinned;
        self.toolbar_use_icons = use_icons;
        self.show_more_colors = show_more_colors;
        self.show_actions_section = show_actions_section;
        self.show_delay_sliders = show_delay_sliders;
        self.show_marker_opacity_section = show_marker_opacity_section;
    }

    /// Wrapper for undo that preserves existing action plumbing.
    pub fn toolbar_undo(&mut self) {
        self.handle_action(Action::Undo);
    }

    /// Wrapper for redo that preserves existing action plumbing.
    pub fn toolbar_redo(&mut self) {
        self.handle_action(Action::Redo);
    }

    /// Wrapper for clear that preserves existing action plumbing.
    pub fn toolbar_clear(&mut self) {
        self.handle_action(Action::ClearCanvas);
    }

    /// Wrapper for entering text mode.
    pub fn toolbar_enter_text_mode(&mut self) {
        self.handle_action(Action::EnterTextMode);
    }
}
