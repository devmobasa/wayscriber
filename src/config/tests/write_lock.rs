//! Two writers, one `config.toml`.
//!
//! The configurator's Save and the overlay's narrow editors are separate
//! processes writing the same file. Each one checks that the file still holds
//! the bytes its document loaded and then replaces it by rename — two steps,
//! and without exclusion both writers can pass the check before either renames.
//! The second rename then discards the first edit while both writers report
//! success, and both `.bak` copies hold the same pre-edit source, so the lost
//! edit is not recoverable either.
//!
//! These exercise the advisory lock that makes the check and the rename one
//! step. Removing `acquire_config_write_lock` from `merge_and_write` fails both.

use super::super::io::{persist_preset_slot_at, persist_quick_color_at};
use super::super::{ColorSpec, ToolPresetConfig};
use crate::draw::Color;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Barrier;
use std::thread;
use std::time::Duration;

/// Long enough that the waiting editor is provably still waiting, and far
/// inside the save's own five-second lock deadline.
const HOLD: Duration = Duration::from_millis(150);

/// Hand-authored text both writers must respect.
const ORIGINAL: &str = "\
# Wayscriber configuration. Neither writer may lose this.
[ui]
setting_from_a_later_release = 7
";

/// What the writer that wins the lock leaves behind.
const WINNER_KEY: &str = "show_status_bar = false";

fn preset(name: &str) -> ToolPresetConfig {
    ToolPresetConfig {
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
    }
}

const NEW_COLOR: Color = Color {
    r: 0.1,
    g: 0.2,
    b: 0.3,
    a: 1.0,
};

fn fixture(directory: &Path) -> PathBuf {
    fs::create_dir_all(directory).expect("the fixture directory this test just named");
    let path = directory.join("config.toml");
    fs::write(&path, ORIGINAL).expect("the fixture this test just named a directory for");
    path
}

/// The lock file a save takes, named here rather than reached for through the
/// writer's own module: it is the rendezvous two *processes* have to agree on,
/// so the name is part of the contract and belongs in the test.
fn config_write_lock_path(config_path: &Path) -> PathBuf {
    let name = config_path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .expect("the fixture path this test built has a file name");
    config_path.with_file_name(format!("{name}.lock"))
}

/// Hold that lock the way a second process inside its write window would.
fn hold_config_write_lock(config_path: &Path) -> File {
    let path = config_write_lock_path(config_path);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .expect("the lock file beside a fixture this test created");
    crate::session::try_lock_exclusive(&file)
        .expect("nothing else can hold the lock on a directory this test just made");
    file
}

fn backups_in(directory: &Path) -> Vec<String> {
    let mut contents = Vec::new();
    for entry in fs::read_dir(directory).expect("the directory this test created") {
        let path = entry.expect("a directory entry this test created").path();
        if path.extension().is_some_and(|extension| extension == "bak") {
            contents.push(fs::read_to_string(&path).expect("a backup this test's writers made"));
        }
    }
    contents
}

/// The deterministic half: an edit that arrives while another writer holds the
/// window waits for it, then finds the file changed and reapplies on top.
///
/// The hold is staged rather than raced, so the interleaving is the same on
/// every run. Without the lock the editor does not wait at all: it writes inside
/// the hold, and the write that follows the hold — the one that would have been
/// under the lock — replaces the file and takes the preset with it.
#[test]
fn an_edit_that_waits_for_the_lock_reapplies_onto_the_writer_that_won() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let path = fixture(temp.path());

    let held = hold_config_write_lock(&path);

    thread::scope(|scope| {
        let waiting = scope.spawn(|| persist_preset_slot_at(&path, 1, Some(&preset("Waited"))));

        // The editor is inside its wait by now; land the winning write and let
        // it through.
        thread::sleep(HOLD);
        fs::write(&path, format!("{ORIGINAL}{WINNER_KEY}\n"))
            .expect("the winning writer's rename, staged");
        drop(held);

        waiting
            .join()
            .expect("the waiting editor must not panic")
            .expect("the waiting editor must succeed once the window closes");
    });

    let after = fs::read_to_string(&path).expect("readable");
    assert!(
        after.contains(WINNER_KEY),
        "the write that held the window must survive: {after}"
    );
    assert!(
        after.contains("[presets.slot_1]"),
        "the edit that waited for it must land on top, not be dropped: {after}"
    );
    assert!(
        after.contains("setting_from_a_later_release = 7"),
        "and neither writer may lose what the file already said: {after}"
    );
}

/// The property, raced for real: two editors starting together both land, and
/// the backup chain still holds what the file said before either of them ran.
///
/// One pass is enough to describe the contract; the repetition is what makes
/// removing the lock show up, since an unguarded pair only loses an edit when
/// the two windows actually overlap.
#[test]
fn two_racing_edits_both_land_and_a_backup_still_holds_the_original() {
    let temp = crate::test_temp::tempdir().expect("tempdir");

    for attempt in 0..24 {
        let directory = temp.path().join(format!("attempt-{attempt}"));
        let path = fixture(&directory);
        let start = Barrier::new(2);

        thread::scope(|scope| {
            let preset_edit = scope.spawn(|| {
                start.wait();
                persist_preset_slot_at(&path, 1, Some(&preset("Raced")))
            });
            let color_edit = scope.spawn(|| {
                start.wait();
                persist_quick_color_at(&path, 0, NEW_COLOR)
            });

            preset_edit
                .join()
                .expect("the preset editor must not panic")
                .expect("the preset editor must not lose to a race it can retry");
            color_edit
                .join()
                .expect("the quick-color editor must not panic")
                .expect("the quick-color editor must not lose to a race it can retry");
        });

        let after = fs::read_to_string(&path).expect("readable");
        assert!(
            after.contains("[presets.slot_1]"),
            "attempt {attempt}: the preset edit was overwritten: {after}"
        );
        assert!(
            after.contains("[[drawing.quick_colors]]"),
            "attempt {attempt}: the quick-color edit was overwritten: {after}"
        );
        assert!(
            after.contains("setting_from_a_later_release = 7"),
            "attempt {attempt}: the authored file was lost: {after}"
        );

        let backups = backups_in(&directory);
        assert_eq!(
            backups.len(),
            2,
            "attempt {attempt}: each write copies the previous contents aside"
        );
        assert!(
            backups.iter().any(|contents| contents == ORIGINAL),
            "attempt {attempt}: the state before either edit must still be recoverable"
        );
    }
}
