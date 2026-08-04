//! Boards page: the templates a new session is seeded from.
//!
//! Boards are a user-sized list, so the per-board sections cannot be a fixed
//! set of [`PageBuilder`] rows. One container holds one `GtkListBox` per
//! board, and every section is built together with the closure that refreshes
//! it: that closure owns the section's typed widget handles and the signal
//! handler ids guarding each write, so a refresh never rediscovers a control
//! by position and never reports a programmatic write as a user edit.
//!
//! The binding owns both halves of that arrangement: the [`SectionLayout`]
//! list it last built, and one typed refresh closure per section. When the
//! layouts differ the two are replaced together, so a section and the values
//! handed to it can never drift apart — there is no count to check and no
//! mismatch to report. A rebuilt row gets its layout values before its
//! handlers connect, which is what keeps the first refresh from echoing the
//! model straight back at itself.
//!
//! Ids, names, and color text stay out of the layout on purpose: they change
//! on every keystroke, and rebuilding would tear the row the caret sits in out
//! from under it. Those values ride [`BoardValues`] into the sections' own
//! refresh closures, which write them with their handlers blocked — the hex
//! path is lossy in that direction, so an echo would quantize a board's float
//! components to whatever the 8-bit hex said.

mod color;
mod header;
mod rows;
mod section;
#[cfg(test)]
mod tests;

use relm4::prelude::*;
use relm4::{adw, gtk};

use crate::messages::Message;
use crate::models::{
    BoardBackgroundOption, ColorPickerId, ColorTripletInput, TabId, TextField, ToggleField,
};
use adw::prelude::*;

use super::super::search::{AppSearchSummary, SearchArea, TabSearchSummary};
use super::super::state::ConfiguratorApp;
use super::{BuiltPage, PageBuilder};
use rows::{is_collapsed, note_label, picker_hex, sync_combo, validate_min};
use section::{BoundBoardSection, rebuild_sections};

/// `GTK_INVALID_LIST_POSITION`: what a `GtkSingleSelection` reads as "nothing
/// is selected", which is how the Iced pick list rendered an unknown default.
const NO_SELECTION: u32 = u32::MAX;

const INTRO: &str = "Templates used to seed a new session. Boards you add or rename in the overlay belong to that session, not to this list.";

// Kept visible rather than hidden: the key is still in existing files, and a
// control that quietly disappears leaves the user to work out on their own why
// a setting they wrote stopped doing anything.
const PERSIST_NOTE: &str = "No effect: board renames, recolors, additions, and deletions belong to the running session, and nothing but Save writes config.toml. This setting is still parsed so older files load cleanly, and will be removed.";

const LEGACY_NOTE: &str =
    "Legacy [board] settings detected. Editing board settings will write [boards].";

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let mut page = PageBuilder::new(sender, TabId::Boards);

    // The list below is the seed, not the running session, so the heading has
    // to say which of the two this screen edits.
    page.group_in_area("Boards", SearchArea::BoardsGeneral);
    page.custom(&note_label(INTRO));

    page.group_in_area("", SearchArea::BoardsGeneral)
        .entry_row_validated(
            "Max boards",
            |app| app.draft.boards.max_count.clone(),
            |value| Message::TextChanged(TextField::BoardsMaxCount, value),
            |app| validate_min(&app.draft.boards.max_count, 1),
        )
        .switch_row(
            "Auto-create missing boards",
            "",
            |app| app.draft.boards.auto_create,
            |value| Message::ToggleChanged(ToggleField::BoardsAutoCreate, value),
        )
        .switch_row(
            "Show board badge",
            "",
            |app| app.draft.boards.show_board_badge,
            |value| Message::ToggleChanged(ToggleField::BoardsShowBadge, value),
        )
        .switch_row(
            "Persist runtime customizations (deprecated)",
            PERSIST_NOTE,
            |app| app.draft.boards.persist_customizations,
            |value| Message::ToggleChanged(ToggleField::BoardsPersistCustomizations, value),
        );

    add_default_board_row(&mut page);
    add_add_button(&mut page);
    add_legacy_note(&mut page);
    add_board_list(&mut page);

    page.finish()
}

/// The default-board picker: a combo whose model is the effective board ids,
/// which change as the user edits an id.
fn add_default_board_row(page: &mut PageBuilder) {
    let row = adw::ComboRow::builder().title("Default board").build();
    row.set_model(Some(&gtk::StringList::new(&[])));
    let handler = {
        let sender = page.sender();
        row.connect_selected_notify(move |row| {
            let Some(item) = row
                .selected_item()
                .and_then(|item| item.downcast::<gtk::StringObject>().ok())
            else {
                return;
            };
            sender.input(Message::BoardsDefaultChanged(item.string().to_string()));
        })
    };
    page.custom(&row);
    // The combo's own entries, kept by the binding that writes them: a model
    // is only replaced when the choices themselves changed.
    let mut shown: Vec<String> = Vec::new();
    page.bind(move |app, _summary| {
        let ids = app.draft.boards.effective_ids();
        let selected = ids
            .iter()
            .position(|id| *id == app.draft.boards.default_board);
        sync_combo(&row, &handler, &mut shown, &ids, selected);

        let has_boards = !ids.is_empty();
        if row.is_sensitive() != has_boards {
            row.set_sensitive(has_boards);
        }
        let subtitle = if has_boards {
            ""
        } else {
            "Add a board to choose a default"
        };
        if row.subtitle().as_deref() != Some(subtitle) {
            row.set_subtitle(subtitle);
        }
    });
}

fn add_add_button(page: &mut PageBuilder) {
    let button = gtk::Button::builder()
        .label("Add board")
        .halign(gtk::Align::Start)
        .margin_top(12)
        .build();
    {
        let sender = page.sender();
        button.connect_clicked(move |_| sender.input(Message::BoardsAddItem));
    }
    page.custom(&button);
}

fn add_legacy_note(page: &mut PageBuilder) {
    let label = note_label(LEGACY_NOTE);
    label.set_margin_top(6);
    // Hidden until a load reports a legacy file, so it cannot flash on the
    // first frame.
    label.set_visible(false);
    page.custom(&label);
    page.bind(move |app, _summary| {
        let visible = app
            .base_document
            .as_ref()
            .is_some_and(|document| document.config().boards.is_none());
        if label.is_visible() != visible {
            label.set_visible(visible);
        }
    });
}

/// The per-board sections, in their own untitled group so search can hide
/// them one at a time instead of as a block.
fn add_board_list(page: &mut PageBuilder) {
    page.group("");
    let container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .build();
    page.custom(&container);

    let sender = page.sender();
    // Each built section owns the layout that produced it and its typed refresh.
    let mut sections: Vec<BoundBoardSection> = Vec::new();
    page.bind(move |app, summary| {
        let layouts = section_layouts(app, summary);
        if !sections
            .iter()
            .map(|section| section.layout)
            .eq(layouts.iter().copied())
        {
            sections = rebuild_sections(&container, &layouts, &sender);
        }
        for ((index, item), section) in app
            .draft
            .boards
            .items
            .iter()
            .enumerate()
            .zip(sections.iter())
        {
            let values = BoardValues {
                id: &item.id,
                name: &item.name,
                background: ColorValues {
                    hex: picker_hex(app, ColorPickerId::BoardBackground(index)),
                    color: &item.background_color,
                },
                pen: ColorValues {
                    hex: picker_hex(app, ColorPickerId::BoardPen(index)),
                    color: &item.default_pen_color.color,
                },
            };
            (section.refresh)(&values);
        }
    });
}

// ---- What a rebuild depends on -----------------------------------------

/// Everything a board section shows before any text is written into it.
///
/// This is the whole rebuild trigger: the binding compares the list of these
/// it built against the list the model asks for now, so a control added here
/// rebuilds on its own value without anyone having to remember to extend a
/// format string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SectionLayout {
    visible: bool,
    expanded: bool,
    background_kind: BoardBackgroundOption,
    pen_enabled: bool,
    auto_adjust: bool,
    persist: bool,
    pinned: bool,
}

fn section_layouts(app: &ConfiguratorApp, summary: &AppSearchSummary) -> Vec<SectionLayout> {
    let tab = summary.tab(TabId::Boards);
    let show_all = tab.is_none_or(TabSearchSummary::show_all);
    let matched: &[usize] = tab.map_or(&[], TabSearchSummary::board_indices);

    app.draft
        .boards
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| SectionLayout {
            visible: show_all || matched.contains(&index),
            // A search that singled this board out expands it: a matched board
            // that stays folded shut is a match the user cannot read.
            expanded: !show_all || !is_collapsed(app, index),
            background_kind: item.background_kind,
            pen_enabled: item.default_pen_color.enabled,
            auto_adjust: item.auto_adjust_pen,
            persist: item.persist,
            pinned: item.pinned,
        })
        .collect()
}

/// One board row's values: everything the layout deliberately left out.
struct BoardValues<'a> {
    id: &'a str,
    name: &'a str,
    background: ColorValues<'a>,
    pen: ColorValues<'a>,
}

/// One color field's values: the picker's editing hex, and the draft's own
/// triplet, so the swatch parses the components by the same rules the draft
/// does.
struct ColorValues<'a> {
    hex: &'a str,
    color: &'a ColorTripletInput,
}
