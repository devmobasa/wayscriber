use wayscriber::config::{
    ConfigDiagnosticKind, ConfigDocument, ConfigValidationReport, InvalidKeybinding,
    KeybindingConflictResolution, MigrationPreview,
};

use super::super::super::state::StatusMessage;

const SHOWN_DIAGNOSTICS: usize = 8;

/// Why a save was refused before it began.
///
/// The count is all the banner can give: which rows are at fault is the row's
/// own job to show, and the fix is the same for every one of them — type a
/// color. Clearing the field is not a way out: the picker edits a color the
/// config requires, so an empty field is refused like any other text that is
/// not a color.
pub(super) fn invalid_color_hex_message(count: usize) -> String {
    if count == 1 {
        return "1 color field does not hold a color. Enter #RRGGBB or #RRGGBBAA before saving."
            .to_string();
    }
    format!("{count} color fields do not hold a color. Enter #RRGGBB or #RRGGBBAA before saving.")
}

pub(super) fn config_document_status(document: &ConfigDocument, success: &str) -> StatusMessage {
    let diagnostics = document.diagnostics();
    if diagnostics.is_empty() {
        return StatusMessage::success(success);
    }

    let mut message = success.to_string();
    let mut unknown = Vec::new();
    let mut retired = Vec::new();
    let mut conflicts = Vec::new();
    let mut invalid = Vec::new();
    let mut skipped_defaults = Vec::new();
    // Exhaustive on purpose: a kind with no section here would leave the
    // status warning-styled and empty of the very thing it is warning about,
    // so a new variant has to be a compile error rather than a silent drop.
    for diagnostic in diagnostics {
        match diagnostic.kind() {
            ConfigDiagnosticKind::UnknownSetting => unknown.push(diagnostic.path().to_string()),
            ConfigDiagnosticKind::RetiredSetting => retired.push(diagnostic.path().to_string()),
            // Every keybinding kind is resolved in memory only, so the file
            // the editor is showing still contains them: carry the diagnostic's
            // own wording, which names the actions, instead of just the path.
            ConfigDiagnosticKind::KeybindingConflict => conflicts.push(diagnostic.to_string()),
            ConfigDiagnosticKind::InvalidKeybinding => invalid.push(diagnostic.to_string()),
            ConfigDiagnosticKind::DefaultShortcutSkipped => {
                skipped_defaults.push(diagnostic.to_string());
            }
        }
    }

    if !unknown.is_empty() {
        message.push_str(&format!(
            "\nUnrecognized settings were preserved: {}.",
            list_with_overflow(&borrowed(&unknown), ", ")
        ));
    }
    if !retired.is_empty() {
        message.push_str(&format!(
            "\nRetired toolbar settings were preserved but are no longer used and can be removed: {}.",
            list_with_overflow(&borrowed(&retired), ", ")
        ));
    }
    if !invalid.is_empty() {
        message.push_str(&format!(
            "\nShortcuts that could not be parsed are ignored for the running session; the file still has them: {}.",
            list_with_overflow(&borrowed(&invalid), "; ")
        ));
    }
    if !conflicts.is_empty() {
        message.push_str(&format!(
            "\nConflicting shortcuts were resolved for the running session only; the file still has them: {}.",
            list_with_overflow(&borrowed(&conflicts), "; ")
        ));
    }
    // Its own sentence, and the last one: nothing in the file is wrong here.
    // An action this configuration never mentions was offered a shortcut this
    // build added, and the configuration already spends that key.
    if !skipped_defaults.is_empty() {
        message.push_str(&format!(
            "\nNew default shortcuts stayed inactive because this configuration already uses those keys: {}.",
            list_with_overflow(&borrowed(&skipped_defaults), "; ")
        ));
    }

    StatusMessage::warning(message)
}

/// What validating the saved configuration changed in the shortcuts the user
/// typed, or `None` when it changed nothing.
///
/// The load-time sentences in [`config_document_status`] all end in "the file
/// still has them", because loading resolves in memory only. These are the
/// other case: the draft is the authored text, the resolution is what reached
/// `config.toml`, and the reloaded document no longer contains the collision
/// to report. Naming which action kept the key and which lost it is therefore
/// the only account the user gets of an edit their Save made for them.
///
/// A skipped default cannot appear here: the draft spells every action out
/// (`ConfigDraft::to_config` marks the section explicit), so the omitted-default
/// pass has nothing to offer and reports nothing.
pub(super) fn save_validation_note(validation: &ConfigValidationReport) -> Option<String> {
    // The summaries, not the full `Display` forms: those say the file keeps
    // the shortcut and the session does without it, which is the load story.
    let invalid = clauses(
        validation
            .invalid_keybindings
            .iter()
            .map(InvalidKeybinding::summary),
    );
    let conflicts = clauses(
        validation
            .keybinding_conflicts
            .iter()
            .map(KeybindingConflictResolution::summary),
    );
    if invalid.is_empty() && conflicts.is_empty() {
        return None;
    }

    let mut note = String::new();
    if !invalid.is_empty() {
        note.push_str(&format!(
            "Shortcuts that could not be parsed were left out of the saved configuration: {}.",
            list_with_overflow(&borrowed(&invalid), "; ")
        ));
    }
    if !conflicts.is_empty() {
        if !note.is_empty() {
            note.push('\n');
        }
        note.push_str(&format!(
            "Shortcuts two actions claimed were settled before saving, and the saved configuration keeps that outcome: {}.",
            list_with_overflow(&borrowed(&conflicts), "; ")
        ));
    }
    Some(note)
}

/// Toast-sized summaries as list items: each is a finished sentence, and the
/// sentence they are listed inside supplies the final stop.
fn clauses(summaries: impl Iterator<Item = String>) -> Vec<String> {
    summaries
        .map(|summary| summary.trim_end_matches('.').to_string())
        .collect()
}

fn borrowed(entries: &[String]) -> Vec<&str> {
    entries.iter().map(String::as_str).collect()
}

pub(super) fn list_with_overflow(entries: &[&str], separator: &str) -> String {
    let shown = entries
        .iter()
        .take(SHOWN_DIAGNOSTICS)
        .copied()
        .collect::<Vec<_>>()
        .join(separator);
    match entries.len().saturating_sub(SHOWN_DIAGNOSTICS) {
        0 => shown,
        remaining => format!("{shown}{separator}and {remaining} more"),
    }
}

/// The migration offer as the banner shows it, with its whole change list in
/// view.
///
/// The list is not behind a Review button: a recipe proposes at most a handful
/// of shortcuts, and putting Apply next to something the user has not read yet
/// is the one thing this flow exists to avoid.
pub(crate) fn migration_offer_text(preview: &MigrationPreview) -> String {
    let mut lines = vec![
        "Configuration update available".to_string(),
        format!(
            "Shortcut defaults changed since this configuration was written. Applying updates this draft only; nothing reaches the file until you press Save, which also records revision {}.",
            preview.proposed_revision()
        ),
    ];
    for change in preview.changes() {
        lines.push(format!(
            "{} ({}): {} → {}",
            change.action_label(),
            change.config_key(),
            binding_summary(change.before()),
            binding_summary(change.after()),
        ));
    }
    lines.join("\n")
}

fn binding_summary(bindings: &[String]) -> String {
    if bindings.is_empty() {
        return "unbound".to_string();
    }
    bindings.join(", ")
}
