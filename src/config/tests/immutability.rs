//! `config.toml` is an authored input: reading it must leave it exactly as it
//! was, down to the mtime and the mode, and must not put anything next to it.
//!
//! These run every loader over the same set of awkward files — historical
//! revisions, shortcuts that conflict or do not parse, a read-only file, a
//! symlink, a file that is not there — and assert the whole footprint after
//! each one. The startup migration write used to be the exception; there is no
//! exception now, so the invariant is stated once and checked everywhere.

use super::super::test_helpers::{ConfigFileSnapshot, with_temp_config_home};
use super::super::{CURRENT_CONFIG_REVISION, Config, ConfigDocument, PRIMARY_CONFIG_DIR};
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::{PermissionsExt, symlink};

/// Every configuration these tests load, with a name for the failure message.
///
/// The revision rows matter because an old stamp used to trigger a rewrite; the
/// keybinding rows because resolution changes the most in memory and therefore
/// has the most to accidentally write back.
fn fixtures() -> Vec<(&'static str, String)> {
    vec![
        (
            "current revision",
            format!("config_revision = {CURRENT_CONFIG_REVISION}\n\n[ui]\nshow_status_bar = false\n"),
        ),
        (
            "revision 0",
            "[keybindings]\ntoggle_command_palette = [\"Ctrl+K\"]\ncapture_full_screen = [\"Ctrl+Shift+P\"]\n"
                .to_string(),
        ),
        (
            "revision 1",
            "config_revision = 1\n\n[keybindings]\ntoggle_toolbar = [\"F2\", \"F9\"]\n".to_string(),
        ),
        (
            "revision 2",
            "config_revision = 2\n\n[keybindings]\ncapture_clipboard_full = [\"Ctrl+Shift+K\"]\n"
                .to_string(),
        ),
        (
            "invalid, conflicting and case-variant bindings",
            format!(
                "config_revision = {CURRENT_CONFIG_REVISION}\n\n[keybindings]\n\
                 clear_canvas = [\"Ctrl+Shift\"]\n\
                 undo = [\"ctrl+alt+u\"]\n\
                 redo = [\"Ctrl+Alt+U\"]\n\
                 toggle_toolbar = [\"F2\", \"F9\"]\n"
            ),
        ),
        (
            "unknown settings",
            "future_root = \"keep\"\n\n[performance]\nfuture_knob = 7\n".to_string(),
        ),
    ]
}

/// Runs every loader the read paths use, in one process, over one path.
///
/// The snapshot is taken before the first loader and re-checked after each one,
/// so a failure names the loader that broke the invariant rather than the batch.
fn assert_every_loader_is_read_only(config_path: &Path, label: &str) {
    let snapshot = ConfigFileSnapshot::capture(config_path);

    let _ = Config::load();
    snapshot.assert_unchanged(&format!("{label}: Config::load"));

    let _ = Config::load_unvalidated();
    snapshot.assert_unchanged(&format!("{label}: Config::load_unvalidated"));

    let _ = ConfigDocument::load_from_path(config_path);
    snapshot.assert_unchanged(&format!("{label}: ConfigDocument::load_from_path"));

    let _ = ConfigDocument::load_for_editing_from_path(config_path);
    snapshot.assert_unchanged(&format!("{label}: ConfigDocument::load_for_editing"));

    let _ = ConfigDocument::load();
    snapshot.assert_unchanged(&format!("{label}: ConfigDocument::load"));
}

fn primary_config_path(config_root: &Path) -> PathBuf {
    let primary_dir = config_root.join(PRIMARY_CONFIG_DIR);
    fs::create_dir_all(&primary_dir).expect("create config dir");
    primary_dir.join("config.toml")
}

#[test]
fn loading_never_changes_the_config_file() {
    for (label, contents) in fixtures() {
        with_temp_config_home(|config_root| {
            let config_path = primary_config_path(config_root);
            fs::write(&config_path, &contents).expect("write fixture config");

            assert_every_loader_is_read_only(&config_path, label);
        });
    }
}

/// The loaders are also not allowed to conjure a config out of nothing: a
/// missing file is a valid state that means "use the defaults", and creating
/// one would put a file in the user's dotfiles they never asked for.
#[test]
fn loading_never_creates_a_missing_config_file() {
    with_temp_config_home(|config_root| {
        let config_path = primary_config_path(config_root);

        assert_every_loader_is_read_only(&config_path, "missing config");
        assert!(!config_path.exists(), "loading created a config file");
    });
}

/// A read-only config is the sharpest version of the invariant: the loaders
/// have to succeed, and the only reason they can is that they never try to
/// write. Failing to load here would be as wrong as writing.
#[cfg(unix)]
#[test]
fn loading_a_read_only_config_succeeds_and_changes_nothing() {
    with_temp_config_home(|config_root| {
        let config_path = primary_config_path(config_root);
        fs::write(
            &config_path,
            "config_revision = 1\n\n[keybindings]\ntoggle_toolbar = [\"F2\", \"F9\"]\n",
        )
        .expect("write fixture config");
        fs::set_permissions(&config_path, fs::Permissions::from_mode(0o444))
            .expect("make the config read-only");

        let loaded = Config::load().expect("a read-only config still loads");
        assert_eq!(loaded.config.keybindings.ui.toggle_toolbar, ["F2", "F9"]);
        assert!(
            loaded
                .config
                .keybindings
                .ui
                .cycle_toolbar_display
                .is_empty()
        );
        assert_eq!(loaded.config.config_revision, 1);

        assert_every_loader_is_read_only(&config_path, "0444 config");
        assert_eq!(
            fs::metadata(&config_path)
                .expect("read config metadata")
                .permissions()
                .mode()
                & 0o777,
            0o444,
            "loading must not chmod the config"
        );
    });
}

/// A symlinked config has two things to leave alone: the link and what it
/// points at. A write that followed the link would show up in the target's
/// bytes and mtime, and one that replaced the path would show up as the link
/// becoming a regular file.
#[cfg(unix)]
#[test]
fn loading_a_symlinked_config_changes_neither_the_link_nor_the_target() {
    with_temp_config_home(|config_root| {
        let config_path = primary_config_path(config_root);
        let managed_dir = config_root.join("managed-config");
        fs::create_dir_all(&managed_dir).expect("create managed dir");
        let target = managed_dir.join("config.toml");
        fs::write(
            &target,
            "config_revision = 2\n\n[keybindings]\ncapture_clipboard_full = [\"Ctrl+Shift+K\"]\n",
        )
        .expect("write symlink target");
        symlink(&target, &config_path).expect("link the config");

        let target_snapshot = ConfigFileSnapshot::capture(&target);
        assert_every_loader_is_read_only(&config_path, "symlinked config");
        target_snapshot.assert_unchanged("symlinked config: the target");

        assert!(
            fs::symlink_metadata(&config_path)
                .expect("read link metadata")
                .file_type()
                .is_symlink(),
            "the config path must still be a symlink"
        );
        assert_eq!(fs::read_link(&config_path).expect("read link"), target);
    });
}

/// A file the parser cannot make sense of is the one case where a loader has a
/// repair path, and that path is a draft in memory: the unreadable text stays
/// on disk until the user saves over it deliberately.
#[test]
fn loading_an_unparseable_config_leaves_it_for_the_user_to_fix() {
    with_temp_config_home(|config_root| {
        let config_path = primary_config_path(config_root);
        fs::write(&config_path, "not = [valid\n").expect("write broken config");

        let snapshot = ConfigFileSnapshot::capture(&config_path);
        let (document, warning) = ConfigDocument::load_for_editing_from_path(&config_path)
            .expect("a broken config loads as a repairable draft");
        assert!(warning.is_some());
        assert_eq!(document.config().config_revision, CURRENT_CONFIG_REVISION);

        assert_every_loader_is_read_only(&config_path, "unparseable config");
        snapshot.assert_unchanged("unparseable config: the repair draft");
    });
}
