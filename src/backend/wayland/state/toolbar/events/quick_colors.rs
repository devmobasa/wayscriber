use super::*;
use crate::backend::wayland::config_edits::{ConfigEdit, ConfigEditWorker};
use crate::config::{
    Config, ConfigEditNotReadBack, ConfigEditOutcome, ConfigEditWrite, QuickColorSlotMissing,
};
use crate::input::state::{QuickColorEdit, Toast, ToastPriority};

/// What the user is told when the swatch and the file both took the recolor.
fn quick_color_saved_message(index: usize) -> String {
    format!("Updated quick color {}.", index + 1)
}

/// What the user is told when the slot already painted this color.
///
/// Reachable from the picker's own controls — accepting the color it opened
/// with, or "Default" on a slot that never moved — and there is no delta to
/// write, so the message must not claim one.
fn quick_color_unchanged_message(index: usize) -> String {
    format!("Quick color {} already uses that color.", index + 1)
}

/// What the user is told when the swatch changed but the file did not.
fn quick_color_save_failed_message(index: usize) -> String {
    format!(
        "Quick color {} changed for this run, but saving to config.toml failed (see logs).",
        index + 1
    )
}

/// What the user is told when the write landed but the slot does not read back.
/// The file changed, so this cannot borrow the wording above.
fn quick_color_write_unverified_message(index: usize) -> String {
    format!(
        "Quick color {} changed for this run, but config.toml was written and does not read back \
         with it (see logs).",
        index + 1
    )
}

/// What the user is told when there is no longer a slot to write to.
///
/// The palette shrank while the picker was open — the configurator or a hand
/// edit removed entries — so the recolor has nowhere durable to go. The live
/// swatch keeps it for the run, and the message says which slot vanished
/// rather than reporting a save failure the user cannot act on.
fn quick_color_slot_missing_message(index: usize) -> String {
    format!(
        "Quick color {} is no longer in config.toml, so the new color applies to this run only.",
        index + 1
    )
}

/// Apply an accepted recolor to the run and queue its durable half.
///
/// Over the fields it touches, for the reason given on `queue_preset_action`.
pub(in crate::backend::wayland) fn queue_quick_color_edit(
    config: &mut Config,
    worker: &mut ConfigEditWorker,
    edit: QuickColorEdit,
) {
    let QuickColorEdit { index, color } = edit;
    // Keep the effective config beside the live palette, so anything reading
    // `config.drawing.quick_colors` this session agrees with the swatch the
    // user is looking at. The write is queued for the config-edit worker; the
    // toast waits for it, because the swatch is already visibly changed and
    // only the claim about the file is at stake.
    let _ = config.drawing.quick_colors.set_color_at(index, color);
    worker.submit(ConfigEdit::QuickColor(edit));
}

impl WaylandState {
    pub(in crate::backend::wayland) fn handle_quick_color_edit(&mut self, edit: QuickColorEdit) {
        queue_quick_color_edit(&mut self.config, &mut self.config_edits, edit);
    }

    pub(in crate::backend::wayland) fn finish_quick_color_edit(
        &mut self,
        edit: QuickColorEdit,
        result: Result<ConfigEditOutcome, anyhow::Error>,
    ) {
        let index = edit.index;
        match result {
            Ok(outcome) => {
                if let Some(backup) = outcome.backup_path {
                    log::info!("Backed up config to {} before the write", backup.display());
                }
                let message = match outcome.write {
                    ConfigEditWrite::Wrote => quick_color_saved_message(index),
                    ConfigEditWrite::AlreadyCurrent => quick_color_unchanged_message(index),
                };
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "drawing.quick-color",
                    Toast::info(message),
                );
            }
            Err(error) => {
                let message = if error.downcast_ref::<QuickColorSlotMissing>().is_some() {
                    log::warn!("Quick color slot {index} is no longer in config.toml");
                    quick_color_slot_missing_message(index)
                } else if error.downcast_ref::<ConfigEditNotReadBack>().is_some() {
                    log::warn!("Quick color slot {index} did not read back: {error:#}");
                    quick_color_write_unverified_message(index)
                } else {
                    log::warn!("Failed to save quick color slot {index}: {error:#}");
                    quick_color_save_failed_message(index)
                };
                self.input_state.push_toast(
                    ToastPriority::Action,
                    "drawing.quick-color",
                    Toast::warning(message)
                        .action("Edit", crate::domain::Action::OpenConfiguratorQuickColors),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::io::persist_quick_color_at;
    use crate::config::persist_quick_color;
    use crate::config::test_helpers::with_temp_config_home;
    use crate::config::{Config, QuickColorSlotMissing, QuickColorsConfig};
    use crate::draw::Color;
    use std::fs;

    const NEW_COLOR: Color = Color {
        r: 0.1,
        g: 0.2,
        b: 0.3,
        a: 1.0,
    };

    /// Hand-authored text a recolor must respect: comments, a contested pair of
    /// shortcuts the loader resolves only in memory, and a key from some later
    /// release. The palette is spelled out so the write edits it rather than
    /// materializing it.
    const AUTHORED_FILE: &str = "\
# Wayscriber configuration. These comments must survive a recolor.

[keybindings]
# A contested pair the loader resolves for the session and never repairs.
undo = [\"Ctrl+Alt+Shift+Q\"]
redo = [\"Ctrl+Alt+Shift+Q\"]

[ui]
setting_from_a_later_release = 7

[[drawing.quick_colors]]
label = \"Red\"
color = \"#F5333F\"

[[drawing.quick_colors]]
label = \"Green\"
color = \"#33F54F\"
";

    fn write_fixture(dir: &std::path::Path) -> std::path::PathBuf {
        let path = dir.join("config.toml");
        fs::write(&path, AUTHORED_FILE).expect("the fixture should be written");
        path
    }

    /// The recolor edits its own slot's color and leaves every other byte —
    /// comments, the contested pair, the neighbouring swatch's label, and the
    /// length of the authored palette — alone.
    ///
    /// The written form is an RGB array because that is what `ColorSpec`
    /// serializes to; the neighbouring `"#F5333F"` staying as authored text is
    /// the point, since it proves the write did not re-render entries it was
    /// not asked to change.
    #[test]
    fn a_recolor_changes_exactly_its_own_slot() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = write_fixture(temp.path());

        persist_quick_color_at(&path, 1, NEW_COLOR).expect("the write should succeed");

        assert_eq!(
            fs::read_to_string(&path).expect("readable"),
            AUTHORED_FILE.replace(
                "color = \"#33F54F\"",
                "color = [\n    26,\n    51,\n    77,\n]"
            ),
            "a recolor must change one slot's color and nothing else"
        );
    }

    /// A slot the file only implies has to materialize the array up to it —
    /// there is no other way to express the color — but no further.
    ///
    /// The slots past it stay implied, which is what keeps them tracking the
    /// shipped palette: materializing all eight would freeze colors the user
    /// never chose, and a future build's improved default would never reach
    /// them. The entries it does gain carry the values already in effect, so
    /// nothing changes meaning.
    #[test]
    fn recoloring_an_implied_slot_materializes_only_up_to_that_slot() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = write_fixture(temp.path());

        // Slot 3 is past the two authored entries, inside the default backfill.
        persist_quick_color_at(&path, 3, NEW_COLOR).expect("the write should succeed");

        let after = fs::read_to_string(&path).expect("readable");
        assert_eq!(
            after.matches("[[drawing.quick_colors]]").count(),
            4,
            "the array must reach the edited slot and stop: {after}"
        );
        let reloaded = crate::config::ConfigDocument::load_from_path(&path)
            .expect("the written config should reload");
        let palette = &reloaded.config().drawing.quick_colors;
        assert_eq!(
            palette.configured_entry_count(),
            Some(4),
            "slots past the edited one stay implied, so they keep tracking the defaults"
        );
        let entries = palette.effective_entries();
        assert_eq!(
            entries.get(3).map(|entry| entry.color.clone()),
            Some(crate::config::ColorSpec::from(NEW_COLOR))
        );
        assert_eq!(
            entries.get(2).map(|entry| entry.color.clone()),
            QuickColorsConfig::default()
                .effective_entries()
                .get(2)
                .map(|entry| entry.color.clone()),
            "the slot the write had to pass over keeps the value it already had"
        );
        assert_eq!(
            entries.first().map(|entry| entry.label.clone()),
            Some("Red".to_string()),
            "the authored entries keep their labels"
        );
        assert!(
            after.contains("# A contested pair the loader resolves for the session"),
            "unrelated sections and comments survive: {after}"
        );
    }

    /// Accepting the color a slot already paints has nothing to write, so the
    /// file is not touched and the caller is told that instead of being handed
    /// a success message for a save that did not happen.
    #[test]
    fn recoloring_a_slot_to_the_color_it_already_has_writes_nothing() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = write_fixture(temp.path());
        let existing = crate::config::ConfigDocument::load_from_path(&path)
            .expect("load")
            .config()
            .drawing
            .quick_colors
            .effective_entries()
            .get(1)
            .map(|entry| entry.color.to_color())
            .expect("the fixture authors a second slot");

        let outcome = persist_quick_color_at(&path, 1, existing).expect("a no-op is not a failure");

        assert_eq!(outcome.write, ConfigEditWrite::AlreadyCurrent);
        assert!(outcome.backup_path.is_none(), "no write, no backup");
        assert_eq!(fs::read_to_string(&path).expect("readable"), AUTHORED_FILE);
        assert_eq!(
            quick_color_unchanged_message(1),
            "Quick color 2 already uses that color."
        );
    }

    /// The same for a slot the file only implies: the palette already paints it
    /// that color, so materializing the array to say so is not this gesture's
    /// business.
    #[test]
    fn recoloring_an_implied_slot_to_its_default_writes_nothing() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = write_fixture(temp.path());
        let default =
            crate::config::default_quick_color_for_index(3).expect("slot 4 has a shipped default");

        let outcome = persist_quick_color_at(&path, 3, default).expect("a no-op is not a failure");

        assert_eq!(outcome.write, ConfigEditWrite::AlreadyCurrent);
        assert_eq!(fs::read_to_string(&path).expect("readable"), AUTHORED_FILE);
    }

    #[test]
    fn a_recolor_leaves_a_timestamped_backup() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = write_fixture(temp.path());

        let outcome =
            persist_quick_color_at(&path, 0, NEW_COLOR).expect("the write should succeed");

        let backup = outcome.backup_path.expect("an existing file is backed up");
        assert_eq!(
            fs::read_to_string(&backup).expect("the backup should be readable"),
            AUTHORED_FILE
        );
    }

    /// A slot the palette no longer has is reported as such, not as a save
    /// failure: nothing is wrong with the file, it simply got shorter.
    #[test]
    fn a_missing_slot_is_reported_without_touching_the_file() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = write_fixture(temp.path());

        // Past the authored entries and past the default backfill.
        let error = persist_quick_color_at(&path, 99, NEW_COLOR)
            .expect_err("there is no slot 100 to write");

        assert!(
            error.downcast_ref::<QuickColorSlotMissing>().is_some(),
            "the caller must be able to tell this from a save failure: {error:#}"
        );
        assert_eq!(fs::read_to_string(&path).expect("readable"), AUTHORED_FILE);
        assert_eq!(
            quick_color_slot_missing_message(99),
            "Quick color 100 is no longer in config.toml, so the new color applies to this run only."
        );
    }

    #[test]
    fn a_read_only_config_fails_the_recolor_and_leaves_the_file_alone() {
        use std::os::unix::fs::PermissionsExt;

        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = write_fixture(temp.path());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).expect("chmod file");
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o555)).expect("chmod dir");

        let result = persist_quick_color_at(&path, 0, NEW_COLOR);

        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o755)).expect("restore dir");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("restore file");

        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        assert!(result.is_err(), "a read-only config must fail the write");
        assert!(
            result
                .as_ref()
                .err()
                .and_then(|error| error.downcast_ref::<QuickColorSlotMissing>())
                .is_none(),
            "a permission failure is not a missing slot"
        );
        assert_eq!(fs::read_to_string(&path).expect("readable"), AUTHORED_FILE);
        assert_eq!(
            quick_color_save_failed_message(0),
            "Quick color 1 changed for this run, but saving to config.toml failed (see logs)."
        );
    }

    /// Restart semantics: what this run accepted is what the next process
    /// loads.
    #[test]
    fn a_fresh_load_returns_the_written_quick_color() {
        with_temp_config_home(|config_root| {
            let config_dir = config_root.join(crate::config::PRIMARY_CONFIG_DIR);
            fs::create_dir_all(&config_dir).expect("test config directory");
            fs::write(config_dir.join("config.toml"), AUTHORED_FILE)
                .expect("test config should be written");

            persist_quick_color(1, NEW_COLOR).expect("the write should succeed");

            let restarted = Config::load().expect("test config should reload").config;
            assert_eq!(
                restarted
                    .drawing
                    .quick_colors
                    .effective_entries()
                    .get(1)
                    .map(|entry| entry.color.clone()),
                Some(crate::config::ColorSpec::from(NEW_COLOR))
            );
            assert_eq!(quick_color_saved_message(1), "Updated quick color 2.");
        });
    }
}
