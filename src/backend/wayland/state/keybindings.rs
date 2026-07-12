use super::WaylandState;
use crate::config::{Config, KeybindingsConfig, action_label};
use crate::input::state::{KeybindingEditOperation, KeybindingEditRequest, UiToastKind};

enum PrepareKeybindingEditError {
    Load(anyhow::Error),
    Edit(String),
}

fn merge_keybinding_edit(
    config: &mut Config,
    request: &KeybindingEditRequest,
) -> Result<(), String> {
    let bindings = match &request.operation {
        KeybindingEditOperation::Replace(bindings) => bindings.clone(),
        KeybindingEditOperation::Delete => Vec::new(),
        KeybindingEditOperation::Reset => KeybindingsConfig::default()
            .bindings_for_action(request.action)
            .map(ToOwned::to_owned)
            .unwrap_or_default(),
    };
    config
        .keybindings
        .set_bindings_for_action(request.action, bindings)
}

fn load_and_merge_keybinding_edit(
    request: &KeybindingEditRequest,
) -> Result<Config, PrepareKeybindingEditError> {
    let mut config = Config::load()
        .map_err(PrepareKeybindingEditError::Load)?
        .config;
    merge_keybinding_edit(&mut config, request).map_err(PrepareKeybindingEditError::Edit)?;
    Ok(config)
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
                self.input_state.set_ui_toast(
                    UiToastKind::Error,
                    "Shortcut not changed because the current config could not be reloaded.",
                );
                return;
            }
            Err(PrepareKeybindingEditError::Edit(err)) => {
                self.input_state.set_ui_toast(UiToastKind::Warning, err);
                return;
            }
        };
        let action_map = match next.keybindings.build_action_map() {
            Ok(map) => map,
            Err(err) => {
                self.input_state
                    .set_ui_toast(UiToastKind::Warning, format!("Shortcut not changed: {err}"));
                return;
            }
        };
        let action_bindings = match next.keybindings.build_action_bindings() {
            Ok(bindings) => bindings,
            Err(err) => {
                self.input_state
                    .set_ui_toast(UiToastKind::Warning, format!("Shortcut not changed: {err}"));
                return;
            }
        };
        if let Err(err) = next.save() {
            log::warn!("Failed to save keybinding edit: {err}");
            self.input_state.set_ui_toast(
                UiToastKind::Error,
                "Shortcut could not be saved (see logs).",
            );
            return;
        }

        self.config = next;
        self.input_state
            .set_keybinding_maps(action_map, action_bindings);
        self.toolbar.mark_dirty();
        self.input_state.set_ui_toast(
            UiToastKind::Info,
            format!("Updated shortcut for {}.", action_label(request.action)),
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
}
