use wayscriber::config::{CURRENT_CONFIG_REVISION, ConfigDocument, Shortcut};

use crate::app::effects::Effect;
use crate::app::state::{ConfiguratorApp, PendingConfirmation, StatusMessage};
use crate::models::keybindings::keyval;
use crate::models::{KeybindingField, KeyboardModifiers, RecorderDeviceKind, SearchQuery};
use crate::test_temp::TempDir;

fn status_contains(status: &StatusMessage, needle: &str) -> bool {
    status.text().is_some_and(|text| text.contains(needle))
}

fn load_config(app: &mut ConfiguratorApp, contents: &str) -> (TempDir, std::path::PathBuf) {
    let dir = crate::test_temp::tempdir().expect("temporary test directory");
    let path = dir.path().join("config.toml");
    std::fs::write(&path, contents).expect("write test config");
    let document = ConfigDocument::load_from_path(&path).expect("load test config document");
    let _ = app.handle_config_loaded(Ok((Box::new(document), None)));
    (dir, path)
}

fn save_draft(app: &mut ConfiguratorApp) {
    let mut effects = app.handle_save_requested();
    assert_eq!(effects.len(), 1, "a Save asks for exactly one write");
    match effects.remove(0) {
        Effect::SaveConfig { document, config } => {
            let (saved, backup) = document
                .save_with_backup(*config)
                .expect("the document saves")
                .into_parts();
            let _ = app.handle_config_saved(Ok((backup, Box::new(saved))));
        }
        other => panic!("a Save must ask for a write, not {other:?}"),
    }
}

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

#[test]
fn starting_one_recorder_closes_any_older_recorder() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let _ = app.handle_shortcut_recording_started(KeybindingField::ClearCanvas);
    let _ = app.handle_shortcut_recording_started(KeybindingField::Undo);
    assert_eq!(
        app.active_shortcut_recorder
            .as_ref()
            .map(|recorder| recorder.field),
        Some(KeybindingField::Undo)
    );
}

#[test]
fn confirmation_and_shortcut_conflict_do_not_consume_each_other() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.pending_confirmation = Some(PendingConfirmation::DefaultsReset);
    let _ = app.handle_shortcut_recording_started(KeybindingField::ToggleFloatingBadge);
    let _ = app.handle_shortcut_recorder_key(
        u32::from(b'e'),
        KeyboardModifiers {
            ctrl: false,
            shift: false,
            alt: false,
            super_held: false,
        },
    );
    assert!(
        app.pending_confirmation.is_some(),
        "recording a conflict must not disarm Defaults"
    );
    assert!(app.pending_shortcut_conflict.is_some());
    let _ = app.handle_window_escape_pressed();
    assert!(
        app.pending_confirmation.is_none(),
        "Escape still cancels Defaults when the recorder is closed"
    );
    assert!(
        app.pending_shortcut_conflict.is_some(),
        "Escape must not take the shortcut conflict with it"
    );
}

#[test]
fn search_changes_do_not_dirty_the_config() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.is_dirty = false;
    let _ = app.handle_search_changed("undo".to_string());
    assert!(!app.is_dirty);
    assert_eq!(app.search_query, SearchQuery::new("undo"));
}

#[test]
fn recording_reset_and_removal_dirty_without_saving() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let _ = app.handle_shortcut_recording_started(KeybindingField::ToggleFloatingBadge);
    let effects = app.handle_shortcut_recorder_key(keyval::F5, KeyboardModifiers::default());
    assert!(effects.is_empty());
    assert!(app.is_dirty);
    assert_eq!(
        app.draft
            .keybindings
            .value_for(KeybindingField::ToggleFloatingBadge),
        Some("F5")
    );

    let _ = app.handle_shortcut_reset_requested(KeybindingField::ClearCanvas);
    assert_eq!(
        app.draft
            .keybindings
            .value_for(KeybindingField::ClearCanvas),
        Some("E")
    );
    let effects = app.handle_shortcut_removed(
        KeybindingField::ToggleFloatingBadge,
        Shortcut::parse("F5").expect("parses"),
    );
    assert!(effects.is_empty());
    assert_eq!(
        app.draft
            .keybindings
            .value_for(KeybindingField::ToggleFloatingBadge),
        Some("")
    );
}

#[test]
fn recorder_escape_does_not_cancel_defaults_confirmation() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.pending_confirmation = Some(PendingConfirmation::DefaultsReset);
    let _ = app.handle_shortcut_recording_started(KeybindingField::ToggleFloatingBadge);
    let _ = app.handle_window_escape_pressed();
    assert!(app.pending_confirmation.is_some());
    assert!(app.active_shortcut_recorder.is_some());
}

#[test]
fn conflict_cancel_leaves_the_draft_byte_for_byte() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let before = app.draft.clone();
    let _ = app.handle_shortcut_recording_started(KeybindingField::ToggleFloatingBadge);
    let _ = app.handle_shortcut_recorder_key(u32::from(b'e'), KeyboardModifiers::default());
    assert!(app.pending_shortcut_conflict.is_some());
    let _ = app.handle_shortcut_conflict_canceled();
    assert!(app.pending_shortcut_conflict.is_none());
    assert_eq!(app.draft, before);
}

#[test]
fn invalid_raw_text_blocks_save_and_stays_visible() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let _dir = load_config(
        &mut app,
        &format!("config_revision = {CURRENT_CONFIG_REVISION}\n"),
    );
    app.draft
        .keybindings
        .set(KeybindingField::Exit, "Ctrl+Shift".to_string());
    let effects = app.handle_save_requested();
    assert!(effects.is_empty());
    assert!(app.base_document.is_some());
    assert_eq!(
        app.draft.keybindings.value_for(KeybindingField::Exit),
        Some("Ctrl+Shift")
    );
    assert!(status_contains(&app.status, "keybindings.exit"));
}

#[test]
fn save_after_confirmed_replacement_writes_only_the_intended_fields() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let (_dir, path) = load_config(
        &mut app,
        &format!(
            "config_revision = {CURRENT_CONFIG_REVISION}\n\n# keep me\n[keybindings]\nundo = [\"Ctrl+Alt+U\"]\n# trailing\nunknown_future = true\n"
        ),
    );
    let _ = app.handle_shortcut_recording_started(KeybindingField::ClearCanvas);
    let _ = app.handle_shortcut_recorder_key(
        u32::from(b'u'),
        KeyboardModifiers {
            ctrl: true,
            shift: false,
            alt: true,
            super_held: false,
        },
    );
    assert!(app.pending_shortcut_conflict.is_some());
    let _ = app.handle_shortcut_conflict_replace_confirmed();
    save_draft(&mut app);

    let contents = std::fs::read_to_string(&path).expect("read saved config");
    assert!(contents.contains("# keep me"), "{contents}");
    assert!(contents.contains("unknown_future = true"), "{contents}");
    let revision = format!("config_revision = {CURRENT_CONFIG_REVISION}");
    assert_eq!(
        config_setting(&contents, "config_revision").as_deref(),
        Some(revision.as_str())
    );
    assert_eq!(
        config_setting(&contents, "clear_canvas").as_deref(),
        Some("clear_canvas = [ \"E\", \"Ctrl+Alt+U\", ]")
    );
    assert_eq!(
        config_setting(&contents, "undo").as_deref(),
        Some("undo = []")
    );
}

#[test]
fn pending_conflict_blocks_save_without_taking_the_document() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let _dir = load_config(
        &mut app,
        &format!("config_revision = {CURRENT_CONFIG_REVISION}\n"),
    );
    let _ = app.handle_shortcut_recording_started(KeybindingField::ToggleFloatingBadge);
    let _ = app.handle_shortcut_recorder_key(u32::from(b'e'), KeyboardModifiers::default());
    let effects = app.handle_save_requested();
    assert!(effects.is_empty());
    assert!(app.base_document.is_some());
    assert!(status_contains(&app.status, "shortcut conflict"));
}

#[test]
fn text_editor_keeps_invalid_text_until_canceled() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let before = app.draft.clone();
    let _ = app.handle_shortcut_text_edit_started(KeybindingField::Undo);
    let _ = app.handle_shortcut_text_edit_changed("Ctrl+Shift".to_string());
    let _ = app.handle_shortcut_text_edit_applied();
    assert_eq!(app.draft, before);
    assert!(app.shortcut_text_editor.is_some());
    let _ = app.handle_shortcut_text_edit_canceled(KeybindingField::Undo);
    assert!(app.shortcut_text_editor.is_none());
    assert_eq!(app.draft, before);
}

#[test]
fn recording_does_not_emit_save_config() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let effects = app.handle_shortcut_recording_started(KeybindingField::Undo);
    assert!(effects.is_empty());
    let effects = app.handle_shortcut_reset_requested(KeybindingField::Undo);
    assert!(effects.is_empty());
}

#[test]
fn keybinding_changed_still_updates_the_authored_string() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let effects = app.handle_keybinding_changed(KeybindingField::Undo, "F8".to_string());
    assert!(effects.is_empty());
    assert_eq!(
        app.draft.keybindings.value_for(KeybindingField::Undo),
        Some("F8")
    );
}

#[test]
fn resetting_an_edited_omitted_binding_restores_clean_default_source() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let _dir = load_config(
        &mut app,
        &format!("config_revision = {CURRENT_CONFIG_REVISION}\n"),
    );
    assert!(!app.draft.keybindings.is_authored(KeybindingField::Undo));

    let _ = app.handle_keybinding_changed(KeybindingField::Undo, "F8".to_string());
    assert!(app.is_dirty);
    assert!(app.draft.keybindings.is_authored(KeybindingField::Undo));

    let _ = app.handle_shortcut_reset_requested(KeybindingField::Undo);
    assert!(!app.is_dirty, "returning to the sparse baseline is clean");
    assert!(!app.draft.keybindings.is_authored(KeybindingField::Undo));
    assert_eq!(
        app.shortcut_manager_summary()
            .row(KeybindingField::Undo)
            .expect("undo summary")
            .badge_titles(),
        vec!["Default"]
    );
}

#[test]
fn resetting_an_explicit_binding_stays_authored_through_save_reload() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let (_dir, path) = load_config(
        &mut app,
        &format!("config_revision = {CURRENT_CONFIG_REVISION}\n\n[keybindings]\nundo = [\"F8\"]\n"),
    );
    assert!(app.draft.keybindings.is_authored(KeybindingField::Undo));

    let _ = app.handle_shortcut_reset_requested(KeybindingField::Undo);
    assert!(app.is_dirty);
    assert!(app.draft.keybindings.is_authored(KeybindingField::Undo));
    save_draft(&mut app);

    let contents = std::fs::read_to_string(path).expect("read saved config");
    assert_eq!(
        config_setting(&contents, "undo").as_deref(),
        Some("undo = [\"Ctrl+Z\"]")
    );
    assert!(app.draft.keybindings.is_authored(KeybindingField::Undo));
    assert_eq!(
        app.shortcut_manager_summary()
            .row(KeybindingField::Undo)
            .expect("undo summary")
            .badge_titles(),
        vec!["Authored"]
    );
}

#[test]
fn typing_an_edited_omitted_binding_back_to_baseline_restores_clean_source() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let _dir = load_config(
        &mut app,
        &format!("config_revision = {CURRENT_CONFIG_REVISION}\n"),
    );

    let _ = app.handle_keybinding_changed(KeybindingField::Undo, "F8".to_string());
    let _ = app.handle_keybinding_changed(KeybindingField::Undo, "Ctrl+Z".to_string());

    assert!(!app.is_dirty, "an exact revert matches the sparse baseline");
    assert!(!app.draft.keybindings.is_authored(KeybindingField::Undo));
}

#[test]
fn active_confirmation_canceled_clears_defaults_without_touching_conflicts() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.pending_confirmation = Some(PendingConfirmation::DefaultsReset);
    let _ = app.handle_shortcut_recording_started(KeybindingField::ToggleFloatingBadge);
    let _ = app.handle_shortcut_recorder_key(u32::from(b'e'), KeyboardModifiers::default());
    let effects = app.handle_active_confirmation_canceled();
    assert!(effects.is_empty());
    assert!(app.pending_confirmation.is_none());
    assert!(app.pending_shortcut_conflict.is_some());
}

#[test]
fn auxiliary_mouse_button_records_into_the_draft() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let _ = app.handle_shortcut_recording_started(KeybindingField::ToggleFloatingBadge);
    let effects = app.handle_shortcut_recorder_button(
        8,
        RecorderDeviceKind::Mouse,
        KeyboardModifiers::default(),
    );
    assert!(effects.is_empty());
    assert!(app.active_shortcut_recorder.is_none());
    assert_eq!(
        app.draft
            .keybindings
            .value_for(KeybindingField::ToggleFloatingBadge),
        Some("MouseBack")
    );
}

#[cfg(feature = "tablet-input")]
#[test]
fn recording_stylus_primary_prompts_to_move_the_default_legacy_barrel() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    assert_eq!(
        app.draft.keybindings.legacy_tablet.stylus_primary,
        Some(wayscriber::config::Action::ToggleRadialMenu)
    );
    let _ = app.handle_shortcut_recording_started(KeybindingField::Undo);
    let _ = app.handle_shortcut_recorder_button(
        2,
        RecorderDeviceKind::Pen,
        KeyboardModifiers::default(),
    );
    let pending = app
        .pending_shortcut_conflict
        .as_ref()
        .expect("legacy barrel is already assigned");
    assert_eq!(pending.replace_label(), "Move Legacy Binding");
    let effects = app.handle_shortcut_conflict_replace_confirmed();
    assert!(effects.is_empty());
    assert_eq!(app.draft.keybindings.legacy_tablet.stylus_primary, None);
    assert!(
        app.draft
            .keybindings
            .value_for(KeybindingField::Undo)
            .is_some_and(|value| value.contains("StylusPrimary"))
    );
}

#[test]
fn recording_a_two_step_sequence_commits_on_finish() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let _ = app.handle_shortcut_sequence_recording_started(KeybindingField::ToggleFloatingBadge);
    let chord = KeyboardModifiers {
        ctrl: true,
        shift: true,
        alt: true,
        super_held: false,
    };
    let effects = app.handle_shortcut_recorder_key(u32::from(b'k'), chord);
    assert!(effects.is_empty());
    assert!(app.active_shortcut_recorder.is_some());
    let effects = app.handle_shortcut_recorder_key(u32::from(b'c'), chord);
    assert!(effects.is_empty());
    assert!(
        app.active_shortcut_recorder
            .as_ref()
            .is_some_and(|recorder| recorder.can_finish())
    );
    let effects = app.handle_shortcut_sequence_finish();
    assert!(effects.is_empty());
    assert!(app.active_shortcut_recorder.is_none());
    assert_eq!(
        app.draft
            .keybindings
            .value_for(KeybindingField::ToggleFloatingBadge),
        Some("Ctrl+Shift+Alt+K > Ctrl+Shift+Alt+C")
    );
}

#[test]
fn third_sequence_step_finishes_automatically() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let _ = app.handle_shortcut_sequence_recording_started(KeybindingField::ToggleFloatingBadge);
    let chord = KeyboardModifiers {
        ctrl: true,
        shift: true,
        alt: true,
        super_held: false,
    };
    let _ = app.handle_shortcut_recorder_key(u32::from(b'k'), chord);
    let _ = app.handle_shortcut_recorder_key(u32::from(b'c'), chord);
    let effects = app.handle_shortcut_recorder_key(u32::from(b'v'), chord);
    assert!(effects.is_empty());
    assert!(app.active_shortcut_recorder.is_none());
    assert_eq!(
        app.draft
            .keybindings
            .value_for(KeybindingField::ToggleFloatingBadge),
        Some("Ctrl+Shift+Alt+K > Ctrl+Shift+Alt+C > Ctrl+Shift+Alt+V")
    );
}

#[test]
fn sequence_recording_rejects_device_buttons() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let _ = app.handle_shortcut_sequence_recording_started(KeybindingField::ToggleFloatingBadge);
    let effects = app.handle_shortcut_recorder_button(
        8,
        RecorderDeviceKind::Mouse,
        KeyboardModifiers::default(),
    );
    assert!(effects.is_empty());
    assert!(app.active_shortcut_recorder.is_some());
    assert!(
        app.active_shortcut_recorder
            .as_ref()
            .is_some_and(|recorder| recorder.prompt.contains("keyboard-only"))
    );
}

#[test]
fn sequence_prefix_conflict_uses_the_same_replace_flow() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let _ = app.handle_shortcut_sequence_recording_started(KeybindingField::ToggleFloatingBadge);
    let ctrl = KeyboardModifiers {
        ctrl: true,
        shift: false,
        alt: false,
        super_held: false,
    };
    let _ = app.handle_shortcut_recorder_key(u32::from(b'k'), ctrl);
    let _ = app.handle_shortcut_recorder_key(u32::from(b'c'), ctrl);
    let _ = app.handle_shortcut_sequence_finish();
    let pending = app
        .pending_shortcut_conflict
        .as_ref()
        .expect("Ctrl+K is the command palette");
    let prompt = pending.prompt();
    assert!(prompt.contains("Ctrl+K"), "{prompt}");
    assert!(
        prompt.contains("cannot coexist")
            || prompt.contains("Command Palette")
            || prompt.contains("command palette"),
        "{prompt}"
    );
}

#[test]
fn text_editor_accepts_a_sequence_beside_a_single() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let _ = app.handle_shortcut_text_edit_started(KeybindingField::ToggleFloatingBadge);
    let _ = app
        .handle_shortcut_text_edit_changed("F5, Ctrl+Alt+Shift+K > Ctrl+Alt+Shift+C".to_string());
    let effects = app.handle_shortcut_text_edit_applied();
    assert!(effects.is_empty());
    assert!(app.pending_shortcut_conflict.is_none());
    assert_eq!(
        app.draft
            .keybindings
            .value_for(KeybindingField::ToggleFloatingBadge),
        Some("F5, Ctrl+Alt+Shift+K > Ctrl+Alt+Shift+C")
    );
}

#[test]
fn search_filter_and_selection_do_not_dirty_the_config() {
    use crate::models::{ShortcutManagerFilter, ShortcutManagerSort};

    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.is_dirty = false;
    let _ = app.handle_search_changed("undo".to_string());
    let _ = app.handle_shortcut_manager_filter_changed(ShortcutManagerFilter::Changed);
    let _ = app.handle_shortcut_manager_sort_changed(ShortcutManagerSort::Name);
    let _ = app.handle_shortcut_manager_show_all();
    let _ = app.handle_shortcut_manager_row_selected(KeybindingField::Undo);
    assert!(!app.is_dirty);
    assert_eq!(app.selected_keybinding, Some(KeybindingField::Undo));
    assert!(app.keybindings_show_all);
}

#[test]
fn reset_visible_affects_exactly_the_filtered_identity_set() {
    use crate::models::ShortcutManagerFilter;

    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.is_loading = false;
    app.keybindings_show_all = true;
    app.draft
        .keybindings
        .set(KeybindingField::Undo, "F9".to_string());
    app.draft
        .keybindings
        .set(KeybindingField::ClearCanvas, "X".to_string());
    app.draft
        .keybindings
        .set(KeybindingField::Redo, "F4".to_string());
    app.baseline = app.draft.clone();
    app.is_dirty = false;
    let _ = app.handle_shortcut_manager_filter_changed(ShortcutManagerFilter::Changed);
    let visible = app.visible_keybinding_fields();
    assert!(visible.contains(&KeybindingField::Undo));
    assert!(visible.contains(&KeybindingField::ClearCanvas));
    assert!(visible.contains(&KeybindingField::Redo));

    let _ = app.handle_shortcut_reset_visible_requested();
    assert!(app.shortcut_reset_visible_pending());
    assert_eq!(
        app.draft.keybindings.value_for(KeybindingField::Undo),
        Some("F9")
    );

    let _ = app.handle_shortcut_reset_visible_requested();
    assert_eq!(
        app.draft.keybindings.value_for(KeybindingField::Undo),
        Some("F9"),
        "asking twice must not apply the reset"
    );

    let _ = app.handle_shortcut_reset_visible_confirmed();
    assert_eq!(
        app.draft.keybindings.value_for(KeybindingField::Undo),
        Some("Ctrl+Z")
    );
    assert_eq!(
        app.draft
            .keybindings
            .value_for(KeybindingField::ClearCanvas),
        Some("E")
    );
    assert_eq!(
        app.draft.keybindings.value_for(KeybindingField::Redo),
        Some("Ctrl+Shift+Z, Ctrl+Y")
    );
    assert!(app.is_dirty);
    assert!(app.pending_confirmation.is_none());
}

#[test]
fn reset_visible_does_not_touch_fields_outside_the_filter() {
    use crate::models::ShortcutManagerFilter;

    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.is_loading = false;
    app.keybindings_show_all = true;
    app.draft
        .keybindings
        .set(KeybindingField::Undo, "F9".to_string());
    app.draft
        .keybindings
        .set(KeybindingField::ClearCanvas, "X".to_string());
    let _ = app.handle_shortcut_manager_filter_changed(ShortcutManagerFilter::Unbound);
    app.draft
        .keybindings
        .set(KeybindingField::ToggleFloatingBadge, "".to_string());
    let _ = app.handle_shortcut_reset_visible_requested();
    let _ = app.handle_shortcut_reset_visible_confirmed();
    assert_eq!(
        app.draft.keybindings.value_for(KeybindingField::Undo),
        Some("F9"),
        "a changed bound action must survive an Unbound reset"
    );
    assert_eq!(
        app.draft
            .keybindings
            .value_for(KeybindingField::ClearCanvas),
        Some("X")
    );
}

#[test]
fn reset_all_requires_confirmation_and_stays_draft_only() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.is_loading = false;
    app.draft
        .keybindings
        .set(KeybindingField::Undo, "F9".to_string());
    app.draft
        .keybindings
        .set(KeybindingField::ClearCanvas, "X".to_string());
    app.baseline = app.draft.clone();
    app.is_dirty = false;
    let before = app.draft.keybindings.clone();

    let effects = app.handle_shortcut_reset_all_requested();
    assert!(effects.is_empty());
    assert!(app.shortcut_reset_all_pending());
    assert_eq!(app.draft.keybindings, before);

    let effects = app.handle_shortcut_reset_all_confirmed();
    assert!(effects.is_empty(), "reset all must not write the file");
    assert_eq!(
        app.draft.keybindings.value_for(KeybindingField::Undo),
        Some("Ctrl+Z")
    );
    assert_eq!(
        app.draft
            .keybindings
            .value_for(KeybindingField::ClearCanvas),
        Some("E")
    );
    assert!(app.is_dirty);
}

#[test]
fn reset_all_confirm_without_request_changes_nothing() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.is_loading = false;
    app.draft
        .keybindings
        .set(KeybindingField::Undo, "F9".to_string());
    let _ = app.handle_shortcut_reset_all_confirmed();
    assert_eq!(
        app.draft.keybindings.value_for(KeybindingField::Undo),
        Some("F9")
    );
}

#[test]
fn conflict_review_queue_arms_the_next_conflict_after_replace() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.is_loading = false;
    app.draft
        .keybindings
        .set(KeybindingField::ClearCanvas, "Ctrl+Shift+X".to_string());
    app.draft
        .keybindings
        .set(KeybindingField::ToggleToolbar, "Ctrl+Shift+X".to_string());
    app.draft
        .keybindings
        .set(KeybindingField::Undo, "Ctrl+Alt+Shift+Y".to_string());
    app.draft
        .keybindings
        .set(KeybindingField::Redo, "Ctrl+Alt+Shift+Y".to_string());

    let _ = app.handle_shortcut_conflict_review_started();
    assert!(app.shortcut_conflict_review);
    assert!(app.pending_shortcut_conflict.is_some());
    let first_target = match app.pending_shortcut_conflict.as_ref() {
        Some(crate::models::PendingShortcutConflict::Recorded { target, .. }) => *target,
        other => panic!("expected a recorded conflict, got {other:?}"),
    };

    let _ = app.handle_shortcut_conflict_replace_confirmed();
    assert!(
        app.shortcut_conflict_review,
        "the queue continues after one replace"
    );
    let second = app
        .pending_shortcut_conflict
        .as_ref()
        .expect("the next conflict should be armed");
    match second {
        crate::models::PendingShortcutConflict::Recorded { target, .. } => {
            assert_ne!(*target, first_target);
        }
        other => panic!("expected a recorded conflict, got {other:?}"),
    }
}

#[test]
fn conflict_review_cancel_stops_the_queue() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.draft
        .keybindings
        .set(KeybindingField::ClearCanvas, "Ctrl+Shift+X".to_string());
    app.draft
        .keybindings
        .set(KeybindingField::ToggleToolbar, "Ctrl+Shift+X".to_string());
    let _ = app.handle_shortcut_conflict_review_started();
    let _ = app.handle_shortcut_conflict_canceled();
    assert!(!app.shortcut_conflict_review);
    assert!(app.pending_shortcut_conflict.is_none());
}

#[test]
fn jump_to_conflict_selects_the_other_claimant() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.draft
        .keybindings
        .set(KeybindingField::ClearCanvas, "Ctrl+Shift+X".to_string());
    app.draft
        .keybindings
        .set(KeybindingField::ToggleToolbar, "Ctrl+Shift+X".to_string());
    let _ = app.handle_shortcut_conflict_review_started();
    let jump = app
        .pending_shortcut_conflict
        .as_ref()
        .and_then(crate::models::PendingShortcutConflict::jump_field)
        .expect("a claimant to jump to");
    let _ = app.handle_shortcut_manager_jump_to(jump);
    assert_eq!(app.selected_keybinding, Some(jump));
    assert_eq!(app.active_keybindings_tab, jump.tab());
    assert!(!app.is_dirty);
}
