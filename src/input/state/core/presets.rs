use super::base::{PresetFeedbackKind, PresetFeedbackState};
use crate::config::{PRESET_SLOTS_MAX, PresetSlotsConfig, ToolPresetConfig};
use std::time::{Duration, Instant};

/// Runtime preset slots, active selection, and transient feedback state.
pub(crate) struct PresetSlots {
    pub(crate) preset_slot_count: usize,
    pub(crate) presets: Vec<Option<ToolPresetConfig>>,
    pub(crate) active_preset_slot: Option<usize>,
    pub(crate) preset_feedback: Vec<Option<PresetFeedbackState>>,
}

impl PresetSlots {
    pub(crate) fn slot_count(&self) -> usize {
        self.preset_slot_count
    }

    pub(crate) fn presets(&self) -> &[Option<ToolPresetConfig>] {
        &self.presets
    }

    pub(crate) fn active(&self) -> Option<usize> {
        self.active_preset_slot
    }

    pub(crate) fn feedback(&self) -> &[Option<PresetFeedbackState>] {
        &self.preset_feedback
    }

    pub(crate) fn preset(&self, slot: usize) -> Option<ToolPresetConfig> {
        self.preset_index(slot)
            .and_then(|index| self.presets.get(index))
            .and_then(Option::as_ref)
            .cloned()
    }

    pub(crate) fn save(
        &mut self,
        slot: usize,
        mut preset: ToolPresetConfig,
    ) -> Option<ToolPresetConfig> {
        let index = self.preset_index(slot)?;
        if let Some(existing) = self.presets.get(index).and_then(Option::as_ref) {
            if preset.name.is_none() {
                preset.name.clone_from(&existing.name);
            }
            if existing == &preset {
                return None;
            }
        }
        *self.presets.get_mut(index)? = Some(preset.clone());
        Some(preset)
    }

    pub(crate) fn clear(&mut self, slot: usize) -> Option<bool> {
        let index = self.preset_index(slot)?;
        let slot = self.presets.get_mut(index)?;
        let had_preset = slot.take().is_some();
        if self.active_preset_slot == Some(index + 1) {
            self.active_preset_slot = None;
        }
        Some(had_preset)
    }

    pub(crate) fn activate(&mut self, slot: usize) {
        self.active_preset_slot = Some(slot);
    }

    pub(crate) fn restore_active(&mut self, slot: Option<usize>) {
        self.active_preset_slot = slot;
    }

    pub(crate) fn clear_active(&mut self) {
        self.active_preset_slot = None;
    }

    pub(crate) fn set_feedback(&mut self, slot: usize, kind: PresetFeedbackKind, now: Instant) {
        let Some(index) = self.preset_index(slot) else {
            return;
        };
        if self.preset_feedback.len() < PRESET_SLOTS_MAX {
            self.preset_feedback.resize_with(PRESET_SLOTS_MAX, || None);
        }
        if let Some(slot) = self.preset_feedback.get_mut(index) {
            *slot = Some(PresetFeedbackState { kind, started: now });
        }
    }

    pub(crate) fn advance_feedback(&mut self, now: Instant, duration: Duration) -> bool {
        if !self.preset_feedback.iter().any(Option::is_some) {
            return false;
        }

        let mut active = false;
        for slot in &mut self.preset_feedback {
            let expired = slot
                .as_ref()
                .is_some_and(|state| now.saturating_duration_since(state.started) >= duration);
            if expired {
                *slot = None;
            } else if slot.is_some() {
                active = true;
            }
        }
        active
    }

    fn preset_index(&self, slot: usize) -> Option<usize> {
        (slot > 0 && slot <= PRESET_SLOTS_MAX && slot <= self.preset_slot_count).then(|| slot - 1)
    }
}

impl Default for PresetSlots {
    fn default() -> Self {
        Self {
            preset_slot_count: PRESET_SLOTS_MAX,
            presets: vec![None; PRESET_SLOTS_MAX],
            active_preset_slot: None,
            preset_feedback: vec![None; PRESET_SLOTS_MAX],
        }
    }
}

impl From<&PresetSlotsConfig> for PresetSlots {
    fn from(config: &PresetSlotsConfig) -> Self {
        let mut presets = (1..=PRESET_SLOTS_MAX)
            .map(|slot| config.get_slot(slot).cloned())
            .collect::<Vec<_>>();
        presets.resize_with(PRESET_SLOTS_MAX, || None);

        Self {
            preset_slot_count: config.slot_count,
            presets,
            active_preset_slot: None,
            preset_feedback: vec![None; PRESET_SLOTS_MAX],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marker_preset() -> ToolPresetConfig {
        ToolPresetConfig {
            name: Some("Marker".into()),
            tool: crate::input::Tool::Marker,
            color: crate::config::ColorSpec::Name("yellow".into()),
            size: 8.0,
            tool_settings: None,
            eraser_kind: None,
            eraser_mode: None,
            marker_opacity: None,
            fill_enabled: None,
            font_size: None,
            text_background_enabled: None,
            arrow_length: None,
            arrow_angle: None,
            arrow_head_at_end: None,
            polygon_sides: None,
            show_status_bar: None,
            drag_tools: None,
        }
    }

    #[test]
    fn default_allocates_every_supported_slot() {
        let slots = PresetSlots::default();

        assert_eq!(slots.preset_slot_count, PRESET_SLOTS_MAX);
        assert_eq!(slots.presets, vec![None; PRESET_SLOTS_MAX]);
        assert_eq!(slots.preset_feedback.len(), PRESET_SLOTS_MAX);
        assert!(slots.active_preset_slot.is_none());
    }

    #[test]
    fn save_preserves_an_existing_name_and_clear_resets_the_active_slot() {
        let mut slots = PresetSlots::default();
        let original = marker_preset();
        assert_eq!(slots.save(2, original.clone()), Some(original));
        slots.activate(2);

        let mut replacement = marker_preset();
        replacement.name = None;
        replacement.size = 12.0;
        let saved = slots.save(2, replacement).expect("changed preset");

        assert_eq!(saved.name.as_deref(), Some("Marker"));
        assert_eq!(slots.clear(2), Some(true));
        assert!(slots.active().is_none());
        assert_eq!(slots.clear(0), None);
    }

    #[test]
    fn feedback_expires_at_the_supplied_duration() {
        let mut slots = PresetSlots::default();
        let started = Instant::now();
        slots.set_feedback(1, PresetFeedbackKind::Save, started);

        assert!(slots.advance_feedback(started, Duration::from_secs(1)));
        assert!(!slots.advance_feedback(started + Duration::from_secs(1), Duration::from_secs(1)));
        assert!(slots.feedback()[0].is_none());
    }

    #[test]
    fn from_config_preserves_the_visible_count_and_slot_positions() {
        let mut config = PresetSlotsConfig {
            slot_count: 3,
            ..Default::default()
        };
        let preset = marker_preset();
        config.slot_2 = Some(preset.clone());

        let slots = PresetSlots::from(&config);

        assert_eq!(slots.preset_slot_count, 3);
        assert_eq!(slots.presets[1], Some(preset));
        assert_eq!(slots.presets.len(), PRESET_SLOTS_MAX);
        assert!(slots.active_preset_slot.is_none());
    }
}
