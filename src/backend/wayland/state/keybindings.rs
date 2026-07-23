use super::WaylandState;
use crate::config::{Action, Config, KeyBinding, KeybindingsConfig, action_label};
use crate::input::state::{KeybindingEditOperation, KeybindingEditRequest};
use crate::input::state::{Toast, ToastPriority};

#[derive(Debug)]
enum PrepareKeybindingEditError {
    Load(anyhow::Error),
    Edit(String),
    Conflict {
        binding: String,
        existing_action: Action,
    },
}

fn merge_keybinding_edit(
    config: &mut Config,
    request: &KeybindingEditRequest,
) -> Result<(), PrepareKeybindingEditError> {
    let bindings = match &request.operation {
        KeybindingEditOperation::Replace(bindings) => bindings.clone(),
        KeybindingEditOperation::Delete => Vec::new(),
        KeybindingEditOperation::Reset => KeybindingsConfig::default()
            .bindings_for_action(request.action)
            .map(ToOwned::to_owned)
            .unwrap_or_default(),
    };

    // Build the conflict lookup without the action being edited. Its current
    // bindings may be the invalid part the user is trying to replace/delete.
    let mut other_bindings = config.keybindings.clone();
    other_bindings
        .set_bindings_for_action(request.action, Vec::new())
        .map_err(PrepareKeybindingEditError::Edit)?;
    let current_map = other_bindings
        .build_action_map()
        .map_err(PrepareKeybindingEditError::Edit)?;
    for binding_text in &bindings {
        let binding = KeyBinding::parse(binding_text).map_err(PrepareKeybindingEditError::Edit)?;
        if let Some(existing_action) = current_map.get(&binding)
            && *existing_action != request.action
        {
            return Err(PrepareKeybindingEditError::Conflict {
                binding: binding_text.clone(),
                existing_action: *existing_action,
            });
        }
    }

    config
        .keybindings
        .set_bindings_for_action(request.action, bindings)
        .map_err(PrepareKeybindingEditError::Edit)
}

fn load_and_merge_keybinding_edit(
    request: &KeybindingEditRequest,
) -> Result<Config, PrepareKeybindingEditError> {
    let mut config = Config::load_unvalidated()
        .map_err(PrepareKeybindingEditError::Load)?
        .config;
    config.apply_keybinding_migrations();
    merge_keybinding_edit(&mut config, request)?;
    // Refuse to run normal validation while the repaired keymap is still
    // invalid: validate_keybindings intentionally falls back to defaults.
    config
        .keybindings
        .build_action_map()
        .map_err(PrepareKeybindingEditError::Edit)?;
    config.validate_and_clamp();
    Ok(config)
}

fn shortcut_conflict_message(binding: &str, existing_action: Action) -> String {
    format!(
        "Shortcut not changed — {binding} is already assigned to {}.",
        action_label(existing_action)
    )
}

impl WaylandState {
    pub(in crate::backend::wayland) fn handle_keybinding_edit(
        &mut self,
        request: KeybindingEditRequest,
    ) {
        let next = match load_and_merge_keybinding_edit(&request) {
            Ok(config) => config,
            Err(PrepareKeybindingEditError::Load(err)) => {
                log::warn!("Failed to reload config before keybinding edit: {err}");
                self.input_state.push_toast(
                    ToastPriority::Critical,
                    "keybindings",
                    Toast::error(
                        "Shortcut not changed because the current config could not be reloaded.",
                    ),
                );
                return;
            }
            Err(PrepareKeybindingEditError::Edit(err)) => {
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "keybindings",
                    Toast::warning(err),
                );
                return;
            }
            Err(PrepareKeybindingEditError::Conflict {
                binding,
                existing_action,
            }) => {
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "keybindings",
                    Toast::warning(shortcut_conflict_message(&binding, existing_action)),
                );
                return;
            }
        };
        let action_map = match next.keybindings.build_action_map() {
            Ok(map) => map,
            Err(err) => {
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "keybindings",
                    Toast::warning(format!("Shortcut not changed: {err}")),
                );
                return;
            }
        };
        let action_bindings = match next.keybindings.build_action_bindings() {
            Ok(bindings) => bindings,
            Err(err) => {
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "keybindings",
                    Toast::warning(format!("Shortcut not changed: {err}")),
                );
                return;
            }
        };
        if let Err(err) = next.save() {
            log::warn!("Failed to save keybinding edit: {err}");
            self.input_state.push_toast(
                ToastPriority::Critical,
                "keybindings",
                Toast::error("Shortcut could not be saved (see logs)."),
            );
            return;
        }

        self.config = next;
        self.input_state
            .set_keybinding_maps(action_map, action_bindings);
        // The edit is merged into a fresh read of the complete authored
        // config. Reconcile runtime-state seeds with that installed snapshot
        // so permits and active previews cannot retain the pre-reload values.
        self.refresh_runtime_ui_config_seeds();
        self.toolbar.mark_dirty();
        self.input_state.push_toast(
            ToastPriority::Info,
            "keybindings",
            Toast::info(format!(
                "Updated shortcut for {}.",
                action_label(request.action)
            )),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Action;
    use std::fs;

    #[test]
    fn editing_a_reloaded_config_preserves_unrelated_external_changes() {
        crate::config::test_helpers::with_temp_config_home(|config_root| {
            let config_dir = config_root.join(crate::config::PRIMARY_CONFIG_DIR);
            fs::create_dir_all(&config_dir).unwrap();
            fs::write(
                config_dir.join("config.toml"),
                "[ui]\nshow_status_bar = false\n\n[capture]\nfilename_template = 'externally-changed-{timestamp}'\n",
            )
            .unwrap();

            let merged = load_and_merge_keybinding_edit(&KeybindingEditRequest {
                action: Action::SelectPenTool,
                operation: KeybindingEditOperation::Replace(vec!["Ctrl+Alt+Shift+K".to_string()]),
            })
            .unwrap_or_else(|_| panic!("reload and merge should succeed"));

            assert!(!merged.ui.show_status_bar);
            assert_eq!(
                merged.capture.filename_template,
                "externally-changed-{timestamp}"
            );
            assert_eq!(
                merged
                    .keybindings
                    .bindings_for_action(Action::SelectPenTool),
                Some(&["Ctrl+Alt+Shift+K".to_string()][..])
            );
        });
    }

    #[test]
    fn duplicate_shortcut_reports_the_existing_action() {
        let mut config = Config::default();
        let error = merge_keybinding_edit(
            &mut config,
            &KeybindingEditRequest {
                action: Action::ClearCanvas,
                operation: KeybindingEditOperation::Replace(vec!["F".to_string()]),
            },
        )
        .expect_err("pen shortcut should conflict");

        match error {
            PrepareKeybindingEditError::Conflict {
                binding,
                existing_action,
            } => {
                assert_eq!(binding, "F");
                assert_eq!(existing_action, Action::SelectPenTool);
                assert_eq!(
                    shortcut_conflict_message(&binding, existing_action),
                    "Shortcut not changed — F is already assigned to Pen Tool."
                );
            }
            _ => panic!("expected a structured shortcut conflict"),
        }
    }

    #[test]
    fn replacing_an_invalid_actions_binding_can_repair_the_keymap() {
        let mut config = Config::default();
        config.keybindings.core.clear_canvas = vec!["F".to_string()];
        assert!(config.keybindings.build_action_map().is_err());

        merge_keybinding_edit(
            &mut config,
            &KeybindingEditRequest {
                action: Action::ClearCanvas,
                operation: KeybindingEditOperation::Replace(vec!["Ctrl+L".to_string()]),
            },
        )
        .expect("replacing the offending binding should repair the keymap");

        assert!(config.keybindings.build_action_map().is_ok());
        assert_eq!(
            config.keybindings.bindings_for_action(Action::ClearCanvas),
            Some(&["Ctrl+L".to_string()][..])
        );
    }

    #[test]
    fn repairing_an_invalid_disk_keymap_preserves_unrelated_shortcuts() {
        crate::config::test_helpers::with_temp_config_home(|config_root| {
            let config_dir = config_root.join(crate::config::PRIMARY_CONFIG_DIR);
            fs::create_dir_all(&config_dir).unwrap();
            fs::write(
                config_dir.join("config.toml"),
                "config_revision = 1\n\n[keybindings]\nclear_canvas = ['F']\nselect_pen_tool = ['F']\nundo = ['Ctrl+Alt+U']\n",
            )
            .unwrap();

            let repaired = load_and_merge_keybinding_edit(&KeybindingEditRequest {
                action: Action::ClearCanvas,
                operation: KeybindingEditOperation::Replace(vec!["Ctrl+L".to_string()]),
            })
            .expect("disk-backed repair should succeed");

            repaired.save().expect("repaired config should save");
            let reloaded = Config::load()
                .expect("repaired config should reload")
                .config;

            assert_eq!(
                reloaded
                    .keybindings
                    .bindings_for_action(Action::ClearCanvas),
                Some(&["Ctrl+L".to_string()][..])
            );
            assert_eq!(
                reloaded.keybindings.bindings_for_action(Action::Undo),
                Some(&["Ctrl+Alt+U".to_string()][..])
            );
        });
    }

    #[test]
    fn unrelated_edit_cannot_overwrite_an_invalid_disk_keymap() {
        crate::config::test_helpers::with_temp_config_home(|config_root| {
            let config_dir = config_root.join(crate::config::PRIMARY_CONFIG_DIR);
            fs::create_dir_all(&config_dir).unwrap();
            let config_path = config_dir.join("config.toml");
            let original = "config_revision = 1\n\n[keybindings]\nclear_canvas = ['F']\nselect_pen_tool = ['F']\nundo = ['Ctrl+Alt+U']\n";
            fs::write(&config_path, original).unwrap();

            let error = load_and_merge_keybinding_edit(&KeybindingEditRequest {
                action: Action::Redo,
                operation: KeybindingEditOperation::Replace(vec!["Ctrl+Alt+R".to_string()]),
            })
            .expect_err("an unrelated edit must not conceal the existing conflict");

            assert!(matches!(error, PrepareKeybindingEditError::Edit(_)));
            assert_eq!(fs::read_to_string(config_path).unwrap(), original);
        });
    }
}
