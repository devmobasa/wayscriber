use super::super::base::{
    DrawingState, InputState, MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS, UiToastKind,
};
use crate::draw::{Color, FontDescriptor, clamp_regular_sides};
use crate::input::{
    DragBinding, MouseButton,
    modifiers::DragToolBindings,
    tool::{EraserMode, PerToolDrawingSettings, Tool},
};
use crate::ui::toolbar::model::ToolbarSliderSpec;

impl InputState {
    /// Returns the stored drawing color for a tool.
    pub fn color_for_tool(&self, tool: Tool) -> Color {
        self.tool_settings.get(tool).color
    }

    /// Returns the drawing color for the currently active tool.
    pub fn color_for_active_tool(&self) -> Color {
        self.color_for_tool(self.active_tool())
    }

    /// Returns the stored size for a tool, using eraser size for the eraser.
    pub fn thickness_for_tool(&self, tool: Tool) -> f64 {
        if tool.uses_eraser_size() {
            self.eraser_size
        } else {
            self.tool_settings.get(tool).thickness
        }
    }

    /// Returns the stored size for the currently active tool.
    pub fn thickness_for_active_tool(&self) -> f64 {
        self.thickness_for_tool(self.active_tool())
    }

    /// Replaces all per-tool settings and refreshes the visible active values.
    pub(crate) fn replace_tool_settings(&mut self, settings: PerToolDrawingSettings) {
        self.tool_settings = settings;
        self.sync_current_settings_from_active_tool();
        self.sync_highlight_color();
    }

    /// Updates the compatibility current_* fields from the active tool settings.
    pub(crate) fn sync_current_settings_from_active_tool(&mut self) {
        let tool = self.active_tool();
        self.current_color = self.color_for_tool(tool);
        if tool.uses_drawing_thickness() {
            self.current_thickness = self.thickness_for_tool(tool);
        }
    }

    pub(crate) fn sync_current_settings_for_tool(&mut self, tool: Tool) {
        self.current_color = self.color_for_tool(tool);
        if tool.uses_drawing_thickness() {
            self.current_thickness = self.thickness_for_tool(tool);
        }
    }

    pub(crate) fn set_pen_color_from_board(&mut self, color: Color) {
        self.tool_settings.pen.color = color;
        if PerToolDrawingSettings::settings_tool(self.active_tool()) == Tool::Pen {
            self.current_color = color;
        }
        self.sync_highlight_color();
    }

    pub(crate) fn preview_color_for_tool(&mut self, tool: Tool, color: Color) -> bool {
        if self.color_for_tool(tool) == color {
            return false;
        }
        self.tool_settings.get_mut(tool).color = color;
        if self.active_tool().settings_slot() == tool.settings_slot() {
            self.current_color = color;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.sync_highlight_color();
        true
    }

    /// Updates the active drawing thickness from tablet pressure without
    /// treating every pressure sample as a persisted user preference edit.
    #[cfg_attr(not(tablet), allow(dead_code))]
    pub(crate) fn set_pressure_thickness_for_active_tool(&mut self, thickness: f64) -> f64 {
        let clamped = thickness.clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
        let tool = self.active_tool();
        if tool.uses_drawing_thickness() {
            self.tool_settings.get_mut(tool).thickness = clamped;
        }
        self.current_thickness = clamped;
        self.update_initial_pressure_sample(clamped);
        self.needs_redraw = true;
        clamped
    }

    #[allow(dead_code)] // Used by the binary Wayland backend; the lib target has no backend modules.
    pub(crate) fn replace_active_drawing_pressure_samples(&mut self, thickness: f64) -> bool {
        let clamped = thickness.clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS) as f32;
        let DrawingState::Drawing {
            point_thicknesses, ..
        } = &mut self.state
        else {
            return false;
        };
        if point_thicknesses.is_empty() {
            return false;
        }

        point_thicknesses.fill(clamped);
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

    /// Sets or clears an explicit tool override. Returns true if the tool changed.
    pub fn set_tool_override(&mut self, tool: Option<Tool>) -> bool {
        if self.presenter_mode
            && matches!(
                self.presenter_mode_config.tool_behavior,
                crate::config::PresenterToolBehavior::ForceHighlightLocked
            )
            && tool != Some(Tool::Highlight)
        {
            return false;
        }
        if self.tool_override == tool {
            return false;
        }

        self.tool_override = tool;
        self.active_preset_slot = None;

        if tool == Some(Tool::Blur) && !self.frozen_active && !self.pending_frozen_toggle {
            self.request_frozen_toggle();
            self.set_ui_toast(UiToastKind::Info, "Capturing background for blur...");
        }

        // Ensure we are not mid-drawing with a stale tool
        if !matches!(
            self.state,
            DrawingState::Idle | DrawingState::TextInput { .. }
        ) {
            self.cancel_active_interaction();
        }

        self.sync_current_settings_from_active_tool();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }

    /// Sets the marker opacity multiplier (0.05-0.9). Returns true if changed.
    pub fn set_marker_opacity(&mut self, opacity: f64) -> bool {
        let spec = ToolbarSliderSpec::MARKER_OPACITY;
        let clamped = opacity.clamp(spec.min, spec.max);
        if (clamped - self.marker_opacity).abs() < f64::EPSILON {
            return false;
        }
        self.marker_opacity = clamped;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }

    /// Returns the current explicit tool override (if any).
    pub fn tool_override(&self) -> Option<Tool> {
        self.tool_override
    }

    /// Sets drag modifier -> tool mappings. Returns true if changed.
    pub fn set_drag_tool_bindings(&mut self, bindings: DragToolBindings) -> bool {
        if self.drag_tool_bindings == bindings {
            return false;
        }
        self.drag_tool_bindings = bindings;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    pub fn drag_binding_for_button(&self, button: MouseButton) -> DragBinding {
        self.drag_tool_bindings
            .binding_for_button_modifier(button, self.modifiers.active_drag_modifier())
    }

    pub(crate) fn active_drag_color_or_current(&self) -> Color {
        self.active_drag_color
            .unwrap_or_else(|| self.color_for_active_tool())
    }

    pub(crate) fn active_drag_color_or_tool(&self, tool: Tool) -> Color {
        self.active_drag_color
            .unwrap_or_else(|| self.color_for_tool(tool))
    }

    pub(crate) fn begin_pointer_drag(&mut self, button: MouseButton, color: Option<Color>) {
        self.active_drag_button = Some(button);
        self.active_drag_color = color;
    }

    pub(crate) fn end_pointer_drag(&mut self) {
        self.active_drag_button = None;
        self.active_drag_color = None;
    }

    pub(crate) fn pointer_drag_button_matches(&self, button: MouseButton) -> bool {
        self.active_drag_button == Some(button)
    }

    /// Sets thickness or eraser size depending on the active tool.
    pub fn set_thickness_for_active_tool(&mut self, value: f64) -> bool {
        if self.active_tool().uses_eraser_size() {
            self.set_eraser_size(value)
        } else {
            self.set_thickness(value)
        }
    }

    /// Nudges thickness or eraser size depending on the active tool.
    pub fn nudge_thickness_for_active_tool(&mut self, delta: f64) -> bool {
        let tool = self.active_tool();
        if tool.uses_eraser_size() {
            self.set_eraser_size(self.eraser_size + delta)
        } else {
            self.set_thickness(self.thickness_for_tool(tool) + delta)
        }
    }

    /// Returns the current size value for the active tool.
    pub fn size_for_active_tool(&self) -> f64 {
        self.thickness_for_active_tool()
    }

    /// Updates the current drawing color to an arbitrary value. Returns true if changed.
    pub fn set_color(&mut self, color: Color) -> bool {
        let tool = self.active_tool();
        let current = self.color_for_tool(tool);
        if current == color {
            return false;
        }

        self.tool_settings.get_mut(tool).color = color;
        self.current_color = color;
        self.active_preset_slot = None;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.sync_highlight_color();
        self.mark_session_dirty();
        true
    }

    /// Sets the absolute thickness (px), clamped to valid bounds. Returns true if changed.
    pub fn set_thickness(&mut self, thickness: f64) -> bool {
        let clamped = thickness.clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
        let tool = self.active_tool();
        let current = self.tool_settings.get(tool).thickness;
        if (clamped - current).abs() < f64::EPSILON {
            return false;
        }

        self.tool_settings.get_mut(tool).thickness = clamped;
        self.current_thickness = clamped;
        self.active_preset_slot = None;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }

    /// Sets the absolute eraser size (px), clamped to valid bounds. Returns true if changed.
    pub fn set_eraser_size(&mut self, size: f64) -> bool {
        let clamped = size.clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
        if (clamped - self.eraser_size).abs() < f64::EPSILON {
            return false;
        }
        self.eraser_size = clamped;
        self.active_preset_slot = None;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.mark_session_dirty();
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
        self.mark_session_dirty();
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
        self.mark_session_dirty();
        true
    }

    /// Sets the absolute font size (px), clamped to the same range as config validation.
    #[allow(dead_code)]
    pub fn set_font_size(&mut self, size: f64) -> bool {
        let spec = ToolbarSliderSpec::FONT_SIZE;
        let clamped = size.clamp(spec.min, spec.max);
        if (clamped - self.current_font_size).abs() < f64::EPSILON {
            return false;
        }

        self.current_font_size = clamped;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }

    /// Enables or disables fill for fill-capable shapes.
    pub fn set_fill_enabled(&mut self, enabled: bool) -> bool {
        if self.fill_enabled == enabled {
            return false;
        }
        self.fill_enabled = enabled;
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }

    pub fn set_polygon_sides(&mut self, sides: u8) -> bool {
        let clamped = clamp_regular_sides(sides);
        if self.polygon_sides == clamped {
            return false;
        }
        self.polygon_sides = clamped;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }

    pub fn nudge_polygon_sides(&mut self, delta: i8) -> bool {
        let next = if delta.is_negative() {
            self.polygon_sides.saturating_sub(delta.unsigned_abs())
        } else {
            self.polygon_sides.saturating_add(delta as u8)
        };
        self.set_polygon_sides(next)
    }
}
