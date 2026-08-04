//! History page: undo/redo delays and the optional custom section.
//!
//! The custom section is one `AdwExpanderRow`: its enable switch sends the
//! same `HistoryCustomSectionEnabled` toggle the Iced checkbox sent, and the
//! four fields it controls are its child rows, so turning the section off
//! folds them away instead of leaving them greyed out. Keeping switch and
//! fields in one row also keeps the group free for the search filter its
//! area owns — a second visibility binding on the group would overwrite it.

use relm4::adw;
use relm4::prelude::*;

use adw::prelude::*;

use crate::messages::Message;
use crate::models::{TabId, TextField, ToggleField};

use super::super::search::SearchArea;
use super::super::state::ConfiguratorApp;
use super::{BuiltPage, PageBuilder, set_text_blocked, validate_u32_range};

/// Delay bounds, in milliseconds, shared by every delay field here.
const DELAY_RANGE: (u32, u32) = (50, 5000);
/// Step-count bounds for the custom section's undo/redo counts.
const STEPS_RANGE: (u32, u32) = (1, 500);

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let mut page = PageBuilder::new(sender, TabId::History);

    page.group_in_area("Delays", SearchArea::HistoryMain)
        .entry_row_validated(
            "Undo all delay (ms)",
            |app| app.draft.history_undo_all_delay_ms.clone(),
            |value| Message::TextChanged(TextField::HistoryUndoAllDelayMs, value),
            |app| {
                validate_u32_range(
                    &app.draft.history_undo_all_delay_ms,
                    DELAY_RANGE.0,
                    DELAY_RANGE.1,
                )
            },
        )
        .entry_row_validated(
            "Redo all delay (ms)",
            |app| app.draft.history_redo_all_delay_ms.clone(),
            |value| Message::TextChanged(TextField::HistoryRedoAllDelayMs, value),
            |app| {
                validate_u32_range(
                    &app.draft.history_redo_all_delay_ms,
                    DELAY_RANGE.0,
                    DELAY_RANGE.1,
                )
            },
        );

    page.group_in_area("Custom section", SearchArea::HistoryCustom);
    let section = custom_section(&mut page);
    custom_field(
        &mut page,
        &section,
        "Custom undo delay (ms)",
        TextField::HistoryCustomUndoDelayMs,
        |app| app.draft.history_custom_undo_delay_ms.clone(),
        DELAY_RANGE,
    );
    custom_field(
        &mut page,
        &section,
        "Custom redo delay (ms)",
        TextField::HistoryCustomRedoDelayMs,
        |app| app.draft.history_custom_redo_delay_ms.clone(),
        DELAY_RANGE,
    );
    custom_field(
        &mut page,
        &section,
        "Custom undo steps",
        TextField::HistoryCustomUndoSteps,
        |app| app.draft.history_custom_undo_steps.clone(),
        STEPS_RANGE,
    );
    custom_field(
        &mut page,
        &section,
        "Custom redo steps",
        TextField::HistoryCustomRedoSteps,
        |app| app.draft.history_custom_redo_steps.clone(),
        STEPS_RANGE,
    );

    page.finish()
}

/// The expander whose enable switch is the custom-section toggle.
fn custom_section(page: &mut PageBuilder) -> adw::ExpanderRow {
    // Starts folded, the shipped default: a switch that opens agreeing with
    // the draft keeps the first refresh from echoing a toggle back.
    let row = adw::ExpanderRow::builder()
        .title("Enable custom undo/redo section")
        .show_enable_switch(true)
        .enable_expansion(false)
        .build();
    let handler = {
        let sender = page.sender();
        row.connect_enable_expansion_notify(move |row| {
            sender.input(Message::ToggleChanged(
                ToggleField::HistoryCustomSectionEnabled,
                row.enables_expansion(),
            ));
        })
    };
    page.custom(&row);
    {
        let row = row.clone();
        page.bind(move |app, _summary| {
            let enabled = app.draft.history_custom_section_enabled;
            if row.enables_expansion() != enabled {
                // Blocked: the draft decided this, and reporting it back as a
                // user toggle resets the status line — which after a load is
                // carrying that load's diagnostics.
                row.block_signal(&handler);
                row.set_enable_expansion(enabled);
                row.unblock_signal(&handler);
            }
        });
    }
    row
}

/// One field of the custom section, nested in `section` rather than in the
/// group, so it follows the enable switch.
///
/// The Iced view stopped validating these while the section was off: a
/// disabled section's leftover text is not an error the user can act on.
fn custom_field(
    page: &mut PageBuilder,
    section: &adw::ExpanderRow,
    title: &str,
    field: TextField,
    get: impl Fn(&ConfiguratorApp) -> String + 'static,
    range: (u32, u32),
) {
    let row = adw::EntryRow::builder().title(title).build();
    let handler = {
        let sender = page.sender();
        row.connect_changed(move |row| {
            sender.input(Message::TextChanged(field, row.text().to_string()));
        })
    };
    section.add_row(&row);
    page.bind(move |app, _summary| {
        let value = get(app);
        // Blocked: the model owns this text, and a load reporting its own
        // value back as a user edit clears that load's diagnostics.
        set_text_blocked(&row, &handler, &value);
        let error = app
            .draft
            .history_custom_section_enabled
            .then(|| validate_u32_range(&value, range.0, range.1))
            .flatten();
        show_error(&row, error);
    });
}

/// Marks a row `.error` with the reason as its tooltip, the way
/// `PageBuilder::entry_row_validated` marks the rows it owns.
fn show_error(row: &adw::EntryRow, error: Option<String>) {
    let has_error_class = row.has_css_class("error");
    match error {
        Some(message) => {
            if !has_error_class {
                row.add_css_class("error");
            }
            if row.tooltip_text().as_deref() != Some(message.as_str()) {
                row.set_tooltip_text(Some(&message));
            }
        }
        None => {
            if has_error_class {
                row.remove_css_class("error");
                row.set_tooltip_text(None);
            }
        }
    }
}
