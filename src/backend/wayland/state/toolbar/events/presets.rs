use super::*;
use crate::config::Config;
use crate::domain::Action;
use crate::input::state::{PresetAction, Toast, ToastPriority};

/// Apply an accepted preset save/clear to the effective config, and say what
/// the user should be told.
///
/// The preset library is an authored definition, so this run is as far as the
/// overlay's Save/Clear reaches: `InputState` already holds the live slots, and
/// this keeps the effective `Config` beside them so anything reading
/// `config.presets` this session sees the same library. Nothing is written, and
/// the message says so rather than letting a changed slot imply a saved one.
fn apply_preset_action(config: &mut Config, action: PresetAction) -> String {
    match action {
        PresetAction::Save { slot, preset } => {
            config.presets.set_slot(slot, Some(*preset));
            format!("Preset {slot} saved for this run — keep it via the configurator.")
        }
        PresetAction::Clear { slot } => {
            config.presets.set_slot(slot, None);
            format!("Preset {slot} cleared for this run — remove it via the configurator.")
        }
    }
}

impl WaylandState {
    pub(in crate::backend::wayland) fn handle_preset_action(&mut self, action: PresetAction) {
        let message = apply_preset_action(&mut self.config, action);
        self.input_state.push_toast(
            ToastPriority::Action,
            "presets",
            Toast::info(message).action("Edit", Action::OpenConfiguratorPresets),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::test_helpers::{ConfigFileSnapshot, with_temp_config_home};
    use crate::config::{ColorSpec, ToolPresetConfig};
    use crate::draw::Color;
    use std::fs;

    const AUTHORED_PRESET: &str =
        "[presets.slot_1]\nname = 'Authored'\ntool = 'pen'\ncolor = '#112233'\nsize = 3.0\n";

    fn preset(name: &str) -> Box<ToolPresetConfig> {
        Box::new(ToolPresetConfig {
            name: Some(name.to_string()),
            tool: crate::input::Tool::Pen,
            color: ColorSpec::from(Color {
                r: 0.13,
                g: 0.27,
                b: 0.41,
                a: 1.0,
            }),
            size: 7.0,
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
        })
    }

    #[test]
    fn saving_a_preset_updates_the_effective_config_and_says_it_is_for_this_run() {
        let mut config = Config::default();

        let message = apply_preset_action(
            &mut config,
            PresetAction::Save {
                slot: 2,
                preset: preset("Run preset"),
            },
        );

        assert_eq!(
            config
                .presets
                .get_slot(2)
                .and_then(|slot| slot.name.clone()),
            Some("Run preset".to_string())
        );
        assert_eq!(
            message,
            "Preset 2 saved for this run — keep it via the configurator."
        );
    }

    #[test]
    fn clearing_a_preset_empties_the_effective_slot_and_says_it_is_for_this_run() {
        let mut config = Config::default();
        let _ = apply_preset_action(
            &mut config,
            PresetAction::Save {
                slot: 1,
                preset: preset("Run preset"),
            },
        );

        let message = apply_preset_action(&mut config, PresetAction::Clear { slot: 1 });

        assert!(config.presets.get_slot(1).is_none());
        assert_eq!(
            message,
            "Preset 1 cleared for this run — remove it via the configurator."
        );
    }

    /// The whole point of the change: a preset gesture is a memory edit. The
    /// file keeps its bytes, its metadata, and its neighbours.
    #[test]
    fn preset_save_and_clear_leave_the_config_file_untouched() {
        with_temp_config_home(|config_root| {
            let config_dir = config_root.join(crate::config::PRIMARY_CONFIG_DIR);
            fs::create_dir_all(&config_dir).expect("test config directory");
            let path = config_dir.join("config.toml");
            fs::write(&path, AUTHORED_PRESET).expect("test config should be written");
            let snapshot = ConfigFileSnapshot::capture(&path);

            let mut config = Config::load().expect("test config should load").config;
            let _ = apply_preset_action(
                &mut config,
                PresetAction::Save {
                    slot: 1,
                    preset: preset("Run preset"),
                },
            );
            let _ = apply_preset_action(&mut config, PresetAction::Clear { slot: 2 });

            snapshot.assert_unchanged("saving and clearing a preset");
        });
    }

    /// Restart semantics: the next process loads the authored library, not the
    /// slots this run edited.
    #[test]
    fn a_fresh_load_returns_the_configured_preset_library() {
        with_temp_config_home(|config_root| {
            let config_dir = config_root.join(crate::config::PRIMARY_CONFIG_DIR);
            fs::create_dir_all(&config_dir).expect("test config directory");
            fs::write(config_dir.join("config.toml"), AUTHORED_PRESET)
                .expect("test config should be written");

            let mut config = Config::load().expect("test config should load").config;
            let _ = apply_preset_action(
                &mut config,
                PresetAction::Save {
                    slot: 1,
                    preset: preset("Run preset"),
                },
            );
            assert_eq!(
                config
                    .presets
                    .get_slot(1)
                    .and_then(|slot| slot.name.clone()),
                Some("Run preset".to_string())
            );

            let restarted = Config::load().expect("test config should reload").config;
            assert_eq!(
                restarted
                    .presets
                    .get_slot(1)
                    .and_then(|slot| slot.name.clone()),
                Some("Authored".to_string())
            );
        });
    }
}
