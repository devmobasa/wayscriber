use super::base::{
    DrawingState, InputState, PresetFeedbackKind, PresetFeedbackState,
    PRESET_FEEDBACK_DURATION_MS, MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS,
};
use crate::config::{Action, PresetSlotsConfig, ToolPresetConfig, PRESET_SLOTS_MAX};
use crate::draw::{Color, FontDescriptor};
use crate::input::tool::{EraserMode, Tool};
use std::time::{Duration, Instant};

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

    pub fn init_presets_from_config(&mut self, presets: &PresetSlotsConfig) {
        self.preset_slot_count = presets.slot_count;
        self.presets = vec![
            presets.slot_1.clone(),
            presets.slot_2.clone(),
            presets.slot_3.clone(),
            presets.slot_4.clone(),
            presets.slot_5.clone(),
        ];
        if self.presets.len() < PRESET_SLOTS_MAX {
            self.presets.resize_with(PRESET_SLOTS_MAX, || None);
        }
    }

    pub fn apply_preset(&mut self, slot: usize) -> bool {
        let index = match self.preset_index(slot) {
            Some(index) => index,
            None => return false,
        };
        let preset = match self.presets.get(index).and_then(|p| p.as_ref()) {
            Some(preset) => preset.clone(),
            None => return false,
        };

        if matches!(self.state, DrawingState::TextInput { .. }) {
            self.clear_text_preview_dirty();
            self.last_text_preview_bounds = None;
            self.state = DrawingState::Idle;
        }

        if preset.tool == Tool::Highlight {
            self.set_highlight_tool(true);
        } else {
            self.set_tool_override(Some(preset.tool));
        }

        let _ = self.set_color(preset.color.to_color());
        if preset.tool == Tool::Eraser {
            let _ = self.set_eraser_size(preset.size);
        } else {
            let _ = self.set_thickness(preset.size);
        }

        if let Some(kind) = preset.eraser_kind {
            if self.eraser_kind != kind {
                self.eraser_kind = kind;
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
            }
        }
        if let Some(mode) = preset.eraser_mode {
            let _ = self.set_eraser_mode(mode);
        }
        if let Some(opacity) = preset.marker_opacity {
            let _ = self.set_marker_opacity(opacity);
        }
        if let Some(fill_enabled) = preset.fill_enabled {
            let _ = self.set_fill_enabled(fill_enabled);
        }
        if let Some(font_size) = preset.font_size {
            let _ = self.set_font_size(font_size);
        }
        if let Some(text_background_enabled) = preset.text_background_enabled {
            if self.text_background_enabled != text_background_enabled {
                self.text_background_enabled = text_background_enabled;
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
            }
        }
        if let Some(length) = preset.arrow_length {
            let clamped = length.clamp(5.0, 50.0);
            if (self.arrow_length - clamped).abs() > f64::EPSILON {
                self.arrow_length = clamped;
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
            }
        }
        if let Some(angle) = preset.arrow_angle {
            let clamped = angle.clamp(15.0, 60.0);
            if (self.arrow_angle - clamped).abs() > f64::EPSILON {
                self.arrow_angle = clamped;
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
            }
        }
        if let Some(head_at_end) = preset.arrow_head_at_end {
            if self.arrow_head_at_end != head_at_end {
                self.arrow_head_at_end = head_at_end;
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
            }
        }
        if let Some(show_status_bar) = preset.show_status_bar {
            if self.show_status_bar != show_status_bar {
                self.show_status_bar = show_status_bar;
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
            }
        }

        self.set_preset_feedback(slot, PresetFeedbackKind::Apply);
        true
    }

    pub fn save_preset(&mut self, slot: usize) -> bool {
        let index = match self.preset_index(slot) {
            Some(index) => index,
            None => return false,
        };
        let mut preset = self.capture_current_preset();
        if let Some(existing) = self.presets.get(index).and_then(|p| p.as_ref()) {
            if preset.name.is_none() {
                preset.name = existing.name.clone();
            }
            if existing == &preset {
                return false;
            }
        }
        if let Some(slot_ref) = self.presets.get_mut(index) {
            *slot_ref = Some(preset.clone());
        }
        self.set_preset_feedback(slot, PresetFeedbackKind::Save);
        self.pending_preset_action = Some(super::base::PresetAction::Save { slot, preset });
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    pub fn clear_preset(&mut self, slot: usize) -> bool {
        let index = match self.preset_index(slot) {
            Some(index) => index,
            None => return false,
        };
        let had_preset = self
            .presets
            .get(index)
            .and_then(|preset| preset.as_ref())
            .is_some();
        if let Some(slot_ref) = self.presets.get_mut(index) {
            *slot_ref = None;
        }
        if had_preset {
            self.set_preset_feedback(slot, PresetFeedbackKind::Clear);
        }
        self.pending_preset_action = Some(super::base::PresetAction::Clear { slot });
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        had_preset
    }

    pub fn advance_preset_feedback(&mut self, now: Instant) -> bool {
        let duration = Duration::from_millis(PRESET_FEEDBACK_DURATION_MS);
        let mut active = false;
        for slot in &mut self.preset_feedback {
            let expired = slot
                .as_ref()
                .map(|state| now.saturating_duration_since(state.started) >= duration)
                .unwrap_or(false);
            if expired {
                *slot = None;
            } else if slot.is_some() {
                active = true;
            }
        }
        active
    }

    fn preset_index(&self, slot: usize) -> Option<usize> {
        if slot == 0 || slot > PRESET_SLOTS_MAX {
            return None;
        }
        if slot > self.preset_slot_count {
            return None;
        }
        Some(slot - 1)
    }

    fn set_preset_feedback(&mut self, slot: usize, kind: PresetFeedbackKind) {
        let index = match self.preset_index(slot) {
            Some(index) => index,
            None => return,
        };
        if self.preset_feedback.len() < PRESET_SLOTS_MAX {
            self.preset_feedback
                .resize_with(PRESET_SLOTS_MAX, || None);
        }
        if let Some(slot_ref) = self.preset_feedback.get_mut(index) {
            *slot_ref = Some(PresetFeedbackState {
                kind,
                started: Instant::now(),
            });
        }
        self.needs_redraw = true;
    }

    fn capture_current_preset(&self) -> ToolPresetConfig {
        let active_tool = self.active_tool();
        let size = if active_tool == Tool::Eraser {
            self.eraser_size
        } else {
            self.current_thickness
        };
        ToolPresetConfig {
            name: None,
            tool: active_tool,
            color: self.current_color.into(),
            size,
            eraser_kind: Some(self.eraser_kind),
            eraser_mode: Some(self.eraser_mode),
            marker_opacity: Some(self.marker_opacity),
            fill_enabled: Some(self.fill_enabled),
            font_size: Some(self.current_font_size),
            text_background_enabled: Some(self.text_background_enabled),
            arrow_length: Some(self.arrow_length),
            arrow_angle: Some(self.arrow_angle),
            arrow_head_at_end: Some(self.arrow_head_at_end),
            show_status_bar: Some(self.show_status_bar),
        }
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
