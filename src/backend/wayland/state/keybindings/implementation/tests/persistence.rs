use super::*;

/// The persistence fixture: hand-authored text the write must respect.
///
/// It deliberately holds everything the loader changes in memory —
/// comments, an unrelated pair of actions contesting one chord, an
/// unparseable binding, and a setting this build does not know — so that a
/// write which leaked validation results would be obvious in the diff.
const AUTHORED_FILE: &str = "\
# Wayscriber configuration. These comments must survive a shortcut edit.

[keybindings]
# The shortcut under test.
select_pen_tool = [\"Ctrl+Alt+Shift+P\"]
# A contested pair: the loader gives the chord to one of them for the session
# and reports the other, but must never repair the file.
undo = [\"Ctrl+Alt+Shift+Q\"]
redo = [\"Ctrl+Alt+Shift+Q\"]
# Nonsense the loader drops for the session and keeps on disk.
clear_canvas = [\"NotARealKey\"]

[ui]
# An unrelated section, plus a key from some future release.
show_status_bar = false
setting_from_a_later_release = 7
";

/// The chord the stale-edit fixture below hands to another action.
const CONTESTED_CHORD: &str = "Ctrl+Alt+Shift+M";

/// The file as it stands *after* the palette read its keymap: `undo` has
/// taken the chord the edit is about to ask for, and the action being
/// edited is one this file omits — which is what used to make validation
/// treat the requested list as a droppable offer.
const CLAIMED_ON_DISK_FILE: &str = "\
# A writer this run never saw got here first.
[keybindings]
undo = [\"Ctrl+Alt+Shift+M\"]
";

fn config_in(dir: &Path) -> PathBuf {
    dir.join("config.toml")
}

fn write_fixture(dir: &Path) -> PathBuf {
    let path = config_in(dir);
    fs::write(&path, AUTHORED_FILE).expect("the fixture should be written");
    path
}

/// The core durability property: the edited key moves and the file is
/// otherwise byte-identical, comments and all.
#[test]
fn a_shortcut_write_changes_exactly_its_own_key() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let path = write_fixture(temp.path());

    persist_keybinding_edit_at(
        &path,
        Action::SelectPenTool,
        &["Ctrl+Alt+Shift+K".to_string()],
    )
    .expect("the write should succeed");

    let after = fs::read_to_string(&path).expect("the config should be readable");
    assert_eq!(
        after,
        AUTHORED_FILE.replace(
            "select_pen_tool = [\"Ctrl+Alt+Shift+P\"]",
            "select_pen_tool = [\"Ctrl+Alt+Shift+K\"]"
        ),
        "a shortcut write must change one key and nothing else"
    );
}

/// Unbind and reset write through the same one-key path.
#[test]
fn unbinding_and_resetting_also_change_exactly_their_own_key() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let path = write_fixture(temp.path());

    persist_keybinding_edit_at(&path, Action::SelectPenTool, &[])
        .expect("an unbind should be writable");
    assert_eq!(
        fs::read_to_string(&path).expect("readable"),
        AUTHORED_FILE.replace(
            "select_pen_tool = [\"Ctrl+Alt+Shift+P\"]",
            "select_pen_tool = []"
        )
    );

    // Reset writes the shipped list out in full rather than deleting the
    // key. Removing it would hand the action back to presence-based
    // resolution, where the same default can stand down against another
    // binding — the user asked for the default, not for the offer of one.
    // (Deleting the key instead is a possible follow-up, but it is a
    // different promise.)
    let default = KeybindingsConfig::default()
        .bindings_for_action(Action::SelectPenTool)
        .map(<[String]>::to_vec)
        .expect("the pen tool ships a shortcut");
    persist_keybinding_edit_at(&path, Action::SelectPenTool, &default)
        .expect("a reset should be writable");
    assert_eq!(
        fs::read_to_string(&path).expect("readable"),
        AUTHORED_FILE.replace(
            "select_pen_tool = [\"Ctrl+Alt+Shift+P\"]",
            "select_pen_tool = [\"F\"]"
        )
    );
}

#[test]
fn a_shortcut_write_leaves_a_timestamped_backup() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let path = write_fixture(temp.path());

    let outcome = persist_keybinding_edit_at(
        &path,
        Action::SelectPenTool,
        &["Ctrl+Alt+Shift+K".to_string()],
    )
    .expect("the write should succeed");

    let backup = outcome.backup_path.expect("an existing file is backed up");
    assert_eq!(
        fs::read_to_string(&backup).expect("the backup should be readable"),
        AUTHORED_FILE,
        "the backup holds the contents from before the write"
    );
}

/// Presence is what tells an authored shortcut from a compiled-in offer, so
/// an action the file omitted has to come back explicit once it is written.
#[test]
fn a_written_shortcut_reloads_as_explicitly_authored() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let path = write_fixture(temp.path());
    assert!(
        !AUTHORED_FILE.contains("select_step_marker_tool"),
        "the fixture must omit the action this test writes"
    );

    persist_keybinding_edit_at(
        &path,
        Action::SelectStepMarkerTool,
        &["Ctrl+Alt+Shift+M".to_string()],
    )
    .expect("the write should succeed");

    let reloaded = crate::config::ConfigDocument::load_from_path(&path)
        .expect("the written config should reload");
    assert!(
        reloaded
            .keybinding_authorship()
            .is_explicit("select_step_marker_tool"),
        "the written key must read back as authored"
    );
    assert_eq!(
        reloaded
            .config()
            .keybindings
            .bindings_for_action(Action::SelectStepMarkerTool),
        Some(&["Ctrl+Alt+Shift+M".to_string()][..])
    );
}

/// Reset on an action that already resolves to the shipped shortcut has no
/// delta to write, so nothing is written — and the caller is told that
/// rather than being handed a success message for a save that never
/// happened. Most actions are in this state: the file simply omits them.
#[test]
fn resetting_an_omitted_action_already_at_its_default_writes_nothing() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let path = write_fixture(temp.path());
    assert!(
        !AUTHORED_FILE.contains("select_marker_tool"),
        "the fixture must omit the action this test resets"
    );
    let default = KeybindingsConfig::default()
        .bindings_for_action(Action::SelectMarkerTool)
        .map(<[String]>::to_vec)
        .expect("the marker tool has a stored shortcut list");
    assert_eq!(default, ["H"], "and it must ship a shortcut to reset to");

    let outcome = persist_keybinding_edit_at(&path, Action::SelectMarkerTool, &default)
        .expect("a no-op edit is not a failure");

    assert_eq!(
        outcome.write,
        ConfigEditWrite::AlreadyCurrent,
        "the file already resolved to this, so nothing was written"
    );
    assert!(
        outcome.backup_path.is_none(),
        "a write that did not happen must not spend a backup"
    );
    assert_eq!(fs::read_to_string(&path).expect("readable"), AUTHORED_FILE);
    assert_eq!(
        shortcut_unchanged_message(&KeybindingEditRequest {
            action: Action::SelectMarkerTool,
            operation: KeybindingEditOperation::Reset,
        }),
        "Marker Tool already uses the default shortcut.",
        "the toast must not claim a durable change this made"
    );
}

/// The other Reset case: the file authors something else, so the default is
/// a real delta — it is written, which is what pins it against a future
/// build changing the shipped value.
#[test]
fn resetting_an_authored_shortcut_writes_the_default_and_reports_it() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let path = write_fixture(temp.path());
    let default = KeybindingsConfig::default()
        .bindings_for_action(Action::SelectPenTool)
        .map(<[String]>::to_vec)
        .expect("the pen tool ships a shortcut");

    let outcome = persist_keybinding_edit_at(&path, Action::SelectPenTool, &default)
        .expect("a reset should be writable");

    assert_eq!(outcome.write, ConfigEditWrite::Wrote);
    assert!(outcome.backup_path.is_some());
    assert_eq!(
        fs::read_to_string(&path).expect("readable"),
        AUTHORED_FILE.replace(
            "select_pen_tool = [\"Ctrl+Alt+Shift+P\"]",
            "select_pen_tool = [\"F\"]"
        )
    );
}

/// The reviewer's race, staged exactly: the palette's conflict check passed
/// against the keymap this run loaded, and by the time the write runs the
/// file gives that chord to another action.
///
/// The edit is refused before anything is written. Letting it through wrote
/// an *empty* list for the edited action — validation drops a list it still
/// reads as an unauthored offer against the newer claimant, and the merge
/// gate writes that difference — after which the read-back check failed and
/// the caller reported a save failure over a file it had just changed.
#[test]
fn a_chord_claimed_on_disk_since_the_edit_is_refused_without_writing() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let path = config_in(temp.path());
    fs::write(&path, CLAIMED_ON_DISK_FILE).expect("the fixture should be written");
    assert!(
        !CLAIMED_ON_DISK_FILE.contains("clear_canvas"),
        "the edited action must be one the file omits, as in the report"
    );
    assert!(
        !KeybindingsConfig::default()
            .bindings_for_action(Action::ClearCanvas)
            .is_none_or(<[String]>::is_empty),
        "and one with a shipped shortcut, or there is no value to empty out"
    );

    let error =
        persist_keybinding_edit_at(&path, Action::ClearCanvas, &[CONTESTED_CHORD.to_string()])
            .expect_err("the file gives that chord to another action");

    let conflict = error
        .downcast_ref::<ShortcutClaimedOnDisk>()
        .unwrap_or_else(|| panic!("the caller must be able to tell a refusal apart: {error:#}"));
    assert_eq!(conflict.claimed_by, Action::Undo);
    assert_eq!(conflict.binding, CONTESTED_CHORD);
    assert_eq!(
        fs::read_to_string(&path).expect("readable"),
        CLAIMED_ON_DISK_FILE,
        "a refused edit must leave the file byte-identical"
    );
    assert_eq!(
        shortcut_claimed_on_disk_message(&conflict.binding, conflict.claimed_by),
        "Shortcut not changed — config.toml now assigns Ctrl+Alt+Shift+M to Undo.",
        "the refusal names the action that owns the chord"
    );
}

/// The invariant the delta install rests on, staged through real writes.
///
/// The palette's own check refuses this pair before it is queued — the
/// second edit is checked against the first one's outstanding delta — but
/// that check only knows about this run's queue. Another window, the
/// configurator, or a hand edit can take a chord it has never heard of, so
/// the write re-reads the file it is about to change and refuses there. It
/// is the *file*, not the run, that is the authority on a contest, and the
/// second edit is refused rather than installed. That is why folding deltas
/// into the current keymap cannot put two actions on one chord.
///
/// The contesting pair is therefore staged directly, by checking each edit
/// against a keymap that carries neither: what is under test is what the
/// worker does with two of them, not how they got past the palette.
#[test]
fn an_overlapping_edit_onto_the_chord_the_first_took_is_refused_by_the_file() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let path = write_fixture(temp.path());
    let running = KeybindingsConfig::default();
    let contested = "Ctrl+Alt+Shift+X";

    let pen = prepare(&running, replace(Action::SelectPenTool, contested))
        .expect("the chord is free in this run's keymap");
    let marker = prepare(&running, replace(Action::SelectMarkerTool, contested))
        .expect("and still free, because the first edit is not installed");

    // The worker writes them in submission order.
    persist_keybinding_edit_at(&path, pen.request.action, &pen.bindings)
        .expect("the first write should land");
    let error = persist_keybinding_edit_at(&path, marker.request.action, &marker.bindings)
        .expect_err("the second write reads the file the first one changed");

    let completion = shortcut_completion(marker, Err(error));
    assert!(
        completion.install.is_none(),
        "a chord the file has just given away is refused, not folded in"
    );
    assert!(!completion.saved);
    assert_eq!(
        completion.message,
        "Shortcut not changed — config.toml now assigns Ctrl+Alt+Shift+X to Pen Tool.",
        "and the refusal names the action that took it"
    );
    let after = fs::read_to_string(&path).expect("readable");
    assert!(
        after.contains("select_pen_tool = [\"Ctrl+Alt+Shift+X\"]"),
        "the first edit is what the file kept: {after}"
    );
    assert!(
        !after.contains("select_marker_tool"),
        "and the refused edit wrote nothing: {after}"
    );
}

/// What loading decided in memory stays in memory, even when the write
/// marks a key authored on its way past.
///
/// The fixture spends `undo`'s shipped `Ctrl+Z` on an authored
/// `toggle_input_hud`, so loading stands the omitted `undo` default down.
/// That decision belongs to the session: an edit to an unrelated action
/// must not pin it into the file, in either direction.
#[test]
fn an_edit_leaves_the_omitted_default_that_stood_down_alone() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let path = config_in(temp.path());
    let authored = "[keybindings]\ntoggle_input_hud = [\"Ctrl+Z\"]\n";
    fs::write(&path, authored).expect("the fixture should be written");

    persist_keybinding_edit_at(
        &path,
        Action::SelectPenTool,
        &["Ctrl+Alt+Shift+K".to_string()],
    )
    .expect("the write should succeed");

    assert_eq!(
        fs::read_to_string(&path).expect("readable"),
        format!("{authored}select_pen_tool = [\"Ctrl+Alt+Shift+K\"]\n"),
        "only the edited action's key may appear"
    );
}

/// A config the process cannot write is not a reason to lose the shortcut:
/// the caller keeps the in-memory edit and is told the file missed it.
#[test]
fn a_read_only_config_fails_the_write_and_leaves_the_file_alone() {
    use std::os::unix::fs::PermissionsExt;

    let temp = crate::test_temp::tempdir().expect("tempdir");
    let path = write_fixture(temp.path());
    // The file mode alone would not stop it: the write is an atomic
    // replace, so it is the directory that has to refuse the new entry
    // (and the backup copy that lands beside it).
    fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).expect("chmod file");
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o555)).expect("chmod dir");

    let result = persist_keybinding_edit_at(
        &path,
        Action::SelectPenTool,
        &["Ctrl+Alt+Shift+K".to_string()],
    );

    // Restore before asserting so a failure cannot leave an unremovable
    // directory behind for the harness.
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o755)).expect("restore dir");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("restore file");

    if unsafe { libc::geteuid() } == 0 {
        // Root ignores these modes; the scenario cannot be staged.
        return;
    }
    assert!(result.is_err(), "a read-only config must fail the write");
    assert_eq!(
        fs::read_to_string(&path).expect("readable"),
        AUTHORED_FILE,
        "a failed write must leave the file exactly as it was"
    );
    assert_eq!(
        SHORTCUT_SAVE_FAILED,
        "Shortcut updated for this run, but saving to config.toml failed (see logs).",
        "the degradation message is what the user sees for this case"
    );
}

/// An unparseable config is repaired in the configurator, deliberately and
/// with the damage on screen — never as a side effect of a rebind.
#[test]
fn an_unparseable_config_is_refused_rather_than_rebuilt_from_defaults() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let path = config_in(temp.path());
    fs::write(&path, "[keybindings\nundo = broken").expect("the fixture should be written");

    let error = persist_keybinding_edit_at(
        &path,
        Action::SelectPenTool,
        &["Ctrl+Alt+Shift+K".to_string()],
    )
    .expect_err("a broken config must not be silently replaced");

    assert!(
        format!("{error:#}").contains("could not be parsed"),
        "error: {error:#}"
    );
    assert_eq!(
        fs::read_to_string(&path).expect("readable"),
        "[keybindings\nundo = broken"
    );
}

/// The retry exists for a real error string produced by a real stale save,
/// so this stages that save rather than trusting the wording.
#[test]
fn a_stale_document_is_recognised_and_retried() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let path = write_fixture(temp.path());

    let document = crate::config::ConfigDocument::load_from_path(&path).expect("load");
    fs::write(&path, format!("{AUTHORED_FILE}\n# a second writer\n")).expect("outside write");
    let error = document
        .save_with_backup(document.config().clone())
        .expect_err("the document is stale");

    assert!(
        is_stale_source_error(&error),
        "the retry gate must recognise this error: {error:#}"
    );

    // And the public path recovers from exactly that situation.
    persist_keybinding_edit_at(
        &path,
        Action::SelectPenTool,
        &["Ctrl+Alt+Shift+K".to_string()],
    )
    .expect("a fresh load succeeds");
    assert!(
        fs::read_to_string(&path)
            .expect("readable")
            .contains("select_pen_tool = [\"Ctrl+Alt+Shift+K\"]")
    );
}

/// Restart semantics, now the other way round: what this run wrote is what
/// the next process loads.
#[test]
fn a_fresh_load_returns_the_written_shortcut() {
    with_temp_config_home(|config_root| {
        let config_dir = config_root.join(crate::config::PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&config_dir).expect("test config directory");
        let path = config_dir.join("config.toml");
        fs::write(&path, AUTHORED_SHORTCUTS).expect("test config should be written");

        // Through the environment-resolved entry point the overlay calls,
        // so the wiring from `Config::get_config_path` is covered too.
        persist_keybinding_edit(Action::SelectPenTool, &["Ctrl+Alt+Shift+K".to_string()])
            .expect("the write should succeed");

        let restarted = Config::load().expect("test config should reload").config;
        assert_eq!(
            restarted
                .keybindings
                .bindings_for_action(Action::SelectPenTool),
            Some(&["Ctrl+Alt+Shift+K".to_string()][..])
        );
    });
}
