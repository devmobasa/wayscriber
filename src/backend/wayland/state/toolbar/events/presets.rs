use super::*;
use crate::backend::wayland::config_edits::{ConfigEdit, ConfigEditWorker};
use crate::config::{Config, ConfigEditNotReadBack, ConfigEditOutcome, ConfigEditWrite};
use crate::input::state::{PresetAction, Toast, ToastPriority};

/// Apply an accepted preset save/clear to the effective config.
///
/// `InputState` already holds the live slots; this keeps the effective `Config`
/// beside them so anything reading `config.presets` this session sees the same
/// library. It happens as the gesture completes, because the live slot is the
/// feedback the gesture is for; the durable half is queued and only the wording
/// waits for it.
fn apply_preset_action(config: &mut Config, action: &PresetAction) {
    match action {
        PresetAction::Save { slot, preset } => {
            config.presets.set_slot(*slot, Some((**preset).clone()));
        }
        PresetAction::Clear { slot } => config.presets.set_slot(*slot, None),
    }
}

/// What the user is told once the file has taken the slot.
fn preset_saved_message(action: &PresetAction) -> String {
    match action {
        PresetAction::Save { slot, .. } => format!("Saved preset {slot}."),
        PresetAction::Clear { slot } => format!("Cleared preset {slot}."),
    }
}

/// What the user is told when the file already held exactly this slot.
///
/// Re-saving a slot with the settings it already has, or clearing one the file
/// never spelled out, has no delta to write — so the message says the file
/// already agrees rather than claiming a save that did not happen.
fn preset_unchanged_message(action: &PresetAction) -> String {
    match action {
        PresetAction::Save { slot, .. } => format!("Preset {slot} already holds these settings."),
        PresetAction::Clear { slot } => format!("Preset {slot} was already empty in config.toml."),
    }
}

/// What the user is told when the slot changed but the file did not.
fn preset_save_failed_message(action: &PresetAction) -> String {
    let (slot, verb) = preset_slot_and_verb(action);
    format!("Preset {slot} {verb} for this run, but saving to config.toml failed (see logs).")
}

/// What the user is told when the write landed but the slot does not read back.
///
/// The file changed, so this cannot borrow the wording above: a value the save
/// clamped is fixed in the configurator, not by retrying the gesture.
fn preset_write_unverified_message(action: &PresetAction) -> String {
    let (slot, verb) = preset_slot_and_verb(action);
    format!(
        "Preset {slot} {verb} for this run, but config.toml was written and does not read back \
         with it (see logs)."
    )
}

fn preset_slot_and_verb(action: &PresetAction) -> (usize, &'static str) {
    match action {
        PresetAction::Save { slot, .. } => (*slot, "saved"),
        PresetAction::Clear { slot } => (*slot, "cleared"),
    }
}

/// Apply a preset gesture to the run and queue its durable half.
///
/// Over the fields it touches rather than over the whole state, because
/// teardown queues the same gesture from a path that cannot lend out all of
/// `WaylandState` at once (see `config_edits::finish_config_edits`).
pub(in crate::backend::wayland) fn queue_preset_action(
    config: &mut Config,
    worker: &mut ConfigEditWorker,
    action: PresetAction,
) {
    apply_preset_action(config, &action);
    // The live slot already changed. The save decides which message the user
    // gets, never whether the preset works this run — so it is queued for the
    // config-edit worker and the toast waits for its answer rather than
    // claiming a durable change the file has not taken yet.
    worker.submit(ConfigEdit::Preset(action));
}

impl WaylandState {
    pub(in crate::backend::wayland) fn handle_preset_action(&mut self, action: PresetAction) {
        queue_preset_action(
            &mut self.config,
            self.preferences.config_edits_mut(),
            action,
        );
    }

    pub(in crate::backend::wayland) fn finish_preset_action(
        &mut self,
        action: &PresetAction,
        result: Result<ConfigEditOutcome, anyhow::Error>,
    ) {
        match result {
            Ok(outcome) => {
                if let Some(backup) = outcome.backup_path {
                    log::info!("Backed up config to {} before the write", backup.display());
                }
                let message = match outcome.write {
                    ConfigEditWrite::Wrote => preset_saved_message(action),
                    ConfigEditWrite::AlreadyCurrent => preset_unchanged_message(action),
                };
                self.input_state
                    .push_toast(ToastPriority::Info, "presets", Toast::info(message));
            }
            Err(error) => {
                let slot = preset_slot_and_verb(action).0;
                log::warn!("Failed to save preset slot {slot}: {error:#}");
                let message = match error.downcast_ref::<ConfigEditNotReadBack>() {
                    Some(_) => preset_write_unverified_message(action),
                    None => preset_save_failed_message(action),
                };
                self.input_state.push_toast(
                    ToastPriority::Action,
                    "presets",
                    Toast::warning(message)
                        .action("Edit", crate::domain::Action::OpenConfiguratorPresets),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::io::persist_preset_slot_at;
    use crate::config::persist_preset_slot;
    use crate::config::test_helpers::with_temp_config_home;
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
    fn saving_a_preset_updates_the_effective_config_and_reports_a_durable_change() {
        let mut config = Config::default();

        let action = PresetAction::Save {
            slot: 2,
            preset: preset("Run preset"),
        };
        apply_preset_action(&mut config, &action);

        assert_eq!(
            config
                .presets
                .get_slot(2)
                .and_then(|slot| slot.name.clone()),
            Some("Run preset".to_string())
        );
        // The wording waits for the write; this is what the completion says
        // once the file has the slot.
        assert_eq!(preset_saved_message(&action), "Saved preset 2.");
    }

    #[test]
    fn clearing_a_preset_empties_the_effective_slot_and_reports_a_durable_change() {
        let mut config = Config::default();
        apply_preset_action(
            &mut config,
            &PresetAction::Save {
                slot: 1,
                preset: preset("Run preset"),
            },
        );

        let action = PresetAction::Clear { slot: 1 };
        apply_preset_action(&mut config, &action);

        assert!(config.presets.get_slot(1).is_none());
        assert_eq!(preset_saved_message(&action), "Cleared preset 1.");
    }

    /// The persistence fixture: hand-authored text a slot write must respect.
    /// It carries comments, an unrelated pair of actions contesting one chord,
    /// and a setting from some later release, so a write that leaked the
    /// loader's in-memory repairs would show up in the diff.
    const AUTHORED_FILE: &str = "\
# Wayscriber configuration. These comments must survive a preset edit.

[ui]
setting_from_a_later_release = 7

[keybindings]
# A contested pair the loader resolves for the session and never repairs.
undo = [\"Ctrl+Alt+Shift+Q\"]
redo = [\"Ctrl+Alt+Shift+Q\"]

# The slot under test.
[presets.slot_1]
name = \'Authored\'
tool = \'pen\'
color = \'#112233\'
size = 3.0
";

    /// Everything in the fixture that is not the edited slot's own block.
    ///
    /// The slot table sits last on purpose: a comment above a table header is
    /// that table's prefix in the TOML tree, so clearing the slot takes the
    /// "# The slot under test." line with it. That is how removing any table
    /// behaves, the configurator's Save included — it is not extra reach.
    const WITHOUT_SLOT_BLOCK: &str = "\
# Wayscriber configuration. These comments must survive a preset edit.

[ui]
setting_from_a_later_release = 7

[keybindings]
# A contested pair the loader resolves for the session and never repairs.
undo = [\"Ctrl+Alt+Shift+Q\"]
redo = [\"Ctrl+Alt+Shift+Q\"]
";

    fn write_fixture(dir: &std::path::Path) -> std::path::PathBuf {
        let path = dir.join("config.toml");
        fs::write(&path, AUTHORED_FILE).expect("the fixture should be written");
        path
    }

    /// A preset save rewrites its own slot table and leaves every other byte —
    /// comments, the contested pair, the unknown key — exactly as authored.
    #[test]
    fn saving_a_preset_changes_exactly_its_own_slot() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = write_fixture(temp.path());

        persist_preset_slot_at(&path, 1, Some(&preset("Run preset")))
            .expect("the write should succeed");

        let after = fs::read_to_string(&path).expect("readable");
        assert!(after.contains("name = \"Run preset\""), "{after}");
        for untouched in [
            "# Wayscriber configuration. These comments must survive a preset edit.",
            "undo = [\"Ctrl+Alt+Shift+Q\"]",
            "redo = [\"Ctrl+Alt+Shift+Q\"]",
            "setting_from_a_later_release = 7",
        ] {
            assert!(after.contains(untouched), "lost {untouched:?} from {after}");
        }
        assert!(
            !after.contains("Authored"),
            "the edited slot should hold the new name only: {after}"
        );
    }

    /// Clearing removes the slot's own block and leaves the rest byte-identical.
    #[test]
    fn clearing_a_preset_changes_exactly_its_own_slot() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = write_fixture(temp.path());

        persist_preset_slot_at(&path, 1, None).expect("the write should succeed");

        assert_eq!(
            fs::read_to_string(&path).expect("readable"),
            WITHOUT_SLOT_BLOCK
        );
    }

    /// Saving a slot the settings it already holds, or clearing one the file
    /// never had, has no delta to write — so nothing is written and the message
    /// says so instead of claiming a save.
    #[test]
    fn a_preset_edit_the_file_already_holds_writes_nothing() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = write_fixture(temp.path());
        let stored = crate::config::ConfigDocument::load_from_path(&path)
            .expect("load")
            .config()
            .presets
            .get_slot(1)
            .cloned()
            .expect("the fixture authors slot 1");

        let resaved =
            persist_preset_slot_at(&path, 1, Some(&stored)).expect("a no-op is not a failure");
        let cleared = persist_preset_slot_at(&path, 4, None).expect("a no-op is not a failure");

        assert_eq!(resaved.write, ConfigEditWrite::AlreadyCurrent);
        assert!(resaved.backup_path.is_none(), "no write, no backup");
        assert_eq!(cleared.write, ConfigEditWrite::AlreadyCurrent);
        assert_eq!(fs::read_to_string(&path).expect("readable"), AUTHORED_FILE);
        assert_eq!(
            preset_unchanged_message(&PresetAction::Save {
                slot: 1,
                preset: preset("Ignored"),
            }),
            "Preset 1 already holds these settings."
        );
        assert_eq!(
            preset_unchanged_message(&PresetAction::Clear { slot: 4 }),
            "Preset 4 was already empty in config.toml."
        );
    }

    #[test]
    fn a_preset_write_leaves_a_timestamped_backup() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = write_fixture(temp.path());

        let outcome = persist_preset_slot_at(&path, 2, Some(&preset("Run preset")))
            .expect("the write should succeed");

        let backup = outcome.backup_path.expect("an existing file is backed up");
        assert_eq!(
            fs::read_to_string(&backup).expect("the backup should be readable"),
            AUTHORED_FILE
        );
    }

    #[test]
    fn a_read_only_config_fails_the_preset_write_and_leaves_the_file_alone() {
        use std::os::unix::fs::PermissionsExt;

        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = write_fixture(temp.path());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).expect("chmod file");
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o555)).expect("chmod dir");

        let result = persist_preset_slot_at(&path, 1, Some(&preset("Run preset")));

        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o755)).expect("restore dir");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("restore file");

        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        assert!(result.is_err(), "a read-only config must fail the write");
        assert_eq!(fs::read_to_string(&path).expect("readable"), AUTHORED_FILE);
        assert_eq!(
            preset_save_failed_message(&PresetAction::Save {
                slot: 1,
                preset: preset("Run preset")
            }),
            "Preset 1 saved for this run, but saving to config.toml failed (see logs)."
        );
        assert_eq!(
            preset_save_failed_message(&PresetAction::Clear { slot: 3 }),
            "Preset 3 cleared for this run, but saving to config.toml failed (see logs)."
        );
    }

    /// A slot outside the configurable range is refused rather than silently
    /// dropped by `set_slot`.
    #[test]
    fn an_out_of_range_preset_slot_is_refused() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = write_fixture(temp.path());

        assert!(persist_preset_slot_at(&path, 0, Some(&preset("Nope"))).is_err());
        assert!(persist_preset_slot_at(&path, 99, Some(&preset("Nope"))).is_err());
        assert_eq!(fs::read_to_string(&path).expect("readable"), AUTHORED_FILE);
    }

    /// Restart semantics, the durable way round: what this run saved is what
    /// the next process loads.
    #[test]
    fn a_fresh_load_returns_the_written_preset() {
        with_temp_config_home(|config_root| {
            let config_dir = config_root.join(crate::config::PRIMARY_CONFIG_DIR);
            fs::create_dir_all(&config_dir).expect("test config directory");
            fs::write(config_dir.join("config.toml"), AUTHORED_PRESET)
                .expect("test config should be written");

            persist_preset_slot(1, Some(&preset("Run preset"))).expect("the write should succeed");

            let restarted = Config::load().expect("test config should reload").config;
            assert_eq!(
                restarted
                    .presets
                    .get_slot(1)
                    .and_then(|slot| slot.name.clone()),
                Some("Run preset".to_string())
            );
        });
    }
}
