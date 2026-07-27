use super::WaylandState;
use crate::backend::wayland::config_writer::{ConfigMutation, ConfigWriteReceipt};
use crate::config::{Action, Config, KeyBinding, KeybindingsConfig, action_label};
use crate::input::state::{KeybindingEditOperation, KeybindingEditRequest};
use crate::input::state::{Toast, ToastPriority};
use std::collections::HashMap;

/// The shortcut edits this process has accepted but the writer has not yet
/// confirmed, keyed by action.
///
/// Each edit is queued on the background writer, which debounces and retries,
/// so the file on disk can still be one or more edits behind when the next edit
/// reloads it. Replaying these over that snapshot keeps every accepted edit in
/// the keymap the overlay installs, while an action this session never touched
/// still picks up whatever another editor wrote for it.
#[derive(Debug)]
pub(in crate::backend::wayland) struct SessionKeybindingEdits {
    bindings: HashMap<Action, PendingKeybindingEdit>,
    next_receipt: Option<ConfigWriteReceipt>,
}

#[derive(Debug)]
struct PendingKeybindingEdit {
    bindings: Vec<String>,
    receipt: ConfigWriteReceipt,
}

impl Default for SessionKeybindingEdits {
    fn default() -> Self {
        Self {
            bindings: HashMap::new(),
            next_receipt: Some(ConfigWriteReceipt::initial()),
        }
    }
}

impl SessionKeybindingEdits {
    fn next_receipt(&self) -> Option<ConfigWriteReceipt> {
        self.next_receipt
    }

    /// Record an accepted edit by the bindings it produced rather than by the
    /// operation that produced them, so a reset replays as the defaults it
    /// resolved to instead of resetting again against a newer snapshot.
    fn record(
        &mut self,
        action: Action,
        keybindings: &KeybindingsConfig,
        receipt: ConfigWriteReceipt,
    ) {
        self.bindings.insert(
            action,
            PendingKeybindingEdit {
                bindings: keybindings
                    .bindings_for_action(action)
                    .map(<[String]>::to_vec)
                    .unwrap_or_default(),
                receipt,
            },
        );
        self.next_receipt = receipt.successor();
    }

    fn settle(&mut self, receipt: ConfigWriteReceipt) {
        self.bindings.retain(|_, edit| edit.receipt != receipt);
    }

    fn replay(&self, keybindings: &mut KeybindingsConfig) {
        for (action, edit) in &self.bindings {
            // Only actions that already stored bindings are ever recorded, so
            // the unsupported-action error cannot happen here.
            let _ = keybindings.set_bindings_for_action(*action, edit.bindings.clone());
        }
    }
}

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

fn load_keybinding_config() -> Result<Config, PrepareKeybindingEditError> {
    let mut config = Config::load_unvalidated()
        .map_err(PrepareKeybindingEditError::Load)?
        .config;
    config.apply_keybinding_migrations();
    Ok(config)
}

fn merge_loaded_keybinding_edit(
    mut config: Config,
    request: &KeybindingEditRequest,
    session_edits: &SessionKeybindingEdits,
) -> Result<Config, PrepareKeybindingEditError> {
    // Before the new edit, so the conflict check below sees the shortcuts this
    // session already handed to the writer rather than the state they replaced.
    session_edits.replay(&mut config.keybindings);
    merge_keybinding_edit(&mut config, request)?;
    // Check the authored keymap before validation, not after: validation
    // resolves a duplicate shortcut per binding and that resolution is
    // deliberately never written back, so running it first would let this
    // save look clean while the file kept a conflict the user cannot see.
    // Reporting the collision instead keeps the choice theirs.
    config
        .keybindings
        .build_action_map()
        .map_err(PrepareKeybindingEditError::Edit)?;
    config.validate_and_clamp();
    Ok(config)
}

#[cfg(test)]
fn load_and_merge_keybinding_edit(
    request: &KeybindingEditRequest,
    session_edits: &SessionKeybindingEdits,
) -> Result<Config, PrepareKeybindingEditError> {
    merge_loaded_keybinding_edit(load_keybinding_config()?, request, session_edits)
}

/// The writer payload for one edited action: the bindings the merged config
/// ended up storing for it, and nothing else. The background writer reloads the
/// document per batch and the merge records only what its caller changed, so a
/// shortcut edit rewrites exactly its own `[keybindings]` key — the rest of the
/// reloaded snapshot (including anything validation or a migration adjusted in
/// memory) stays as the user authored it.
fn keybinding_mutation(
    config: &Config,
    action: Action,
    receipt: ConfigWriteReceipt,
) -> ConfigMutation {
    ConfigMutation::Keybinding {
        action,
        bindings: config
            .keybindings
            .bindings_for_action(action)
            .map(<[String]>::to_vec)
            .unwrap_or_default(),
        receipt,
    }
}

fn shortcut_conflict_message(binding: &str, existing_action: Action) -> String {
    format!(
        "Shortcut not changed — {binding} is already assigned to {}.",
        action_label(existing_action)
    )
}

impl WaylandState {
    fn settle_completed_keybinding_writes(&mut self) -> usize {
        let receipts = self.config_writer.take_completed_keybinding_writes();
        let count = receipts.len();
        for receipt in receipts {
            self.keybinding_session_edits.settle(receipt);
        }
        count
    }

    /// Load a disk snapshot ordered after every writer completion currently
    /// visible to the event loop. A completion that arrives during a load
    /// discards that snapshot and retries; one that arrives after the final
    /// check remains represented by the pending edit replayed below.
    fn load_keybinding_config_after_completed_writes(
        &mut self,
    ) -> Result<Config, PrepareKeybindingEditError> {
        loop {
            self.settle_completed_keybinding_writes();
            let config = load_keybinding_config()?;
            if self.settle_completed_keybinding_writes() == 0 {
                return Ok(config);
            }
        }
    }

    pub(in crate::backend::wayland) fn handle_keybinding_edit(
        &mut self,
        request: KeybindingEditRequest,
    ) {
        let next = match self
            .load_keybinding_config_after_completed_writes()
            .and_then(|config| {
                merge_loaded_keybinding_edit(config, &request, &self.keybinding_session_edits)
            }) {
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
        let Some(receipt) = self.keybinding_session_edits.next_receipt() else {
            log::error!("Shortcut write receipt sequence exhausted");
            self.input_state.push_toast(
                ToastPriority::Critical,
                "keybindings",
                Toast::error("Shortcut could not be saved (see logs)."),
            );
            return;
        };
        // The write itself moves to the background writer, which debounces and
        // retries; only a queue that cannot accept the edit at all is fatal
        // here, and it leaves the shortcut unchanged exactly as a failed
        // blocking save used to.
        let mutation = keybinding_mutation(&next, request.action, receipt);
        if !self.queue_config_mutation(mutation, "keybinding persistence") {
            self.input_state.push_toast(
                ToastPriority::Critical,
                "keybindings",
                Toast::error("Shortcut could not be saved (see logs)."),
            );
            return;
        }

        // Adopt the reloaded keymap so the in-memory config keeps matching the
        // maps installed below, including bindings another editor changed since
        // startup. Nothing else from the reload is installed: the running app
        // owns the rest of its config, and no runtime-UI seed reads
        // `[keybindings]`, so this needs no seed refresh.
        self.keybinding_session_edits
            .record(request.action, &next.keybindings, receipt);
        self.config.keybindings = next.keybindings;
        self.input_state
            .set_keybinding_maps(action_map, action_bindings);
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
    use crate::backend::wayland::config_writer::persist_mutations_to_path;
    use crate::config::{Action, RuntimeConfigBackup};
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

            let merged = load_and_merge_keybinding_edit(
                &KeybindingEditRequest {
                    action: Action::SelectPenTool,
                    operation: KeybindingEditOperation::Replace(vec![
                        "Ctrl+Alt+Shift+K".to_string(),
                    ]),
                },
                &SessionKeybindingEdits::default(),
            )
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
            let config_path = config_dir.join("config.toml");
            fs::write(
                &config_path,
                "config_revision = 1\n\n[keybindings]\nclear_canvas = ['F']\nselect_pen_tool = ['F']\nundo = ['Ctrl+Alt+U']\n",
            )
            .unwrap();

            let repaired = load_and_merge_keybinding_edit(
                &KeybindingEditRequest {
                    action: Action::ClearCanvas,
                    operation: KeybindingEditOperation::Replace(vec!["Ctrl+L".to_string()]),
                },
                &SessionKeybindingEdits::default(),
            )
            .expect("disk-backed repair should succeed");

            persist_mutations_to_path(
                &config_path,
                &[keybinding_mutation(
                    &repaired,
                    Action::ClearCanvas,
                    ConfigWriteReceipt::initial(),
                )],
                &mut RuntimeConfigBackup::with_directory(config_root.join("config-backups")),
            )
            .expect("the repaired shortcut should persist");
            let reloaded = Config::load()
                .expect("repaired config should reload")
                .config;

            assert_eq!(
                reloaded
                    .keybindings
                    .bindings_for_action(Action::ClearCanvas),
                Some(&["Ctrl+L".to_string()][..])
            );
            // The conflicting partner and an unrelated binding are not part of
            // the edit, so the repair leaves both exactly as authored.
            assert_eq!(
                reloaded
                    .keybindings
                    .bindings_for_action(Action::SelectPenTool),
                Some(&["F".to_string()][..])
            );
            assert_eq!(
                reloaded.keybindings.bindings_for_action(Action::Undo),
                Some(&["Ctrl+Alt+U".to_string()][..])
            );
        });
    }

    /// The queued payload carries the edited action's merged bindings and
    /// nothing else, so a shortcut edit cannot smuggle an unrelated field of
    /// the reloaded snapshot into the file.
    #[test]
    fn the_queued_payload_is_scoped_to_the_edited_action() {
        let mut config = Config::default();
        config
            .keybindings
            .set_bindings_for_action(Action::ClearCanvas, vec!["Ctrl+L".to_string()])
            .unwrap();

        match keybinding_mutation(&config, Action::ClearCanvas, ConfigWriteReceipt::initial()) {
            ConfigMutation::Keybinding {
                action, bindings, ..
            } => {
                assert_eq!(action, Action::ClearCanvas);
                assert_eq!(bindings, vec!["Ctrl+L".to_string()]);
            }
            other => panic!("expected a keybinding mutation, got {other:?}"),
        }
    }

    /// Deleting every binding is a real edit: the payload has to carry the
    /// empty list so the writer clears the key instead of leaving it alone.
    #[test]
    fn deleting_a_shortcut_queues_an_empty_binding_list() {
        let mut config = Config::default();
        merge_keybinding_edit(
            &mut config,
            &KeybindingEditRequest {
                action: Action::ClearCanvas,
                operation: KeybindingEditOperation::Delete,
            },
        )
        .expect("clearing a shortcut should merge");

        match keybinding_mutation(&config, Action::ClearCanvas, ConfigWriteReceipt::initial()) {
            ConfigMutation::Keybinding {
                action, bindings, ..
            } => {
                assert_eq!(action, Action::ClearCanvas);
                assert!(bindings.is_empty());
            }
            other => panic!("expected a keybinding mutation, got {other:?}"),
        }
    }

    /// The overlay's accept path without a `WaylandState`: merge the reloaded
    /// snapshot with this session's still-queued edits replayed over it, then
    /// record the result the way `handle_keybinding_edit` does once the writer
    /// has taken the mutation. Nothing here flushes the writer, so `config.toml`
    /// keeps the contents the test wrote.
    fn accept_edit(
        session_edits: &mut SessionKeybindingEdits,
        action: Action,
        operation: KeybindingEditOperation,
    ) -> Config {
        let request = KeybindingEditRequest { action, operation };
        let next = load_and_merge_keybinding_edit(&request, session_edits)
            .unwrap_or_else(|error| panic!("edit for {action:?} should merge: {error:?}"));
        let receipt = session_edits
            .next_receipt()
            .expect("the test cannot exhaust the shortcut receipt sequence");
        session_edits.record(action, &next.keybindings, receipt);
        next
    }

    fn write_config_with_keybindings(config_root: &std::path::Path, keybindings: &str) {
        let config_dir = config_root.join(crate::config::PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            config_dir.join("config.toml"),
            format!(
                "config_revision = {}\n\n[keybindings]\n{keybindings}",
                crate::config::CURRENT_CONFIG_REVISION
            ),
        )
        .unwrap();
    }

    /// Two edits in a row, with the debounced writer never flushing in between:
    /// the second edit reloads a file that still lacks the first, so only the
    /// session replay keeps that shortcut in the keymap the overlay installs.
    #[test]
    fn a_second_edit_keeps_one_the_writer_has_not_flushed_yet() {
        crate::config::test_helpers::with_temp_config_home(|config_root| {
            write_config_with_keybindings(config_root, "undo = ['Ctrl+Alt+U']\n");
            let mut session_edits = SessionKeybindingEdits::default();

            accept_edit(
                &mut session_edits,
                Action::ClearCanvas,
                KeybindingEditOperation::Replace(vec!["Ctrl+Alt+Shift+K".to_string()]),
            );
            let second = accept_edit(
                &mut session_edits,
                Action::SelectPenTool,
                KeybindingEditOperation::Replace(vec!["Ctrl+Alt+Shift+J".to_string()]),
            );

            assert_eq!(
                second.keybindings.bindings_for_action(Action::ClearCanvas),
                Some(&["Ctrl+Alt+Shift+K".to_string()][..])
            );
            assert_eq!(
                second
                    .keybindings
                    .bindings_for_action(Action::SelectPenTool),
                Some(&["Ctrl+Alt+Shift+J".to_string()][..])
            );
            let map = second
                .keybindings
                .build_action_map()
                .expect("the merged keymap should be valid");
            assert_eq!(
                map.get(&KeyBinding::parse("Ctrl+Alt+Shift+K").unwrap()),
                Some(&Action::ClearCanvas)
            );
            // An action this session never edited still comes from the file, so
            // another editor's change is adopted exactly as before.
            assert_eq!(
                second.keybindings.bindings_for_action(Action::Undo),
                Some(&["Ctrl+Alt+U".to_string()][..])
            );
        });
    }

    /// A reset replays as the bindings it produced. Replaying the operation
    /// instead would re-derive defaults against a newer snapshot, and replaying
    /// the pre-reset edit would undo the reset the user just asked for.
    #[test]
    fn a_queued_reset_replays_as_the_defaults_it_resolved_to() {
        crate::config::test_helpers::with_temp_config_home(|config_root| {
            write_config_with_keybindings(config_root, "clear_canvas = ['Ctrl+Alt+Shift+9']\n");
            let mut session_edits = SessionKeybindingEdits::default();
            let defaults = KeybindingsConfig::default()
                .bindings_for_action(Action::ClearCanvas)
                .expect("clear canvas is configurable")
                .to_vec();

            accept_edit(
                &mut session_edits,
                Action::ClearCanvas,
                KeybindingEditOperation::Replace(vec!["Ctrl+Alt+Shift+K".to_string()]),
            );
            accept_edit(
                &mut session_edits,
                Action::ClearCanvas,
                KeybindingEditOperation::Reset,
            );
            let third = accept_edit(
                &mut session_edits,
                Action::SelectPenTool,
                KeybindingEditOperation::Replace(vec!["Ctrl+Alt+Shift+J".to_string()]),
            );

            assert_eq!(
                third.keybindings.bindings_for_action(Action::ClearCanvas),
                Some(defaults.as_slice())
            );
        });
    }

    /// The replay happens before the merge, so the conflict check compares the
    /// requested shortcut against what this session already assigned rather
    /// than against the stale file.
    #[test]
    fn a_queued_edit_is_visible_to_the_next_conflict_check() {
        crate::config::test_helpers::with_temp_config_home(|config_root| {
            write_config_with_keybindings(config_root, "undo = ['Ctrl+Alt+U']\n");
            let mut session_edits = SessionKeybindingEdits::default();
            accept_edit(
                &mut session_edits,
                Action::ClearCanvas,
                KeybindingEditOperation::Replace(vec!["Ctrl+Alt+Shift+K".to_string()]),
            );

            let error = load_and_merge_keybinding_edit(
                &KeybindingEditRequest {
                    action: Action::SelectPenTool,
                    operation: KeybindingEditOperation::Replace(vec![
                        "Ctrl+Alt+Shift+K".to_string(),
                    ]),
                },
                &session_edits,
            )
            .expect_err("the queued shortcut should still be taken");

            match error {
                PrepareKeybindingEditError::Conflict {
                    binding,
                    existing_action,
                } => {
                    assert_eq!(binding, "Ctrl+Alt+Shift+K");
                    assert_eq!(existing_action, Action::ClearCanvas);
                }
                other => panic!("expected a conflict with the queued edit, got {other:?}"),
            }
        });
    }

    /// Once the writer confirms an edit, replay no longer owns that action.
    /// A later edit therefore adopts an external change made after the flush
    /// instead of restoring the session's older value in the live keymap.
    #[test]
    fn a_completed_edit_stops_overriding_later_disk_changes() {
        crate::config::test_helpers::with_temp_config_home(|config_root| {
            write_config_with_keybindings(
                config_root,
                "clear_canvas = ['Ctrl+Alt+Shift+1']\nundo = ['Ctrl+Alt+U']\n",
            );
            let mut session_edits = SessionKeybindingEdits::default();

            accept_edit(
                &mut session_edits,
                Action::ClearCanvas,
                KeybindingEditOperation::Replace(vec!["Ctrl+Alt+Shift+K".to_string()]),
            );
            session_edits.settle(ConfigWriteReceipt::initial());
            write_config_with_keybindings(
                config_root,
                "clear_canvas = ['Ctrl+Alt+Shift+E']\nundo = ['Ctrl+Alt+U']\n",
            );

            let next = accept_edit(
                &mut session_edits,
                Action::SelectPenTool,
                KeybindingEditOperation::Replace(vec!["Ctrl+Alt+Shift+J".to_string()]),
            );

            assert_eq!(
                next.keybindings.bindings_for_action(Action::ClearCanvas),
                Some(&["Ctrl+Alt+Shift+E".to_string()][..])
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

            let error = load_and_merge_keybinding_edit(
                &KeybindingEditRequest {
                    action: Action::Redo,
                    operation: KeybindingEditOperation::Replace(vec!["Ctrl+Alt+R".to_string()]),
                },
                &SessionKeybindingEdits::default(),
            )
            .expect_err("an unrelated edit must not conceal the existing conflict");

            assert!(matches!(error, PrepareKeybindingEditError::Edit(_)));
            assert_eq!(fs::read_to_string(config_path).unwrap(), original);
        });
    }
}
