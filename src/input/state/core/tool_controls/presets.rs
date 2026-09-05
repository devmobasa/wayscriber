use super::super::base::{
    DrawingState, InputState, PRESET_FEEDBACK_DURATION_MS, PRESET_TOAST_DURATION_MS,
    PresetFeedbackKind,
};
use super::super::default_step_marker_size;
use crate::config::{PresetSlotsConfig, PresetToolStatesConfig, ToolPresetConfig};
use crate::draw::TextMeasurer;
use crate::input::{DragModifier, tool::Tool};
use std::time::{Duration, Instant};

impl InputState {
    pub fn init_presets_from_config(&mut self, presets: &PresetSlotsConfig) {
        self.preset_slots = super::super::PresetSlots::from(presets);
    }

    pub fn apply_preset(&mut self, slot: usize) -> bool {
        let measurer = TextMeasurer::default();
        self.apply_preset_with(&measurer, slot)
    }

    pub fn apply_preset_with(&mut self, measurer: &TextMeasurer, slot: usize) -> bool {
        let Some(preset) = self.preset_slots.preset(slot) else {
            return false;
        };

        match self.state {
            DrawingState::TextInput { .. } => self.cancel_text_input_with(measurer),
            DrawingState::BuildingPolygon { .. } => self.cancel_active_interaction_with(measurer),
            _ => {}
        }

        let legacy_step_marker_preset =
            preset.tool_settings.is_none() && preset.tool == Tool::StepMarker;

        if let Some(tool_settings) = preset.tool_settings.as_ref() {
            self.apply_full_preset_tool_settings(tool_settings);
            self.activate_preset_tool_with(measurer, preset.tool);
            self.sync_current_settings_from_active_tool();
        } else {
            self.activate_preset_tool_with(measurer, preset.tool);
            let _ = self.set_color(preset.color.to_color());
            if preset.tool.uses_eraser_size() {
                let _ = self.set_eraser_size_with(measurer, preset.size);
            } else if !legacy_step_marker_preset {
                let _ = self.set_thickness_with(measurer, preset.size);
            }
        }

        if let Some(kind) = preset.eraser_kind
            && self.style.eraser_kind != kind
        {
            self.style.eraser_kind = kind;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            self.mark_session_dirty();
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
        if legacy_step_marker_preset {
            let _ = self.set_thickness_with(
                measurer,
                default_step_marker_size(self.style.current_font_size),
            );
        }
        if let Some(text_background_enabled) = preset.text_background_enabled
            && self.style.text_background_enabled != text_background_enabled
        {
            self.style.text_background_enabled = text_background_enabled;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            self.mark_session_dirty();
        }
        self.apply_preset_shape_settings(&preset);
        // Redraw only: the bar's visibility is a this-run preference the
        // session snapshot does not carry, unlike every tool value above it.
        if let Some(show_status_bar) = preset.show_status_bar
            && !(self.presenter_mode_active() && self.presenter_mode_config().hide_status_bar)
            && self.set_status_bar_visibility_preserving_focus(show_status_bar)
        {
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
        if let Some(drag_tools) = preset.drag_tools.as_ref() {
            let left_defaults = self.drag_tool_bindings().to_config().left;
            let drag_tools = drag_tools
                .clone()
                .resolve_with_left_defaults(&left_defaults);
            let _ = self
                .set_drag_tool_bindings(crate::input::DragToolBindings::from_config(&drag_tools));
        }

        self.preset_slots.activate(slot);
        self.set_preset_feedback(slot, PresetFeedbackKind::Apply);
        true
    }

    fn apply_preset_shape_settings(&mut self, preset: &ToolPresetConfig) {
        if self.style.apply_preset_shape_settings(preset) {
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            self.mark_session_dirty();
        }
    }

    pub fn save_preset(&mut self, slot: usize) -> bool {
        let preset = self.capture_current_preset();
        let Some(preset) = self.preset_slots.save(slot, preset) else {
            return false;
        };
        self.set_preset_feedback(slot, PresetFeedbackKind::Save);
        self.emit_input_effect(super::super::base::InputEffect::Preset(
            super::super::base::PresetAction::Save {
                slot,
                preset: Box::new(preset),
            },
        ));
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    pub fn clear_preset(&mut self, slot: usize) -> bool {
        let Some(had_preset) = self.preset_slots.clear(slot) else {
            return false;
        };
        if had_preset {
            self.set_preset_feedback(slot, PresetFeedbackKind::Clear);
        }
        self.emit_input_effect(super::super::base::InputEffect::Preset(
            super::super::base::PresetAction::Clear { slot },
        ));
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        had_preset
    }

    pub fn advance_preset_feedback(&mut self, now: Instant) -> bool {
        let duration_ms = if self.ui_visibility.show_preset_toasts {
            PRESET_TOAST_DURATION_MS
        } else {
            PRESET_FEEDBACK_DURATION_MS
        };
        self.preset_slots
            .advance_feedback(now, Duration::from_millis(duration_ms))
    }

    fn set_preset_feedback(&mut self, slot: usize, kind: PresetFeedbackKind) {
        self.preset_slots.set_feedback(slot, kind, Instant::now());
        self.needs_redraw = true;
    }

    fn activate_preset_tool_with(&mut self, measurer: &TextMeasurer, tool: Tool) {
        if tool == Tool::Highlight {
            self.set_highlight_tool_with_measurer(measurer, true);
        } else {
            self.set_tool_override_with(measurer, Some(tool));
        }
    }

    fn apply_full_preset_tool_settings(&mut self, settings: &PresetToolStatesConfig) {
        if self.style.apply_full_preset_tool_settings(settings) {
            self.preset_slots.clear_active();
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            self.mark_session_dirty();
        }
        self.sync_highlight_color();
    }

    fn capture_current_preset(&self) -> ToolPresetConfig {
        // Saving is commonly invoked with Shift+1..5. Those shortcut
        // modifiers also select temporary drag tools (Shift = Line, Ctrl =
        // Rect, and so on), so `active_tool()` would capture the shortcut's
        // transient tool instead of the user's persistent selection.
        let selected_tool = self.tool_override().unwrap_or_else(|| {
            self.drag_tool_bindings()
                .tool_for_modifier(DragModifier::None)
        });
        self.style.capture_preset(
            selected_tool,
            self.ui_visibility.show_status_bar,
            self.drag_tool_bindings().to_config(),
        )
    }
}
