use super::super::base::{DrawingState, InputState, MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};
use crate::draw::{ArrowStyle, BlurStyle, Color, FontDescriptor};
use crate::draw::{TextMeasurer, with_legacy_measurer};
use crate::input::state::{Toast, ToastPriority};
use crate::input::{
    DragBinding, MouseButton,
    modifiers::DragToolBindings,
    tool::{EraserMode, Tool},
};

impl InputState {
    /// Returns the stored drawing color for a tool.
    pub fn color_for_tool(&self, tool: Tool) -> Color {
        self.style.color_for_tool(tool)
    }

    /// Returns the drawing color for the currently active tool.
    pub fn color_for_active_tool(&self) -> Color {
        self.color_for_tool(self.active_tool())
    }

    /// Returns the stored size for a tool, using eraser size for the eraser.
    pub fn thickness_for_tool(&self, tool: Tool) -> f64 {
        self.style.thickness_for_tool(tool)
    }

    /// Returns the stored size for the currently active tool.
    pub fn thickness_for_active_tool(&self) -> f64 {
        self.thickness_for_tool(self.active_tool())
    }

    /// Updates the compatibility current_* fields from the active tool settings.
    pub(crate) fn sync_current_settings_from_active_tool(&mut self) {
        let tool = self.active_tool();
        self.style.sync_current_settings(tool);
    }

    pub(crate) fn sync_current_settings_for_tool(&mut self, tool: Tool) {
        self.style.sync_current_settings(tool);
    }

    pub(crate) fn set_pen_color_from_board(&mut self, color: Color) {
        let active_tool = self.active_tool();
        self.style.set_pen_color(color, active_tool);
        self.sync_highlight_color();
    }

    pub(crate) fn preview_color_for_tool(&mut self, tool: Tool, color: Color) -> bool {
        let active_tool = self.active_tool();
        if !self.style.preview_color(tool, active_tool, color) {
            return false;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.sync_highlight_color();
        true
    }

    /// Updates the active drawing thickness from tablet pressure without
    /// treating every pressure sample as a persisted user preference edit.
    #[cfg_attr(not(feature = "tablet-input"), allow(dead_code))]
    pub(crate) fn set_pressure_thickness_for_active_tool(&mut self, thickness: f64) -> f64 {
        with_legacy_measurer(|measurer| {
            self.set_pressure_thickness_for_active_tool_with(measurer, thickness)
        })
    }

    pub(crate) fn set_pressure_thickness_for_active_tool_with(
        &mut self,
        measurer: &TextMeasurer,
        thickness: f64,
    ) -> f64 {
        let tool = self.active_tool();
        let clamped = self.style.set_pressure_thickness(tool, thickness);
        let initial_pressure_sample_changes =
            self.active_initial_pressure_sample_changes(clamped as f32);
        if initial_pressure_sample_changes {
            self.mark_current_provisional_dirty_full_with(measurer);
        }
        self.update_initial_pressure_sample(clamped);
        if initial_pressure_sample_changes {
            self.mark_current_provisional_dirty_full_with(measurer);
        }
        self.needs_redraw = true;
        clamped
    }

    #[cfg(feature = "tablet-input")]
    pub(crate) fn replace_active_drawing_pressure_samples(&mut self, thickness: f64) -> bool {
        with_legacy_measurer(|measurer| {
            self.replace_active_drawing_pressure_samples_with(measurer, thickness)
        })
    }

    #[cfg(feature = "tablet-input")]
    pub(crate) fn replace_active_drawing_pressure_samples_with(
        &mut self,
        measurer: &TextMeasurer,
        thickness: f64,
    ) -> bool {
        let clamped = thickness.clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS) as f32;
        let DrawingState::Drawing {
            point_thicknesses, ..
        } = &self.state
        else {
            return false;
        };
        if point_thicknesses.is_empty() {
            return false;
        }

        self.mark_current_provisional_dirty_full_with(measurer);

        let DrawingState::Drawing {
            point_thicknesses, ..
        } = &mut self.state
        else {
            return false;
        };
        point_thicknesses.fill(clamped);

        self.mark_current_provisional_dirty_full_with(measurer);
        self.needs_redraw = true;
        true
    }

    fn update_initial_pressure_sample(&mut self, thickness: f64) {
        let DrawingState::Drawing {
            points,
            point_thicknesses,
            ..
        } = &mut self.state
        else {
            return;
        };
        if points.len() == 1 && point_thicknesses.len() == 1 {
            point_thicknesses[0] = thickness as f32;
        }
    }

    fn active_initial_pressure_sample_changes(&self, thickness: f32) -> bool {
        let DrawingState::Drawing {
            points,
            point_thicknesses,
            ..
        } = &self.state
        else {
            return false;
        };

        points.len() == 1
            && point_thicknesses.len() == 1
            && (point_thicknesses[0] - thickness).abs() > f32::EPSILON
    }

    /// Sets or clears an explicit tool override. Returns true if the tool changed.
    pub fn set_tool_override(&mut self, tool: Option<Tool>) -> bool {
        with_legacy_measurer(|measurer| self.set_tool_override_with(measurer, tool))
    }

    pub fn set_tool_override_with(&mut self, measurer: &TextMeasurer, tool: Option<Tool>) -> bool {
        if self.presenter_mode_active()
            && matches!(
                self.presenter_mode_config().tool_behavior,
                crate::config::PresenterToolBehavior::ForceHighlightLocked
            )
            && tool != Some(Tool::Highlight)
        {
            return false;
        }
        if !self.style.set_tool_override(tool) {
            return false;
        }
        self.preset_slots.clear_active();

        if tool == Some(Tool::Blur)
            && self.style.blur_style.needs_backdrop()
            && !self.frozen_active()
            && !self.pending_frozen_toggle()
        {
            self.request_frozen_toggle();
            self.push_toast(
                ToastPriority::Info,
                "toolbar",
                Toast::info("Capturing background for blur..."),
            );
        }

        // Ensure we are not mid-drawing with a stale tool
        if !matches!(
            self.state,
            DrawingState::Idle | DrawingState::TextInput { .. }
        ) {
            self.cancel_active_interaction_with(measurer);
        }

        self.sync_current_settings_from_active_tool();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }

    /// Sets the marker opacity multiplier (0.05-0.9). Returns true if changed.
    pub fn set_marker_opacity(&mut self, opacity: f64) -> bool {
        if !self.style.set_marker_opacity(opacity) {
            return false;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }

    /// Steps the release-time smoothing level. Returns true if it changed.
    ///
    /// Clamped at both ends rather than wrapped: 0 and the maximum are both
    /// settings someone deliberately sits on, and wrapping from one to the
    /// other on a stray keypress would change every later stroke.
    pub fn nudge_pen_smoothing(&mut self, delta: i32) -> bool {
        if !self.style.nudge_pen_smoothing(delta) {
            return false;
        }
        self.mark_session_dirty();
        true
    }

    /// Sets the level directly, for config load and session restore.
    pub fn set_pen_smoothing(&mut self, level: u8) -> bool {
        if !self.style.set_pen_smoothing(level) {
            return false;
        }
        self.mark_session_dirty();
        true
    }

    /// Sets the magnification stored on newly drawn spotlights.
    ///
    /// Deliberately requests no warning feedback: this changes what the *next*
    /// Spotlight will use, and no Spotlight has been created or edited yet.
    /// The style control's inline unavailable state already reports the
    /// default against the current surface, and toasting here would fire
    /// repeatedly while the user drags the slider.
    pub fn set_spotlight_magnification(&mut self, magnification: f64) -> bool {
        if !self.style.set_spotlight_magnification(magnification) {
            return false;
        }
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }

    /// Returns the current explicit tool override (if any).
    pub fn tool_override(&self) -> Option<Tool> {
        self.style.tool_override
    }

    pub fn drag_tool_bindings(&self) -> DragToolBindings {
        self.keymap.drag_tool_bindings()
    }

    /// Sets drag modifier -> tool mappings. Returns true if changed.
    pub fn set_drag_tool_bindings(&mut self, bindings: DragToolBindings) -> bool {
        if !self.keymap.set_drag_tool_bindings(bindings) {
            return false;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    pub fn drag_binding_for_button(&self, button: MouseButton) -> DragBinding {
        self.keymap.drag_binding_for_button(button, self.modifiers)
    }

    pub(crate) fn active_drag_color_or_current(&self) -> Color {
        self.keymap
            .active_drag_color()
            .unwrap_or_else(|| self.color_for_active_tool())
    }

    pub(crate) fn active_drag_color_or_tool(&self, tool: Tool) -> Color {
        self.keymap
            .active_drag_color()
            .unwrap_or_else(|| self.color_for_tool(tool))
    }

    pub(crate) fn begin_pointer_drag(&mut self, button: MouseButton, color: Option<Color>) {
        self.keymap.begin_pointer_drag(button, color);
    }

    pub(crate) fn end_pointer_drag(&mut self) {
        self.keymap.end_pointer_drag();
        // A block-move drag (Alt+drag in text mode) rides on the pointer drag,
        // so tearing the drag down always clears it.
        self.text_editing.set_text_block_drag(None);
    }

    pub(crate) fn pointer_drag_active(&self) -> bool {
        self.keymap.pointer_drag_active()
    }

    pub(crate) fn pointer_drag_button_matches(&self, button: MouseButton) -> bool {
        self.keymap.pointer_drag_button_matches(button)
    }

    /// Sets thickness or eraser size depending on the active tool.
    pub fn set_thickness_for_active_tool(&mut self, value: f64) -> bool {
        with_legacy_measurer(|measurer| self.set_thickness_for_active_tool_with(measurer, value))
    }

    pub fn set_thickness_for_active_tool_with(
        &mut self,
        measurer: &TextMeasurer,
        value: f64,
    ) -> bool {
        let changed = if self.active_tool().uses_eraser_size() {
            self.set_eraser_size_with(measurer, value)
        } else {
            self.set_thickness_with(measurer, value)
        };
        if changed {
            self.pending_onboarding_usage.used_thickness_change = true;
        }
        changed
    }

    /// Nudges thickness or eraser size depending on the active tool.
    pub fn nudge_thickness_for_active_tool(&mut self, delta: f64) -> bool {
        with_legacy_measurer(|measurer| self.nudge_thickness_for_active_tool_with(measurer, delta))
    }

    pub fn nudge_thickness_for_active_tool_with(
        &mut self,
        measurer: &TextMeasurer,
        delta: f64,
    ) -> bool {
        let tool = self.active_tool();
        let changed = if tool.uses_eraser_size() {
            self.set_eraser_size_with(measurer, self.style.eraser_size + delta)
        } else {
            self.set_thickness_with(measurer, self.thickness_for_tool(tool) + delta)
        };
        if changed {
            self.pending_onboarding_usage.used_thickness_change = true;
        }
        changed
    }

    /// Returns the current size value for the active tool.
    pub fn size_for_active_tool(&self) -> f64 {
        self.thickness_for_active_tool()
    }

    /// Updates the current drawing color to an arbitrary value. Returns true if changed.
    pub fn set_color(&mut self, color: Color) -> bool {
        let tool = self.active_tool();
        if !self.style.set_color(tool, color) {
            return false;
        }
        self.preset_slots.clear_active();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.sync_highlight_color();
        self.mark_session_dirty();
        true
    }

    /// Sets the absolute thickness (px), clamped to valid bounds. Returns true if changed.
    pub fn set_thickness(&mut self, thickness: f64) -> bool {
        with_legacy_measurer(|measurer| self.set_thickness_with(measurer, thickness))
    }

    pub fn set_thickness_with(&mut self, measurer: &TextMeasurer, thickness: f64) -> bool {
        let clamped = thickness.clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
        let tool = self.active_tool();
        let current = self.style.tool_settings.get(tool).thickness;
        if (clamped - current).abs() < f64::EPSILON {
            return false;
        }

        self.mark_current_provisional_dirty_full_with(measurer);
        let changed = self.style.set_thickness(tool, clamped);
        debug_assert!(changed);
        self.mark_current_provisional_dirty_full_with(measurer);
        self.preset_slots.clear_active();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }

    /// Sets the absolute eraser size (px), clamped to valid bounds. Returns true if changed.
    pub fn set_eraser_size(&mut self, size: f64) -> bool {
        with_legacy_measurer(|measurer| self.set_eraser_size_with(measurer, size))
    }

    pub fn set_eraser_size_with(&mut self, measurer: &TextMeasurer, size: f64) -> bool {
        let clamped = size.clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
        if (clamped - self.style.eraser_size).abs() < f64::EPSILON {
            return false;
        }
        self.mark_current_provisional_dirty_full_with(measurer);
        let changed = self.style.set_eraser_size(clamped);
        debug_assert!(changed);
        self.mark_current_provisional_dirty_full_with(measurer);
        self.preset_slots.clear_active();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }

    /// Sets the eraser behavior mode. Returns true if changed.
    pub fn set_eraser_mode(&mut self, mode: EraserMode) -> bool {
        if !self.style.set_eraser_mode(mode) {
            return false;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }

    /// Toggles between brush and stroke eraser modes.
    pub fn toggle_eraser_mode(&mut self) -> bool {
        if !self.style.toggle_eraser_mode() {
            return false;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }

    pub(crate) fn eraser_hit_radius(&self) -> f64 {
        self.style.eraser_hit_radius()
    }

    /// Sets how the blur tool obscures its region. Returns true if changed.
    pub fn set_blur_style(&mut self, style: BlurStyle) -> bool {
        if !self.style.set_blur_style(style) {
            return false;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }

    /// Steps to the next blur style, wrapping around.
    pub fn cycle_blur_style(&mut self) -> bool {
        if !self.style.cycle_blur_style() {
            return false;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.mark_session_dirty();

        if self.style.blur_style.needs_backdrop()
            && self.active_tool() == Tool::Blur
            && !self.frozen_active()
            && !self.pending_frozen_toggle()
        {
            self.request_frozen_toggle();
        }

        true
    }

    /// Sets the style copied into the next arrow. Returns true if changed.
    pub fn set_arrow_style(&mut self, style: ArrowStyle) -> bool {
        if !self.style.set_arrow_style(style) {
            return false;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }

    /// Steps to the next arrow style, wrapping around.
    pub fn cycle_arrow_style(&mut self) -> bool {
        if !self.style.cycle_arrow_style() {
            return false;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }

    /// Sets the font descriptor used for text rendering. Returns true if changed.
    #[allow(dead_code)]
    pub fn set_font_descriptor(&mut self, descriptor: FontDescriptor) -> bool {
        if !self.style.set_font_descriptor(descriptor) {
            return false;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }

    /// Sets the absolute font size (px), clamped to the same range as config validation.
    #[allow(dead_code)]
    pub fn set_font_size(&mut self, size: f64) -> bool {
        if !self.style.set_font_size(size) {
            return false;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }

    /// Enables or disables fill for fill-capable shapes.
    pub fn set_fill_enabled(&mut self, enabled: bool) -> bool {
        if !self.style.set_fill_enabled(enabled) {
            return false;
        }
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }

    pub fn set_polygon_sides(&mut self, sides: u8) -> bool {
        if !self.style.set_polygon_sides(sides) {
            return false;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }

    pub fn nudge_polygon_sides(&mut self, delta: i8) -> bool {
        if !self.style.nudge_polygon_sides(delta) {
            return false;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }
}
