use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use wayscriber::config::{Action, CURRENT_CONFIG_REVISION, Config, ConfigDocument};

use super::*;
use crate::app::effects::Effect;
use crate::app::state::{ConfiguratorApp, StatusMessage};
use crate::models::{ColorPickerId, KeybindingField, ToggleField};
use crate::test_temp::TempDir;

fn status_contains(status: &StatusMessage, needle: &str) -> bool {
    status.text().is_some_and(|text| text.contains(needle))
}

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn temp_config_document(name: &str, contents: &str) -> (PathBuf, Box<ConfigDocument>) {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "wayscriber-configurator-update-config-{}-{sequence}-{name}.toml",
        std::process::id(),
    ));
    std::fs::write(&path, contents).expect("write test config");
    let document = ConfigDocument::load_from_path(&path).expect("load test config document");
    (path, Box::new(document))
}

#[test]
fn handle_config_loaded_success_resets_loading_and_dirty_state() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.is_dirty = true;

    let (path, document) = temp_config_document("loaded", "");
    let _ = app.handle_config_loaded(Ok((document, None)));

    assert!(!app.is_loading);
    assert!(!app.is_dirty);
    assert_eq!(app.boards_collapsed.len(), app.draft.boards.items.len());
    assert!(status_contains(
        &app.status,
        "Configuration loaded from disk."
    ));
    let _ = std::fs::remove_file(path);
}

#[test]
fn handle_config_loaded_uses_startup_search_focus_fallback_once() {
    let (mut app, _effects) = ConfiguratorApp::new_app();

    let (first_path, first) = temp_config_document("focus-first", "");
    let _ = app.handle_config_loaded(Ok((first, None)));

    assert_eq!(app.search_focus_serial, 1);
    assert!(!app.startup_search_focus_pending);

    // A reload is not a relaunch: the offer was answered by the first load,
    // so the caret stays wherever the user put it.
    let (second_path, second) = temp_config_document("focus-second", "");
    let _ = app.handle_config_loaded(Ok((second, None)));

    assert_eq!(app.search_focus_serial, 1);
    let _ = std::fs::remove_file(first_path);
    let _ = std::fs::remove_file(second_path);
}

#[test]
fn handle_config_loaded_error_preserves_the_last_good_document_and_draft() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let (path, document) = temp_config_document("before-reload-error", "");
    let destination = document.destination().to_path_buf();
    let _ = app.handle_config_loaded(Ok((document, None)));
    app.draft.capture_enabled = !app.draft.capture_enabled;
    let draft = app.draft.clone();

    let _ = app.handle_config_loaded(Err("broken".to_string()));

    assert!(!app.is_loading);
    assert_eq!(
        app.base_document
            .as_ref()
            .expect("last good document")
            .destination(),
        destination,
        "a failed reload keeps the document the last good load produced"
    );
    assert_eq!(app.draft, draft);
    assert!(status_contains(
        &app.status,
        "Failed to load config from disk: broken"
    ));
    let _ = std::fs::remove_file(path);
}

#[test]
fn handle_config_loaded_repair_document_allows_saving() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let (path, document) = temp_config_document("repair", "");

    let _ = app.handle_config_loaded(Ok((
        document,
        Some("invalid type: string, expected u32".to_string()),
    )));

    assert!(app.base_document.is_some());
    assert!(matches!(app.status, StatusMessage::Warning(_)));
    assert!(status_contains(&app.status, "loaded for repair"));
    assert!(status_contains(
        &app.status,
        "malformed TOML content remains only in the backup"
    ));
    let _ = app.handle_save_requested();
    assert!(app.is_saving);
    let _ = std::fs::remove_file(path);
}

#[test]
fn handle_config_loaded_surfaces_preserved_unknown_settings() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let (path, document) = temp_config_document("unknown", "future_configurator_option = true\n");

    let _ = app.handle_config_loaded(Ok((document, None)));

    assert!(matches!(app.status, StatusMessage::Warning(_)));
    assert!(status_contains(&app.status, "future_configurator_option"));
    assert!(status_contains(&app.status, "were preserved"));
    let _ = std::fs::remove_file(path);
}

/// A resolved shortcut conflict is never written back, so the editor is
/// where the user has to be able to find it (#293). Both sides here are
/// spelled out in the file, which is what makes it their conflict.
#[test]
fn handle_config_loaded_surfaces_resolved_shortcut_conflicts() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let (path, document) = temp_config_document(
        "shortcut-conflict",
        &format!(
            "config_revision = {}\n\n[keybindings]\ntoggle_toolbar = [\"F2\"]\ncycle_toolbar_display = [\"F2\"]\n",
            wayscriber::config::CURRENT_CONFIG_REVISION
        ),
    );

    let _ = app.handle_config_loaded(Ok((document, None)));

    assert!(matches!(app.status, StatusMessage::Warning(_)));
    assert!(status_contains(&app.status, "F2"));
    assert!(status_contains(&app.status, "Toggle Toolbar"));
    assert!(status_contains(&app.status, "Cycle Toolbar Display"));
    assert!(status_contains(&app.status, "running session only"));
    assert!(
        !status_contains(&app.status, "Unrecognized settings"),
        "a conflict is not an unknown setting"
    );
    let _ = std::fs::remove_file(path);
}

/// A default this build added and the file never mentions gets its own
/// sentence: the user's configuration is fine, and the shortcut they read
/// about in the release notes simply is not theirs (#293).
#[test]
fn handle_config_loaded_surfaces_skipped_default_shortcuts() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let (path, document) = temp_config_document(
        "skipped-default",
        &format!(
            "config_revision = {}\n\n[keybindings]\ntoggle_toolbar = [\"F2\", \"F9\"]\n",
            wayscriber::config::CURRENT_CONFIG_REVISION
        ),
    );

    let _ = app.handle_config_loaded(Ok((document, None)));

    assert!(matches!(app.status, StatusMessage::Warning(_)));
    assert!(status_contains(&app.status, "F2"));
    assert!(status_contains(&app.status, "Cycle Toolbar Display"));
    assert!(status_contains(
        &app.status,
        "New default shortcuts stayed inactive"
    ));
    assert!(
        !status_contains(&app.status, "Conflicting shortcuts")
            && !status_contains(&app.status, "Unrecognized settings"),
        "a skipped default is neither a conflict nor an unknown setting"
    );
    let _ = std::fs::remove_file(path);
}

/// A string the parser rejects is dropped for the session and kept by the
/// file, so the editor is where the user has to be able to find it. With
/// nothing else wrong in the file, this section is the entire warning.
#[test]
fn handle_config_loaded_surfaces_shortcuts_that_could_not_be_parsed() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let (path, document) = temp_config_document(
        "invalid-shortcut",
        &format!(
            "config_revision = {}\n\n[keybindings]\nclear_canvas = [\"Ctrl+Shift\"]\n",
            wayscriber::config::CURRENT_CONFIG_REVISION
        ),
    );

    let _ = app.handle_config_loaded(Ok((document, None)));

    assert!(matches!(app.status, StatusMessage::Warning(_)));
    assert!(status_contains(&app.status, "Ctrl+Shift"));
    assert!(status_contains(&app.status, "Clear Canvas"));
    assert!(status_contains(&app.status, "could not be parsed"));
    assert!(
        !status_contains(&app.status, "Unrecognized settings")
            && !status_contains(&app.status, "Conflicting shortcuts"),
        "an unparseable shortcut is neither an unknown setting nor a conflict"
    );
    let _ = std::fs::remove_file(path);
}

/// All three keybinding kinds can land in one file, and each gets its own
/// sentence: they need different fixes, and one of them needs no fix.
#[test]
fn handle_config_loaded_separates_every_keybinding_diagnostic_kind() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let (path, document) = temp_config_document(
        "invalid-and-conflicting",
        &format!(
            "config_revision = {}\n\n[keybindings]\nclear_canvas = [\"Ctrl+Shift\"]\ntoggle_toolbar = [\"F2\", \"F9\"]\nundo = [\"Ctrl+Alt+U\"]\nredo = [\"Ctrl+Alt+U\"]\n",
            wayscriber::config::CURRENT_CONFIG_REVISION
        ),
    );

    let _ = app.handle_config_loaded(Ok((document, None)));

    assert!(matches!(app.status, StatusMessage::Warning(_)));
    assert!(status_contains(&app.status, "could not be parsed"));
    assert!(status_contains(&app.status, "running session only"));
    assert!(status_contains(
        &app.status,
        "New default shortcuts stayed inactive"
    ));
    assert!(status_contains(&app.status, "Ctrl+Shift"));
    assert!(status_contains(&app.status, "Ctrl+Alt+U"));
    assert!(status_contains(&app.status, "F2"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn handle_save_requested_blocks_without_loaded_document() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    // A fresh app is still running its startup load; this test is about
    // the load having finished without producing a document.
    app.is_loading = false;

    let effects = app.handle_save_requested();

    assert!(effects.is_empty());
    assert!(!app.is_saving);
    assert!(status_contains(
        &app.status,
        "Configuration has not loaded successfully"
    ));
}

#[test]
fn handle_save_requested_sets_saving_for_valid_draft() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.is_saving = false;
    let (path, document) = temp_config_document("save-request", "");
    let _ = app.handle_config_loaded(Ok((document, None)));

    let effects = app.handle_save_requested();

    assert!(matches!(effects.as_slice(), [Effect::SaveConfig { .. }]));
    assert!(app.is_saving);
    assert!(status_contains(&app.status, "Saving configuration..."));
    let _ = std::fs::remove_file(path);
}

/// The document is the model's only copy, and the write needs it moved. It
/// therefore leaves while the write runs and the result brings one back —
/// the saved document on success.
#[test]
fn the_running_save_holds_the_document_and_the_result_returns_one() {
    let (mut app, _dir, _path) = app_with_config_file("");

    let (document, config) = save_effect(&mut app);

    assert!(
        app.base_document.is_none(),
        "the write holds the document while it runs"
    );
    let (saved, backup) = document
        .save_with_backup(*config)
        .expect("the document saves")
        .into_parts();
    let _ = app.handle_config_saved(Ok((backup, Box::new(saved))));

    assert!(
        app.base_document.is_some(),
        "a finished save hands a document back"
    );
}

/// A write that failed wrote nothing, so the document it borrowed is still
/// the one the editor is against: it comes back, and the next Save works.
#[test]
fn a_failed_save_hands_the_document_back() {
    let (mut app, _dir, _path) = app_with_config_file("");
    app.draft.drawing_default_thickness = "6".to_string();
    app.refresh_dirty_flag();

    let (document, _config) = save_effect(&mut app);
    assert!(app.base_document.is_none());

    let _ = app.handle_config_saved(Err((Some(document), "Permission denied".to_string())));

    assert!(!app.is_saving);
    assert!(
        app.base_document.is_some(),
        "the document the failed write borrowed must return to the model"
    );
    assert!(app.is_dirty, "the draft is still unsaved");
    assert!(status_contains(
        &app.status,
        "Failed to save configuration: Permission denied"
    ));
    assert!(
        !status_contains(&app.status, "Reload before saving again"),
        "the document came back, so there is nothing to reload for: {:?}",
        app.status
    );

    // The proof that it came back whole: the very next Save is accepted.
    let effects = app.handle_save_requested();
    assert!(matches!(effects.as_slice(), [Effect::SaveConfig { .. }]));
}

/// The one failure with nothing to hand back is a blocking job that never
/// returned. Saving again cannot work until a reload produces a document,
/// so the status has to say so.
#[test]
fn a_save_whose_job_never_returned_asks_for_a_reload() {
    let (mut app, _dir, _path) = app_with_config_file("");
    let (_document, _config) = save_effect(&mut app);

    let _ = app.handle_config_saved(Err((None, "config save blocking job panicked".to_string())));

    assert!(app.base_document.is_none());
    assert!(status_contains(&app.status, "Reload before saving again"));
}

/// A draft the converter rejects never reaches a write, so the document
/// must be back in the model by the time the handler returns.
#[test]
fn a_draft_the_converter_rejects_keeps_the_document() {
    let (mut app, _dir, _path) = app_with_config_file("");
    app.draft.drawing_default_thickness = "thick".to_string();

    let effects = app.handle_save_requested();

    assert!(effects.is_empty());
    assert!(!app.is_saving);
    assert!(
        app.base_document.is_some(),
        "a refused save must not take the document with it"
    );
    assert!(status_contains(
        &app.status,
        "Cannot save due to validation"
    ));
}

/// Out-of-range numbers used to parse, then `validate_and_clamp` wrote the
/// clamped value. The converter must refuse them so Save cannot change them.
#[test]
fn a_draft_with_out_of_range_numbers_keeps_the_document() {
    let (mut app, _dir, _path) = app_with_config_file("");
    app.draft.drawing_default_thickness = "99".to_string();

    let effects = app.handle_save_requested();

    assert!(effects.is_empty());
    assert!(!app.is_saving);
    assert!(
        app.base_document.is_some(),
        "a refused save must not take the document with it"
    );
    assert!(status_contains(
        &app.status,
        "drawing.default_thickness: Expected 1-50"
    ));
}

/// Hex text the parser rejects was never applied to the draft, so a save
/// would write the last value that did parse and the reload would replace
/// the text with it. The Save is refused instead.
#[test]
fn a_color_field_holding_invalid_hex_blocks_the_save() {
    let (mut app, _dir, _path) = app_with_config_file("");
    app.draft.drawing_default_thickness = "6".to_string();
    app.refresh_dirty_flag();
    let _ = app.handle_color_picker_hex_changed(ColorPickerId::HelpBg, "#12zz".to_string());

    assert_eq!(app.invalid_color_hex_count(), 1);

    let effects = app.handle_save_requested();

    assert!(effects.is_empty());
    assert!(!app.is_saving);
    assert!(app.base_document.is_some(), "nothing was written or taken");
    assert!(status_contains(&app.status, "1 color field"));
    assert!(status_contains(
        &app.status,
        "Enter #RRGGBB or #RRGGBBAA before saving"
    ));
}

/// The one way out of the refusal: type a color that parses.
#[test]
fn correcting_the_color_field_allows_the_save_again() {
    let (mut app, _dir, _path) = app_with_config_file("");
    app.draft.drawing_default_thickness = "6".to_string();
    app.refresh_dirty_flag();
    let _ = app.handle_color_picker_hex_changed(ColorPickerId::HelpBg, "#12zz".to_string());
    assert!(app.handle_save_requested().is_empty());

    let _ = app.handle_color_picker_hex_changed(ColorPickerId::HelpBg, "#102030".to_string());

    assert_eq!(app.invalid_color_hex_count(), 0);
    assert!(matches!(
        app.handle_save_requested().as_slice(),
        [Effect::SaveConfig { .. }]
    ));
}

/// Clearing is not a way out. The picker edits a color the config
/// requires, so an empty field is an edit the save cannot write: letting
/// it through would keep the previous color and put it straight back in
/// the field on the next reload.
#[test]
fn clearing_the_color_field_keeps_the_save_blocked() {
    for cleared in ["", "   "] {
        let (mut app, _dir, _path) = app_with_config_file("");
        app.draft.drawing_default_thickness = "6".to_string();
        app.refresh_dirty_flag();

        let _ = app.handle_color_picker_hex_changed(ColorPickerId::HelpBg, cleared.to_string());

        assert_eq!(app.invalid_color_hex_count(), 1, "{cleared:?}");
        let effects = app.handle_save_requested();
        assert!(effects.is_empty(), "{cleared:?} must not reach a save");
        assert!(status_contains(&app.status, "1 color field"));
    }
}

/// Deleting the row a refused color was in has to release the save with
/// it: the field is gone from the screen, so nothing is left to fix.
#[test]
fn removing_a_quick_color_releases_the_save_its_hex_had_refused() {
    let (mut app, _dir, _path) = app_with_config_file("");
    let _ = app.handle_quick_color_added();
    let last = app.draft.drawing_quick_colors.entries.len() - 1;
    let _ =
        app.handle_color_picker_hex_changed(ColorPickerId::QuickColor(last), "#12zz".to_string());
    assert!(app.handle_save_requested().is_empty());

    let _ = app.handle_quick_color_removed(last);

    assert_eq!(app.invalid_color_hex_count(), 0);
    assert!(matches!(
        app.handle_save_requested().as_slice(),
        [Effect::SaveConfig { .. }]
    ));
}

/// The transient the empty rule could have wedged: editing a component
/// resyncs that picker's hex, so a normal edit never leaves the field
/// blank and the save gate never closes behind the user's back.
#[test]
fn applying_a_color_leaves_the_field_holding_that_color() {
    let (mut app, _dir, _path) = app_with_config_file("");
    let _ = app.handle_color_picker_hex_changed(ColorPickerId::HelpBg, String::new());
    assert_eq!(app.invalid_color_hex_count(), 1);

    let _ = app.handle_color_picker_hex_changed(ColorPickerId::HelpBg, "#102030".to_string());

    assert_eq!(app.invalid_color_hex_count(), 0);
}

/// Several bad fields are one refusal, and the count is what tells the user
/// how much is left to fix.
#[test]
fn every_invalid_color_field_is_counted_for_the_refusal() {
    let (mut app, _dir, _path) = app_with_config_file("");
    let _ = app.handle_color_picker_hex_changed(ColorPickerId::HelpBg, "#12zz".to_string());
    let _ = app.handle_color_picker_hex_changed(ColorPickerId::HelpText, "nope".to_string());

    assert_eq!(app.invalid_color_hex_count(), 2);

    let _ = app.handle_save_requested();

    assert!(status_contains(&app.status, "2 color fields"));
    assert!(status_contains(
        &app.status,
        "Enter #RRGGBB or #RRGGBBAA before saving"
    ));
}

#[test]
fn reset_to_defaults_requires_confirmation() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.is_loading = false;
    app.draft.capture_enabled = !app.defaults.capture_enabled;
    let changed_draft = app.draft.clone();

    let _ = app.handle_reset_to_defaults_requested();

    assert!(app.defaults_reset_pending());
    assert_eq!(app.draft, changed_draft);
    assert!(status_contains(&app.status, "Confirm Defaults"));
}

/// Asking twice is still asking: the request message cannot apply the
/// defaults, so a double press on "Defaults" leaves the draft alone with
/// the question still standing.
#[test]
fn reset_to_defaults_repeated_request_is_a_no_op() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.is_loading = false;
    app.draft.capture_enabled = !app.defaults.capture_enabled;
    let changed_draft = app.draft.clone();

    let _ = app.handle_reset_to_defaults_requested();
    let _ = app.handle_reset_to_defaults_requested();

    assert!(app.defaults_reset_pending());
    assert_eq!(app.draft, changed_draft);
    assert!(status_contains(&app.status, "Confirm Defaults"));
}

/// The confirm is the only message that replaces the draft, and it
/// disarms the question it answered.
#[test]
fn reset_to_defaults_confirmed_applies_the_defaults() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.is_loading = false;
    app.draft.capture_enabled = !app.defaults.capture_enabled;
    app.baseline.capture_enabled = !app.defaults.capture_enabled;

    let _ = app.handle_reset_to_defaults_requested();
    let _ = app.handle_reset_to_defaults_confirmed();

    assert_eq!(app.draft, app.defaults);
    assert!(!app.defaults_reset_pending());
    assert!(status_contains(&app.status, "Loaded default configuration"));
    assert!(
        app.is_dirty,
        "defaults differing from the loaded baseline must read as dirty"
    );
}

/// A confirm with nothing armed answers no question, so it must not
/// replace a draft the user was never warned about.
#[test]
fn reset_to_defaults_confirmed_without_a_request_changes_nothing() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.is_loading = false;
    app.draft.capture_enabled = !app.defaults.capture_enabled;
    let changed_draft = app.draft.clone();

    let _ = app.handle_reset_to_defaults_confirmed();

    assert_eq!(app.draft, changed_draft);
    assert!(!app.defaults_reset_pending());
    assert!(
        !status_contains(&app.status, "Loaded default configuration"),
        "an unarmed confirm reports nothing, because it did nothing"
    );
}

/// Cancel withdraws the question and takes its warning off the status
/// line, leaving the draft exactly as the user left it.
#[test]
fn reset_to_defaults_canceled_disarms_and_clears_the_hint() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.is_loading = false;
    app.draft.capture_enabled = !app.defaults.capture_enabled;
    let changed_draft = app.draft.clone();

    let _ = app.handle_reset_to_defaults_requested();
    let _ = app.handle_reset_to_defaults_canceled();

    assert!(!app.defaults_reset_pending());
    assert_eq!(app.draft, changed_draft);
    assert!(matches!(app.status, StatusMessage::Idle));

    // Nothing is armed now, so the confirm that follows it is inert.
    let _ = app.handle_reset_to_defaults_confirmed();
    assert_eq!(app.draft, changed_draft);
}

#[test]
fn reset_to_defaults_canceled_keeps_status_that_replaced_the_hint() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.is_loading = false;

    let _ = app.handle_reset_to_defaults_requested();
    app.status = StatusMessage::error("Failed to clear session s-1: nope");

    let _ = app.handle_reset_to_defaults_canceled();

    assert!(!app.defaults_reset_pending());
    assert!(matches!(app.status, StatusMessage::Error(_)));
    assert!(status_contains(&app.status, "Failed to clear session s-1"));
}

/// A cancel with nothing armed has no question to withdraw, so it must
/// not wipe the status the user is reading.
#[test]
fn reset_to_defaults_canceled_without_a_request_keeps_the_status() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.is_loading = false;
    app.status = StatusMessage::error("Failed to load config from disk: nope");

    let _ = app.handle_reset_to_defaults_canceled();

    assert!(status_contains(&app.status, "Failed to load config"));
}

#[test]
fn active_confirmation_cancel_uses_the_typed_owner() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.is_loading = false;

    let _ = app.handle_reset_to_defaults_requested();
    let effects = app.handle_active_confirmation_canceled();

    assert!(effects.is_empty());
    assert!(app.pending_confirmation.is_none());
    assert!(matches!(app.status, StatusMessage::Idle));
}

#[test]
fn active_confirmation_cancel_preserves_newer_feedback() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.is_loading = false;

    let _ = app.handle_reset_to_defaults_requested();
    app.status = StatusMessage::error("A newer operation failed");
    let _ = app.handle_active_confirmation_canceled();

    assert!(app.pending_confirmation.is_none());
    assert!(matches!(app.status, StatusMessage::Error(_)));
    assert!(status_contains(&app.status, "newer operation failed"));
}

#[test]
fn reset_to_defaults_confirmation_is_canceled_by_draft_edit() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.is_loading = false;

    let _ = app.handle_reset_to_defaults_requested();
    let _ = app.handle_toggle_changed(ToggleField::CaptureEnabled, !app.draft.capture_enabled);

    assert!(!app.defaults_reset_pending());
    assert!(matches!(app.status, StatusMessage::Idle));
}

/// The disarming an edit does is what the confirm is guarded on: the
/// stale answer to a withdrawn question must not throw the edit away.
#[test]
fn a_draft_edit_between_request_and_confirm_refuses_the_confirm() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.is_loading = false;

    let _ = app.handle_reset_to_defaults_requested();
    let _ = app.handle_toggle_changed(ToggleField::CaptureEnabled, !app.draft.capture_enabled);
    let edited_draft = app.draft.clone();
    let _ = app.handle_reset_to_defaults_confirmed();

    assert_eq!(app.draft, edited_draft);
    assert_ne!(app.draft, app.defaults);
}

/// The reviewer's case: the file spells `undo` out and never mentions
/// `clear_canvas`, and the user then types `undo`'s shortcut into the
/// Clear canvas field. The draft is the authored text now, so both lists
/// are explicit and the traversal order settles the collision (core visits
/// `clear_canvas` before `undo`). Classifying the typed binding as an
/// omitted default instead would filter it away, save an empty list, and
/// report success.
#[test]
fn a_shortcut_typed_for_an_omitted_action_is_arbitrated_not_filtered() {
    let (mut app, _dir, path) = app_with_config_file(&format!(
        "config_revision = {CURRENT_CONFIG_REVISION}\n\n[keybindings]\nundo = [\"Ctrl+Alt+U\"]\n"
    ));
    app.draft
        .keybindings
        .set(KeybindingField::ClearCanvas, "Ctrl+Alt+U".to_string());

    let document = app.base_document.as_ref().expect("a loaded document");
    let mut config = app
        .draft
        .to_config(document.config())
        .expect("the draft converts to a config");
    let report = config.validate_and_clamp();

    assert!(
        report.skipped_default_shortcuts.is_empty(),
        "the user typed this binding; it is not an offer to filter: {:?}",
        report.skipped_default_shortcuts
    );
    assert_eq!(
        config.keybindings.core.clear_canvas,
        ["Ctrl+Alt+U"],
        "the earlier action in traversal order keeps the key"
    );
    assert!(config.keybindings.core.undo.is_empty());
    assert_eq!(report.keybinding_conflicts.len(), 1);
    assert_eq!(report.keybinding_conflicts[0].kept(), Action::ClearCanvas);
    assert_eq!(report.keybinding_conflicts[0].dropped(), Action::Undo);

    let _ = save_draft(&mut app);

    assert!(
        matches!(app.status, StatusMessage::Warning(_)),
        "a binding the save took away is not a plain success: {:?}",
        app.status
    );
    assert!(status_contains(&app.status, "settled before saving"));
    assert!(
        status_contains(
            &app.status,
            "Ctrl+Alt+U kept for Clear Canvas, dropped from Undo."
        ),
        "the status has to name the key, the winner, and the loser: {:?}",
        app.status
    );

    let contents = read_config(&path);
    assert_eq!(
        config_setting(&contents, "clear_canvas").as_deref(),
        Some("clear_canvas = [\"Ctrl+Alt+U\"]"),
        "the typed binding reaches the file"
    );
    assert_eq!(
        config_setting(&contents, "undo").as_deref(),
        Some("undo = []"),
        "the loser is written out too, so the file and the report agree"
    );
}

/// The same collision the other way around: nothing about the draft is
/// wrong, so a save that resolves nothing says nothing extra.
#[test]
fn a_save_without_shortcut_trouble_stays_a_plain_success() {
    let (mut app, _dir, _path) = app_with_config_file(&format!(
        "config_revision = {CURRENT_CONFIG_REVISION}\n\n[keybindings]\nundo = [\"Ctrl+Alt+U\"]\n"
    ));
    app.draft.drawing_default_thickness = "6".to_string();

    let _ = save_draft(&mut app);

    assert!(
        matches!(app.status, StatusMessage::Success(_)),
        "unexpected status: {:?}",
        app.status
    );
    assert!(!status_contains(&app.status, "settled before saving"));
}

/// A shortcut the editor accepts as text but the parser rejects never
/// reaches the file either, so the save status is the only place it can be
/// reported.
#[test]
fn a_typed_shortcut_the_parser_rejects_is_reported_by_the_save() {
    let (mut app, _dir, _path) = app_with_config_file(&format!(
        "config_revision = {CURRENT_CONFIG_REVISION}\n\n[keybindings]\nundo = [\"Ctrl+Alt+U\"]\n"
    ));
    app.draft
        .keybindings
        .set(KeybindingField::ClearCanvas, "Ctrl+Shift".to_string());

    let _ = save_draft(&mut app);

    assert!(
        matches!(app.status, StatusMessage::Warning(_)),
        "unexpected status: {:?}",
        app.status
    );
    assert!(status_contains(&app.status, "Ctrl+Shift"));
    assert!(status_contains(&app.status, "Clear Canvas"));
    assert!(status_contains(&app.status, "could not be parsed"));
}

/// A reload replaces the draft and base document when it lands, so a save
/// started underneath it would write the pre-reload draft and then be
/// judged against a document it never saw.
#[test]
fn save_is_refused_while_a_reload_is_in_flight() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let (path, document) = temp_config_document("save-during-reload", "");
    app.base_document = Some(*document);
    app.is_loading = true;
    app.is_dirty = true;
    let before = app.status.clone();

    let _ = app.handle_save_requested();

    assert!(
        !app.is_saving,
        "no save may start under an in-flight reload"
    );
    assert!(app.is_dirty, "the draft stays dirty for the next attempt");
    assert_eq!(
        format!("{:?}", app.status),
        format!("{before:?}"),
        "a refused save must not claim it is saving"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn handle_config_saved_success_clears_dirty_and_records_backup() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.is_saving = true;
    app.is_dirty = true;
    app.draft.capture_enabled = !app.draft.capture_enabled;
    let backup = PathBuf::from("/tmp/wayscriber-config.bak");
    let (path, document) = temp_config_document("saved", "");

    let _ = app.handle_config_saved(Ok((Some(backup.clone()), document)));
    assert!(app.base_document.is_some());

    assert!(!app.is_saving);
    assert!(!app.is_dirty);
    assert_eq!(app.last_backup_path, Some(backup));
    assert_eq!(app.draft, app.baseline);
    assert!(status_contains(
        &app.status,
        "Configuration saved successfully."
    ));
    let _ = std::fs::remove_file(path);
}

const LEGACY_REVISION_ZERO_CONFIG: &str = "config_revision = 0\n\n[drawing]\ndefault_thickness = 3.0\n\n[keybindings]\ntoggle_command_palette = [\"Ctrl+K\"]\ncapture_full_screen = [\"Ctrl+Shift+P\"]\n";

/// A config file of its own, in a directory the test owns: an applied
/// migration saves, and a save drops its `.bak` next to the file.
fn app_with_config_file(contents: &str) -> (ConfiguratorApp, TempDir, PathBuf) {
    let dir = crate::test_temp::tempdir().expect("temporary test directory");
    let path = dir.path().join("config.toml");
    std::fs::write(&path, contents).expect("write test config");
    let (mut app, _effects) = ConfiguratorApp::new_app();
    load_config_file(&mut app, &path);
    (app, dir, path)
}

fn load_config_file(app: &mut ConfiguratorApp, path: &Path) {
    let document = ConfigDocument::load_from_path(path).expect("load test config document");
    let _ = app.handle_config_loaded(Ok((Box::new(document), None)));
}

/// The Save path with the executor left out: the handler produces exactly
/// this effect, and `save_config_to_disk` performs exactly this write
/// before handing the outcome back to `handle_config_saved`.
fn save_draft(app: &mut ConfiguratorApp) -> Option<PathBuf> {
    let (document, config) = save_effect(app);
    let (saved, backup) = document
        .save_with_backup(*config)
        .expect("the document saves")
        .into_parts();
    let _ = app.handle_config_saved(Ok((backup.clone(), Box::new(saved))));
    backup
}

/// The write a Save asked for, unpacked.
fn save_effect(app: &mut ConfiguratorApp) -> (Box<ConfigDocument>, Box<Config>) {
    let mut effects = app.handle_save_requested();
    assert_eq!(effects.len(), 1, "a Save asks for exactly one write");
    match effects.remove(0) {
        Effect::SaveConfig { document, config } => (document, config),
        other => panic!("a Save must ask for a write, not {other:?}"),
    }
}

/// One setting exactly as the saved file spells it, with the line wrapping
/// the merge may choose for an array folded into single spaces.
fn config_setting(contents: &str, key: &str) -> Option<String> {
    let mut lines = contents.lines().map(str::trim).skip_while(|line| {
        !(line.starts_with(key) && line[key.len()..].trim_start().starts_with('='))
    });
    let mut setting = lines.next()?.to_string();
    while setting.matches('[').count() > setting.matches(']').count() {
        let Some(continuation) = lines.next() else {
            break;
        };
        setting.push(' ');
        setting.push_str(continuation);
    }
    Some(setting.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn read_config(path: &Path) -> String {
    std::fs::read_to_string(path).expect("read the saved config")
}

/// The whole point of the review flow: an old file that the user never
/// migrated keeps both its shortcuts and its revision, however much else
/// they save.
#[test]
fn saving_an_unrelated_field_leaves_old_bindings_and_revision_alone() {
    let (mut app, _dir, path) = app_with_config_file(LEGACY_REVISION_ZERO_CONFIG);
    assert!(app.pending_migration().is_some());

    app.draft.drawing_default_thickness = "6".to_string();
    let _ = save_draft(&mut app);

    let contents = read_config(&path);
    assert_eq!(
        config_setting(&contents, "toggle_command_palette").as_deref(),
        Some("toggle_command_palette = [\"Ctrl+K\"]")
    );
    assert_eq!(
        config_setting(&contents, "capture_full_screen").as_deref(),
        Some("capture_full_screen = [\"Ctrl+Shift+P\"]")
    );
    assert_eq!(
        config_setting(&contents, "config_revision").as_deref(),
        Some("config_revision = 0")
    );
    assert_eq!(
        config_setting(&contents, "default_thickness").as_deref(),
        Some("default_thickness = 6.0"),
        "the field the user did edit still saves"
    );
    assert!(
        app.pending_migration().is_some(),
        "an unrelated save does not answer the migration question"
    );
}

#[test]
fn applying_and_saving_writes_the_reviewed_fields_the_revision_and_a_backup() {
    let (mut app, _dir, path) = app_with_config_file(LEGACY_REVISION_ZERO_CONFIG);

    let _ = app.handle_migration_apply_requested();

    assert!(app.is_dirty);
    assert!(
        app.pending_migration().is_none(),
        "the offer is answered once it is applied"
    );
    assert_eq!(
        app.draft
            .keybindings
            .value_for(KeybindingField::ToggleCommandPalette),
        Some("Ctrl+K, Ctrl+Shift+P")
    );
    assert_eq!(app.draft.config_revision, Some(CURRENT_CONFIG_REVISION));

    let backup = save_draft(&mut app).expect("the save creates a backup");
    assert_eq!(
        std::fs::read_to_string(&backup).expect("read the backup"),
        LEGACY_REVISION_ZERO_CONFIG,
        "the backup holds the file as it was before the migration"
    );

    let contents = read_config(&path);
    assert_eq!(
        config_setting(&contents, "toggle_command_palette").as_deref(),
        Some("toggle_command_palette = [\"Ctrl+K\", \"Ctrl+Shift+P\"]")
    );
    assert_eq!(
        config_setting(&contents, "capture_full_screen").as_deref(),
        Some("capture_full_screen = [\"Ctrl+Alt+F\"]")
    );
    assert_eq!(
        config_setting(&contents, "config_revision"),
        Some(format!("config_revision = {CURRENT_CONFIG_REVISION}"))
    );
    assert_eq!(
        config_setting(&contents, "default_thickness").as_deref(),
        Some("default_thickness = 3.0"),
        "an applied migration is a keybinding delta, not a rewrite"
    );

    load_config_file(&mut app, &path);
    assert!(
        app.pending_migration().is_none(),
        "the reloaded file is current, so there is nothing left to offer"
    );
}

/// The banner is the only place the user reads what Apply would do, so it
/// has to name every proposed change as before → after.
#[test]
fn the_migration_offer_text_lists_every_proposed_change() {
    let (app, _dir, _path) = app_with_config_file(LEGACY_REVISION_ZERO_CONFIG);
    let preview = app.pending_migration().expect("the fixture is out of date");

    let text = migration_offer_text(preview);

    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("Configuration update available"));
    assert!(
        lines
            .next()
            .is_some_and(|line| line.contains("nothing reaches the file until you press Save")),
        "{text}"
    );
    let changes = lines.collect::<Vec<_>>();
    assert_eq!(changes.len(), preview.changes().len());
    for (line, change) in changes.iter().zip(preview.changes()) {
        assert!(line.contains(change.config_key()), "{line}");
        assert!(line.contains(" → "), "{line}");
    }
}

/// Dismissing answers the question for this app run. A reload recomputes
/// the preview, but the user already said no to this file.
#[test]
fn dismissing_hides_the_offer_and_keeps_it_out_of_an_unrelated_save() {
    let (mut app, _dir, path) = app_with_config_file(
        "[keybindings]\ntoggle_command_palette = [\"Ctrl+K\"]\ncapture_full_screen = [\"Ctrl+Shift+P\"]\n",
    );
    assert!(app.pending_migration().is_some());

    let _ = app.handle_migration_dismissed();

    assert!(app.pending_migration().is_none());
    assert!(!app.is_dirty, "dismissing changes nothing in the draft");

    app.draft.drawing_default_thickness = "6".to_string();
    let _ = save_draft(&mut app);

    let contents = read_config(&path);
    assert_eq!(
        config_setting(&contents, "toggle_command_palette").as_deref(),
        Some("toggle_command_palette = [\"Ctrl+K\"]")
    );
    assert_eq!(
        config_setting(&contents, "capture_full_screen").as_deref(),
        Some("capture_full_screen = [\"Ctrl+Shift+P\"]")
    );
    assert_eq!(
        config_setting(&contents, "config_revision"),
        None,
        "a file that never recorded a revision is not stamped by an unrelated save"
    );
    assert!(app.pending_migration().is_none());

    load_config_file(&mut app, &path);
    assert!(
        app.pending_migration().is_none(),
        "pressing Reload is not the user asking again"
    );
}

/// The label the offer itself gives a proposed field, so the assertions on
/// the status text stay in step with the wording the banner shows.
fn change_label(app: &ConfiguratorApp, config_key: &str) -> &'static str {
    app.pending_migration()
        .expect("a pending migration offer")
        .changes()
        .iter()
        .find(|change| change.config_key() == config_key)
        .expect("the offer proposes this key")
        .action_label()
}

/// The preview is computed when the file loads and the draft is editable
/// from that moment on, so Apply must not assume the fields still read the
/// way the proposal was built from. The one the user retyped is theirs; the
/// one they left alone still migrates.
#[test]
fn applying_keeps_a_field_the_user_edited_and_migrates_the_rest() {
    let (mut app, _dir, path) = app_with_config_file(LEGACY_REVISION_ZERO_CONFIG);
    let edited_label = change_label(&app, "toggle_command_palette");
    app.draft
        .keybindings
        .set(KeybindingField::ToggleCommandPalette, "Ctrl+M".to_string());

    let _ = app.handle_migration_apply_requested();

    assert_eq!(
        app.draft
            .keybindings
            .value_for(KeybindingField::ToggleCommandPalette),
        Some("Ctrl+M"),
        "the user's own edit survives the migration they accepted"
    );
    assert_eq!(
        app.draft
            .keybindings
            .value_for(KeybindingField::CaptureFullScreen),
        Some("Ctrl+Alt+F"),
        "a field the user never touched still migrates"
    );
    assert_eq!(
        app.draft.config_revision,
        Some(CURRENT_CONFIG_REVISION),
        "one applied field is a migration, so the revision is recorded"
    );
    assert!(status_contains(&app.status, "Applied 1 shortcut update"));
    assert!(
        status_contains(&app.status, &format!("Kept your edit to {edited_label}")),
        "the status has to name what it did not apply: {:?}",
        app.status
    );

    let _ = save_draft(&mut app);
    let contents = read_config(&path);
    assert_eq!(
        config_setting(&contents, "toggle_command_palette").as_deref(),
        Some("toggle_command_palette = [\"Ctrl+M\"]")
    );
    assert_eq!(
        config_setting(&contents, "capture_full_screen").as_deref(),
        Some("capture_full_screen = [\"Ctrl+Alt+F\"]")
    );
}

/// Comma spacing is formatting, not an edit: the draft reads its fields as
/// a comma-separated list, so text that parses to the proposal's "before"
/// is still the value the proposal was built from, however it is written.
#[test]
fn applying_is_not_defeated_by_the_spacing_of_an_untouched_field() {
    // Revision 1 leaves the `toggle_toolbar` F2 split to propose, which is
    // the migration whose "before" is a list of two.
    let (mut app, _dir, _path) = app_with_config_file(
        "config_revision = 1\n\n[keybindings]\ntoggle_toolbar = [\"F2\", \"F9\"]\n",
    );
    assert_eq!(
        app.draft
            .keybindings
            .value_for(KeybindingField::ToggleToolbar),
        Some("F2, F9")
    );
    app.draft
        .keybindings
        .set(KeybindingField::ToggleToolbar, "F2,F9".to_string());

    let _ = app.handle_migration_apply_requested();

    assert_eq!(
        app.draft
            .keybindings
            .value_for(KeybindingField::ToggleToolbar),
        Some("F9"),
        "the same list written without a space is not an edit to keep"
    );
    assert!(!status_contains(&app.status, "Kept your edit"));
    assert_eq!(app.draft.config_revision, Some(CURRENT_CONFIG_REVISION));
}

/// Every proposed field already reads as its proposed value — the user
/// typed the migration themselves. Nothing is applied, but nothing is left
/// behind either, so the revision that stops the offer coming back is
/// still recorded.
#[test]
fn applying_records_the_revision_when_every_field_already_matches() {
    let (mut app, _dir, _path) = app_with_config_file(LEGACY_REVISION_ZERO_CONFIG);
    app.draft.keybindings.set(
        KeybindingField::ToggleCommandPalette,
        "Ctrl+K, Ctrl+Shift+P".to_string(),
    );
    app.draft
        .keybindings
        .set(KeybindingField::CaptureFullScreen, "Ctrl+Alt+F".to_string());

    let _ = app.handle_migration_apply_requested();

    assert_eq!(app.draft.config_revision, Some(CURRENT_CONFIG_REVISION));
    assert!(status_contains(&app.status, "Applied 0 shortcut updates"));
    assert!(!status_contains(&app.status, "Kept your edit"));
    assert!(status_contains(&app.status, "until you press Save"));
}

/// The user's own edits can answer every proposed field without taking any
/// of the proposal's exact values. Apply keeps those edits, but it is still
/// an explicit answer to the migration question, so saving records the
/// revision and the resolved offer does not return on the next launch.
#[test]
fn a_fully_customized_apply_records_the_revision_without_reoffering() {
    let (mut app, _dir, path) = app_with_config_file(LEGACY_REVISION_ZERO_CONFIG);
    let palette_label = change_label(&app, "toggle_command_palette");
    let capture_label = change_label(&app, "capture_full_screen");
    app.draft
        .keybindings
        .set(KeybindingField::ToggleCommandPalette, "Ctrl+M".to_string());
    app.draft
        .keybindings
        .set(KeybindingField::CaptureFullScreen, "Ctrl+M+F".to_string());
    let _ = app.handle_migration_apply_requested();

    assert_eq!(
        app.draft.config_revision,
        Some(CURRENT_CONFIG_REVISION),
        "Apply records that every proposed field was reviewed"
    );
    assert!(status_contains(&app.status, "Applied 0 shortcut updates"));
    assert!(status_contains(&app.status, palette_label));
    assert!(status_contains(&app.status, capture_label));
    assert!(
        status_contains(&app.status, "until you press Save"),
        "the status has to say the acknowledged revision is still only a draft: {:?}",
        app.status
    );

    let _ = save_draft(&mut app);
    let contents = read_config(&path);
    assert_eq!(
        config_setting(&contents, "config_revision"),
        Some(format!("config_revision = {CURRENT_CONFIG_REVISION}")),
        "saving records the reviewed migration generation"
    );

    load_config_file(&mut app, &path);
    assert!(
        app.pending_migration().is_none(),
        "the user's custom values resolved the offer, so it stays answered after reload"
    );
}

/// A dismissal answers the question about one configuration. With
/// `config.toml` a link into one profile among several, retargeting it and
/// pressing Reload brings up a file the user has never been asked about —
/// and the earlier answer must not hide its offer until a restart.
#[cfg(unix)]
#[test]
fn dismissal_follows_the_file_not_the_path_across_a_retarget() {
    use std::os::unix::fs::symlink;

    let dir = crate::test_temp::tempdir().expect("temporary test directory");
    let first = dir.path().join("profile-a.toml");
    let second = dir.path().join("profile-b.toml");
    std::fs::write(&first, LEGACY_REVISION_ZERO_CONFIG).expect("write the first profile");
    std::fs::write(&second, LEGACY_REVISION_ZERO_CONFIG).expect("write the second profile");
    let link = dir.path().join("config.toml");
    symlink(&first, &link).expect("link the config path at the first profile");

    let (mut app, _effects) = ConfiguratorApp::new_app();
    load_config_file(&mut app, &link);
    assert!(app.pending_migration().is_some());

    let _ = app.handle_migration_dismissed();
    assert!(app.pending_migration().is_none());

    load_config_file(&mut app, &link);
    assert!(
        app.pending_migration().is_none(),
        "the same file reloaded is not the user asking again"
    );

    std::fs::remove_file(&link).expect("unlink the config path");
    symlink(&second, &link).expect("retarget the config path at the second profile");
    load_config_file(&mut app, &link);

    assert!(
        app.pending_migration().is_some(),
        "a different file behind the same path has never been answered"
    );
}

#[test]
fn a_current_revision_file_never_offers_a_migration() {
    let (app, _dir, _path) = app_with_config_file(&format!(
        "config_revision = {CURRENT_CONFIG_REVISION}\n\n[keybindings]\ntoggle_command_palette = [\"Ctrl+K\"]\n"
    ));

    assert!(app.pending_migration().is_none());
}

/// An empty file spells no shortcut out, so every recipe declines and
/// there is nothing to review. A missing file is the same answer from the
/// other direction: the document is this build's defaults.
#[test]
fn an_empty_or_missing_file_never_offers_a_migration() {
    let (app, _dir, _path) = app_with_config_file("");
    assert!(app.pending_migration().is_none());

    let dir = crate::test_temp::tempdir().expect("temporary test directory");
    let (mut app, _effects) = ConfiguratorApp::new_app();
    load_config_file(&mut app, &dir.path().join("missing.toml"));

    assert!(app.pending_migration().is_none());
}

/// The input-HUD step proposes unbinding a default the file never spelled
/// out, and loading already dropped that default from the effective
/// keymap — so the draft text does not change at all. The revision is
/// what makes the applied migration savable.
#[test]
fn applying_marks_the_draft_dirty_when_only_the_revision_changes() {
    let (mut app, _dir, path) = app_with_config_file(
        "config_revision = 2\n\n[keybindings]\ncapture_clipboard_full = [\"Ctrl+Shift+K\"]\n",
    );
    let text_before = app
        .draft
        .keybindings
        .value_for(KeybindingField::ToggleInputHud)
        .map(str::to_string);

    let _ = app.handle_migration_apply_requested();

    assert_eq!(
        app.draft
            .keybindings
            .value_for(KeybindingField::ToggleInputHud)
            .map(str::to_string),
        text_before,
        "loading already resolved this binding away, so the text is unchanged"
    );
    assert!(app.is_dirty, "the proposed revision is a draft change");

    let _ = save_draft(&mut app);

    let contents = read_config(&path);
    assert_eq!(
        config_setting(&contents, "config_revision"),
        Some(format!("config_revision = {CURRENT_CONFIG_REVISION}"))
    );
    assert_eq!(
        config_setting(&contents, "capture_clipboard_full").as_deref(),
        Some("capture_clipboard_full = [\"Ctrl+Shift+K\"]"),
        "the authored binding the migration protects is left alone"
    );

    load_config_file(&mut app, &path);
    assert!(app.pending_migration().is_none());
}
