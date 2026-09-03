use super::base::PresetFeedbackState;
use crate::config::{PRESET_SLOTS_MAX, PresetSlotsConfig, ToolPresetConfig};

/// Runtime preset slots, active selection, and transient feedback state.
pub(crate) struct PresetSlots {
    pub(crate) preset_slot_count: usize,
    pub(crate) presets: Vec<Option<ToolPresetConfig>>,
    pub(crate) active_preset_slot: Option<usize>,
    pub(crate) preset_feedback: Vec<Option<PresetFeedbackState>>,
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

    #[test]
    fn default_allocates_every_supported_slot() {
        let slots = PresetSlots::default();

        assert_eq!(slots.preset_slot_count, PRESET_SLOTS_MAX);
        assert_eq!(slots.presets, vec![None; PRESET_SLOTS_MAX]);
        assert_eq!(slots.preset_feedback.len(), PRESET_SLOTS_MAX);
        assert!(slots.active_preset_slot.is_none());
    }

    #[test]
    fn from_config_preserves_the_visible_count_and_slot_positions() {
        let mut config = PresetSlotsConfig {
            slot_count: 3,
            ..Default::default()
        };
        let preset = ToolPresetConfig {
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
        };
        config.slot_2 = Some(preset.clone());

        let slots = PresetSlots::from(&config);

        assert_eq!(slots.preset_slot_count, 3);
        assert_eq!(slots.presets[1], Some(preset));
        assert_eq!(slots.presets.len(), PRESET_SLOTS_MAX);
        assert!(slots.active_preset_slot.is_none());
    }
}
