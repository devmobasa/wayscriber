use super::super::base::{
    DrawingState, InputState, PRESET_FEEDBACK_DURATION_MS, PRESET_TOAST_DURATION_MS,
    PresetFeedbackKind, PresetFeedbackState,
};
use crate::config::{PRESET_SLOTS_MAX, PresetSlotsConfig, ToolPresetConfig};
use crate::input::tool::Tool;
use std::time::{Duration, Instant};

impl InputState {
    pub fn init_presets_from_config(&mut self, presets: &PresetSlotsConfig) {
        self.preset_slot_count = presets.slot_count;
        self.presets = (1..=PRESET_SLOTS_MAX)
            .map(|slot| presets.get_slot(slot).cloned())
            .collect();
        if self.presets.len() < PRESET_SLOTS_MAX {
            self.presets.resize_with(PRESET_SLOTS_MAX, || None);
        }
        self.active_preset_slot = None;
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
            self.cancel_text_input();
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

        if let Some(kind) = preset.eraser_kind
            && self.eraser_kind != kind
        {
            self.eraser_kind = kind;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
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
        if let Some(text_background_enabled) = preset.text_background_enabled
            && self.text_background_enabled != text_background_enabled
        {
            self.text_background_enabled = text_background_enabled;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
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
        if let Some(head_at_end) = preset.arrow_head_at_end
            && self.arrow_head_at_end != head_at_end
        {
            self.arrow_head_at_end = head_at_end;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
        if let Some(show_status_bar) = preset.show_status_bar
            && !(self.presenter_mode && self.presenter_mode_config.hide_status_bar)
            && self.show_status_bar != show_status_bar
        {
            self.show_status_bar = show_status_bar;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }

        self.active_preset_slot = Some(slot);
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
        self.pending_preset_action = Some(super::super::base::PresetAction::Save { slot, preset });
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
        if self.active_preset_slot == Some(slot) {
            self.active_preset_slot = None;
        }
        self.pending_preset_action = Some(super::super::base::PresetAction::Clear { slot });
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        had_preset
    }

    pub fn advance_preset_feedback(&mut self, now: Instant) -> bool {
        // Early exit if no feedback is active
        if !self.preset_feedback.iter().any(|s| s.is_some()) {
            return false;
        }

        let duration_ms = if self.show_preset_toasts {
            PRESET_TOAST_DURATION_MS
        } else {
            PRESET_FEEDBACK_DURATION_MS
        };
        let duration = Duration::from_millis(duration_ms);
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
            self.preset_feedback.resize_with(PRESET_SLOTS_MAX, || None);
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
}
