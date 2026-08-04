//! Session persistence, storage, and autosave settings.

use relm4::{adw, gtk};

use adw::prelude::*;

use crate::messages::Message;
use crate::models::{SessionCompressionOption, SessionStorageModeOption, TextField, ToggleField};

use super::super::super::search::SearchArea;
use super::super::{PageBuilder, set_text_blocked};

pub(super) fn add(page: &mut PageBuilder) {
    page.group_in_area("Session Persistence", SearchArea::SessionPersistence)
        .switch_row(
            "Persist transparent mode drawings",
            "",
            |app| app.draft.session_persist_transparent,
            |value| Message::ToggleChanged(ToggleField::SessionPersistTransparent, value),
        )
        .switch_row(
            "Persist whiteboard mode drawings",
            "",
            |app| app.draft.session_persist_whiteboard,
            |value| Message::ToggleChanged(ToggleField::SessionPersistWhiteboard, value),
        )
        .switch_row(
            "Persist blackboard mode drawings",
            "",
            |app| app.draft.session_persist_blackboard,
            |value| Message::ToggleChanged(ToggleField::SessionPersistBlackboard, value),
        )
        .switch_row(
            "Persist undo/redo history",
            "",
            |app| app.draft.session_persist_history,
            |value| Message::ToggleChanged(ToggleField::SessionPersistHistory, value),
        )
        .switch_row(
            "Restore tool state on startup",
            "",
            |app| app.draft.session_restore_tool_state,
            |value| Message::ToggleChanged(ToggleField::SessionRestoreToolState, value),
        )
        .switch_row(
            "Per-output persistence",
            "",
            |app| app.draft.session_per_output,
            |value| Message::ToggleChanged(ToggleField::SessionPerOutput, value),
        );

    page.group_in_area("Storage", SearchArea::SessionPersistence)
        .combo_row(
            "Storage mode",
            "",
            SessionStorageModeOption::list(),
            option_labels(SessionStorageModeOption::list(), |option| option.label()),
            |app| app.draft.session_storage_mode,
            Message::SessionStorageModeChanged,
        );
    custom_directory_row(page);
    page.combo_row(
        "Compression",
        "",
        SessionCompressionOption::list(),
        option_labels(SessionCompressionOption::list(), |option| option.label()),
        |app| app.draft.session_compression,
        Message::SessionCompressionChanged,
    )
    .entry_row_validated(
        "Max shapes per frame",
        |app| app.draft.session_max_shapes_per_frame.clone(),
        |value| Message::TextChanged(TextField::SessionMaxShapesPerFrame, value),
        |app| validate_whole_number(&app.draft.session_max_shapes_per_frame, 1, u64::MAX),
    )
    .entry_row(
        "Max persisted undo depth (blank = runtime limit)",
        |app| app.draft.session_max_persisted_undo_depth.clone(),
        |value| Message::TextChanged(TextField::SessionMaxPersistedUndoDepth, value),
    )
    .entry_row_validated(
        "Max file size (MB)",
        |app| app.draft.session_max_file_size_mb.clone(),
        |value| Message::TextChanged(TextField::SessionMaxFileSizeMb, value),
        |app| validate_whole_number(&app.draft.session_max_file_size_mb, 1, 1024),
    )
    .entry_row_validated(
        "Auto-compress threshold (KB)",
        |app| app.draft.session_auto_compress_threshold_kb.clone(),
        |value| Message::TextChanged(TextField::SessionAutoCompressThresholdKb, value),
        |app| validate_whole_number(&app.draft.session_auto_compress_threshold_kb, 1, u64::MAX),
    )
    .entry_row(
        "Backup retention count",
        |app| app.draft.session_backup_retention.clone(),
        |value| Message::TextChanged(TextField::SessionBackupRetention, value),
    );

    page.group_in_area("Autosave", SearchArea::SessionPersistence)
        .switch_row(
            "Enable autosave",
            "",
            |app| app.draft.session_autosave_enabled,
            |value| Message::ToggleChanged(ToggleField::SessionAutosaveEnabled, value),
        )
        .entry_row_validated(
            "Autosave idle (ms)",
            |app| app.draft.session_autosave_idle_ms.clone(),
            |value| Message::TextChanged(TextField::SessionAutosaveIdleMs, value),
            |app| validate_whole_number(&app.draft.session_autosave_idle_ms, 1000, u64::MAX),
        )
        .entry_row_validated(
            "Autosave interval (ms)",
            |app| app.draft.session_autosave_interval_ms.clone(),
            |value| Message::TextChanged(TextField::SessionAutosaveIntervalMs, value),
            |app| validate_whole_number(&app.draft.session_autosave_interval_ms, 1000, u64::MAX),
        )
        .entry_row_validated(
            "Autosave failure backoff (ms)",
            |app| app.draft.session_autosave_failure_backoff_ms.clone(),
            |value| Message::TextChanged(TextField::SessionAutosaveFailureBackoffMs, value),
            |app| {
                validate_whole_number(
                    &app.draft.session_autosave_failure_backoff_ms,
                    1000,
                    u64::MAX,
                )
            },
        );
}

fn option_labels<O: Copy>(options: Vec<O>, label: impl Fn(&O) -> &'static str) -> Vec<String> {
    options
        .iter()
        .map(|option| label(option).to_string())
        .collect()
}

/// The custom directory only applies to one storage mode, so it is a row the
/// mode shows rather than a row that is always there and usually inert.
fn custom_directory_row(page: &mut PageBuilder) {
    let row = adw::EntryRow::builder().title("Custom directory").build();
    let handler = {
        let sender = page.sender();
        row.connect_changed(move |row| {
            sender.input(Message::TextChanged(
                TextField::SessionCustomDirectory,
                row.text().to_string(),
            ));
        })
    };
    page.custom(&row);
    page.bind(move |app, _summary| {
        set_visible(
            &row,
            app.draft.session_storage_mode == SessionStorageModeOption::Custom,
        );
        // Blocked: the model owns this text, and a load reporting its own
        // value back as a user edit clears the diagnostics that load produced.
        set_text_blocked(&row, &handler, &app.draft.session_custom_directory);
    });
}

/// Error text for a whole-number field, `None` while the input is
/// acceptable. `u64::MAX` as the upper bound means "no maximum" and reports
/// only the minimum, matching the legacy view's two validators.
fn validate_whole_number(value: &str, min: u64, max: u64) -> Option<String> {
    let Ok(parsed) = value.trim().parse::<u64>() else {
        return Some("Expected a whole number".to_string());
    };
    if (min..=max).contains(&parsed) {
        return None;
    }
    Some(if max == u64::MAX {
        format!("Minimum: {min}")
    } else {
        format!("Range: {min}-{max}")
    })
}

fn set_visible(widget: &impl IsA<gtk::Widget>, visible: bool) {
    if widget.get_visible() != visible {
        widget.set_visible(visible);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_number_validation_matches_the_old_hints() {
        assert_eq!(validate_whole_number("1000", 1000, u64::MAX), None);
        assert_eq!(
            validate_whole_number("999", 1000, u64::MAX).as_deref(),
            Some("Minimum: 1000")
        );
        assert_eq!(
            validate_whole_number("", 1, u64::MAX).as_deref(),
            Some("Expected a whole number")
        );
        assert_eq!(
            validate_whole_number("-4", 1, u64::MAX).as_deref(),
            Some("Expected a whole number")
        );
        assert_eq!(validate_whole_number(" 512 ", 1, 1024), None);
        assert_eq!(
            validate_whole_number("2048", 1, 1024).as_deref(),
            Some("Range: 1-1024")
        );
    }
}
