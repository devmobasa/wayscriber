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

use relm4::prelude::*;
use relm4::{adw, gtk};

use adw::prelude::*;
use gtk::glib::SignalHandlerId;

use crate::messages::Message;
use crate::models::color::parse_triplet_values;
use crate::models::{
    BoardBackgroundOption, BoardItemTextField, BoardItemToggleField, ColorPickerId,
    ColorTripletInput, TabId, TextField, ToggleField,
};

use super::super::search::{AppSearchSummary, SearchArea, TabSearchSummary};
use super::super::state::ConfiguratorApp;
use super::color_rows::{dialog_hex, mark_hex_error, set_swatch_blocked};
use super::{BuiltPage, PageBuilder, set_text_blocked};

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

// ---- Sections ----------------------------------------------------------

/// One section's refresh: built beside its row, so it owns that row's typed
/// widget handles and the signal handler ids guarding each write. No
/// positional lookup, and no widget the refresh can silently miss.
type BoardRowRefresh = Box<dyn Fn(&BoardValues<'_>)>;

struct BoundBoardSection {
    layout: SectionLayout,
    refresh: BoardRowRefresh,
}

/// One board's section: the list box, and the closure that writes the values
/// the layout deliberately left out.
struct BoardSection {
    section: gtk::ListBox,
    refresh: BoardRowRefresh,
}

fn rebuild_sections(
    container: &gtk::Box,
    layouts: &[SectionLayout],
    sender: &ComponentSender<ConfiguratorApp>,
) -> Vec<BoundBoardSection> {
    // Draining the container, not walking it for a control: the sections that
    // replace these carry their own refresh closures.
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }

    let mut sections = Vec::with_capacity(layouts.len());
    for (index, layout) in layouts.iter().enumerate() {
        let built = build_section(index, *layout, sender);
        container.append(&built.section);
        sections.push(BoundBoardSection {
            layout: *layout,
            refresh: built.refresh,
        });
    }
    sections
}

fn build_section(
    index: usize,
    layout: SectionLayout,
    sender: &ComponentSender<ConfiguratorApp>,
) -> BoardSection {
    let section = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .visible(layout.visible)
        .build();

    let header = build_header_row(index, layout.expanded, sender);
    section.append(&header.row);

    let id = build_text_row("Board id", index, BoardItemTextField::Id, sender);
    id.row.set_visible(layout.expanded);
    section.append(&id.row);

    let name = build_text_row("Display name", index, BoardItemTextField::Name, sender);
    name.row.set_visible(layout.expanded);
    section.append(&name.row);

    let kind_row = build_kind_row(index, layout.background_kind, sender);
    kind_row.set_visible(layout.expanded);
    section.append(&kind_row);

    let background = build_color_row(
        "Background color (0-1)",
        ColorPickerId::BoardBackground(index),
        index,
        Message::BoardsBackgroundColorChanged,
        sender,
    );
    // The Iced view swapped this for a note explaining why the color is
    // inert; a hidden row says the same thing without the clutter.
    background
        .row
        .set_visible(layout.expanded && layout.background_kind == BoardBackgroundOption::Color);
    section.append(&background.row);

    let pen_enabled = adw::SwitchRow::builder()
        .title("Override default pen color")
        .active(layout.pen_enabled)
        .visible(layout.expanded)
        .build();
    {
        let sender = sender.clone();
        pen_enabled.connect_active_notify(move |row| {
            sender.input(Message::BoardsDefaultPenEnabledChanged(
                index,
                row.is_active(),
            ));
        });
    }
    section.append(&pen_enabled);

    let pen = build_color_row(
        "Pen color (0-1)",
        ColorPickerId::BoardPen(index),
        index,
        Message::BoardsDefaultPenColorChanged,
        sender,
    );
    pen.row.set_visible(layout.expanded && layout.pen_enabled);
    section.append(&pen.row);

    for (title, field, active) in [
        (
            "Auto-adjust pen",
            BoardItemToggleField::AutoAdjustPen,
            layout.auto_adjust,
        ),
        ("Persist", BoardItemToggleField::Persist, layout.persist),
        (
            "Configured default pinned",
            BoardItemToggleField::Pinned,
            layout.pinned,
        ),
    ] {
        section.append(&build_toggle_row(
            title,
            index,
            field,
            active,
            layout.expanded,
            sender,
        ));
    }

    let refresh: BoardRowRefresh = Box::new(move |values| {
        header.set_labels(index, values.id, values.name);
        set_text_blocked(&id.row, &id.handler, values.id);
        set_text_blocked(&name.row, &name.handler, values.name);
        background.refresh(&values.background);
        pen.refresh(&values.pen);
    });

    BoardSection { section, refresh }
}

// ---- Header row --------------------------------------------------------

/// The header's two labels, which restate the id and name rows below them.
struct HeaderRow {
    row: gtk::ListBoxRow,
    title: gtk::Label,
    id: gtk::Label,
}

impl HeaderRow {
    fn set_labels(&self, index: usize, id: &str, name: &str) {
        let title = if name.trim().is_empty() {
            format!("Board {}", index + 1)
        } else {
            name.trim().to_string()
        };
        set_label(&self.title, &title);

        let id_label = if id.trim().is_empty() {
            "id: <unset>".to_string()
        } else {
            format!("id: {}", id.trim())
        };
        set_label(&self.id, &id_label);
    }
}

fn build_header_row(
    index: usize,
    expanded: bool,
    sender: &ComponentSender<ConfiguratorApp>,
) -> HeaderRow {
    let labels = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .hexpand(true)
        .build();
    let title = gtk::Label::builder()
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    let id = gtk::Label::builder()
        .xalign(0.0)
        .css_classes(["dim-label", "caption"])
        .build();
    labels.append(&title);
    labels.append(&id);

    let content = row_content_box(gtk::Orientation::Horizontal);
    content.append(&labels);
    for (label, message) in [
        (
            if expanded { "Collapse" } else { "Expand" },
            Message::BoardsCollapseToggled(index),
        ),
        ("Up", Message::BoardsMoveItemUp(index)),
        ("Down", Message::BoardsMoveItemDown(index)),
        ("Duplicate", Message::BoardsDuplicateItem(index)),
        ("Remove", Message::BoardsRemoveItem(index)),
    ] {
        let button = gtk::Button::builder()
            .label(label)
            .valign(gtk::Align::Center)
            .build();
        connect_button(&button, message, sender);
        content.append(&button);
    }

    HeaderRow {
        row: plain_row(&content),
        title,
        id,
    }
}

// ---- Color row ---------------------------------------------------------

/// The Iced view's triplet picker: a hex field, a popup picker, and the three
/// raw components. The native color dialog stands in for the popup; both feed
/// the same `ColorPickerHexChanged` path the popup's hex field used.
struct ColorRow {
    row: gtk::ListBoxRow,
    hex: gtk::Entry,
    hex_handler: SignalHandlerId,
    swatch: gtk::ColorDialogButton,
    swatch_handler: SignalHandlerId,
    components: [ComponentEntry; 3],
}

struct ComponentEntry {
    entry: gtk::Entry,
    handler: SignalHandlerId,
}

const COMPONENT_PLACEHOLDERS: [&str; 3] = ["R", "G", "B"];

impl ColorRow {
    fn refresh(&self, values: &ColorValues<'_>) {
        set_text_blocked(&self.hex, &self.hex_handler, values.hex);
        // The same predicate the save gate counts with, so a field styled
        // clean can never be one the save refuses.
        mark_hex_error(&self.hex, values.hex);

        for (component, value) in self.components.iter().zip(values.color.components.iter()) {
            set_text_blocked(&component.entry, &component.handler, value);
        }

        let rgb = parse_triplet_values(&values.color.components);
        let rgba = gtk::gdk::RGBA::new(rgb[0] as f32, rgb[1] as f32, rgb[2] as f32, 1.0);
        set_swatch_blocked(&self.swatch, &self.swatch_handler, &rgba);
    }
}

fn build_color_row(
    title: &str,
    id: ColorPickerId,
    index: usize,
    to_component: fn(usize, usize, String) -> Message,
    sender: &ComponentSender<ConfiguratorApp>,
) -> ColorRow {
    let controls = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();

    let hex = gtk::Entry::builder()
        .placeholder_text("#RRGGBB")
        .width_chars(9)
        .max_width_chars(9)
        .build();
    let hex_handler = {
        let sender = sender.clone();
        hex.connect_changed(move |entry| {
            sender.input(Message::ColorPickerHexChanged(id, entry.text().to_string()));
        })
    };
    controls.append(&hex);

    let swatch =
        gtk::ColorDialogButton::new(Some(gtk::ColorDialog::builder().with_alpha(false).build()));
    swatch.set_valign(gtk::Align::Center);
    let swatch_handler = {
        let sender = sender.clone();
        swatch.connect_rgba_notify(move |button| {
            sender.input(Message::ColorPickerHexChanged(
                id,
                dialog_hex(&button.rgba()),
            ));
        })
    };
    controls.append(&swatch);

    let components = std::array::from_fn(|component| {
        let entry = gtk::Entry::builder()
            .placeholder_text(COMPONENT_PLACEHOLDERS.get(component).copied().unwrap_or(""))
            .width_chars(6)
            .max_width_chars(6)
            .build();
        let handler = {
            let sender = sender.clone();
            entry.connect_changed(move |entry| {
                sender.input(to_component(index, component, entry.text().to_string()));
            })
        };
        controls.append(&entry);
        ComponentEntry { entry, handler }
    });

    let content = row_content_box(gtk::Orientation::Vertical);
    content.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .css_classes(["dim-label", "caption"])
            .build(),
    );
    content.append(&controls);

    ColorRow {
        row: plain_row(&content),
        hex,
        hex_handler,
        swatch,
        swatch_handler,
        components,
    }
}

// ---- Small shared plumbing --------------------------------------------

/// An entry row whose text the model owns, kept with the handler a refresh
/// has to block before writing it.
struct TextRow {
    row: adw::EntryRow,
    handler: SignalHandlerId,
}

fn build_text_row(
    title: &str,
    index: usize,
    field: BoardItemTextField,
    sender: &ComponentSender<ConfiguratorApp>,
) -> TextRow {
    let row = adw::EntryRow::builder().title(title).build();
    let handler = {
        let sender = sender.clone();
        row.connect_changed(move |row| {
            sender.input(Message::BoardsItemTextChanged(
                index,
                field,
                row.text().to_string(),
            ));
        })
    };
    TextRow { row, handler }
}

fn build_kind_row(
    index: usize,
    selected: BoardBackgroundOption,
    sender: &ComponentSender<ConfiguratorApp>,
) -> adw::ComboRow {
    let options = BoardBackgroundOption::list();
    let labels: Vec<String> = options
        .iter()
        .map(|option| option.label().to_string())
        .collect();
    let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let row = adw::ComboRow::builder()
        .title("Background")
        .model(&gtk::StringList::new(&label_refs))
        .build();

    // The selection is part of the rebuild fingerprint, so it is set before
    // the handler exists and never written again behind the user's back.
    if let Some(position) = options.iter().position(|option| *option == selected) {
        row.set_selected(position as u32);
    }
    {
        let sender = sender.clone();
        row.connect_selected_notify(move |row| {
            if let Some(option) = options.get(row.selected() as usize) {
                sender.input(Message::BoardsBackgroundKindChanged(index, *option));
            }
        });
    }
    row
}

fn build_toggle_row(
    title: &str,
    index: usize,
    field: BoardItemToggleField,
    active: bool,
    expanded: bool,
    sender: &ComponentSender<ConfiguratorApp>,
) -> adw::SwitchRow {
    let row = adw::SwitchRow::builder()
        .title(title)
        .active(active)
        .visible(expanded)
        .build();
    let sender = sender.clone();
    row.connect_active_notify(move |row| {
        sender.input(Message::BoardsItemToggleChanged(
            index,
            field,
            row.is_active(),
        ));
    });
    row
}

fn connect_button(
    button: &gtk::Button,
    message: Message,
    sender: &ComponentSender<ConfiguratorApp>,
) {
    let sender = sender.clone();
    button.connect_clicked(move |_| sender.input(message.clone()));
}

fn set_label(label: &gtk::Label, value: &str) {
    if label.text() != value {
        label.set_text(value);
    }
}

/// Rewrites a combo's model only when the choices themselves changed, and
/// writes both model and selection with the change handler blocked: replacing
/// a model resets the selection to the first row, which would otherwise be
/// reported as if the user had picked it.
///
/// `shown` is what the combo currently offers, owned by the binding that
/// calls this — the entries themselves, not a rendering of them.
fn sync_combo(
    row: &adw::ComboRow,
    handler: &SignalHandlerId,
    shown: &mut Vec<String>,
    entries: &[String],
    selected: Option<usize>,
) {
    let rebuild = shown.as_slice() != entries;
    let target = selected.map_or(NO_SELECTION, |index| index as u32);
    if !rebuild && row.selected() == target {
        return;
    }

    row.block_signal(handler);
    if rebuild {
        let refs: Vec<&str> = entries.iter().map(String::as_str).collect();
        row.set_model(Some(&gtk::StringList::new(&refs)));
        shown.clear();
        shown.extend_from_slice(entries);
    }
    if row.selected() != target {
        row.set_selected(target);
    }
    row.unblock_signal(handler);
}

fn row_content_box(orientation: gtk::Orientation) -> gtk::Box {
    gtk::Box::builder()
        .orientation(orientation)
        .spacing(6)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build()
}

fn plain_row(content: &impl IsA<gtk::Widget>) -> gtk::ListBoxRow {
    gtk::ListBoxRow::builder()
        .child(content)
        .activatable(false)
        .selectable(false)
        .build()
}

fn note_label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .wrap(true)
        .xalign(0.0)
        .css_classes(["dim-label", "caption"])
        .build()
}

fn picker_hex(app: &ConfiguratorApp, id: ColorPickerId) -> &str {
    app.color_picker_hex.get(&id).map_or("", String::as_str)
}

fn is_collapsed(app: &ConfiguratorApp, index: usize) -> bool {
    app.boards_collapsed.get(index).copied().unwrap_or(false)
}

/// Error text for a count field with a lower bound, worded as the Iced view
/// worded it.
fn validate_min(value: &str, min: usize) -> Option<String> {
    match value.trim().parse::<usize>() {
        Ok(parsed) if parsed >= min => None,
        Ok(_) => Some(format!("Minimum: {min}")),
        Err(_) => Some("Expected a whole number".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SearchQuery;

    /// Two boards, so a fingerprint covers more than one section.
    fn app_with_two_boards() -> ConfiguratorApp {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        while app.draft.boards.items.len() < 2 {
            let Some(item) = app.draft.boards.items.first().cloned() else {
                break;
            };
            app.draft.boards.items.push(item);
            app.boards_collapsed.push(false);
        }
        app
    }

    /// The law the binding rests on: one layout per board, so the sections it
    /// builds and the values it hands them are indexed by the same thing.
    #[test]
    fn there_is_one_layout_per_board() {
        let app = app_with_two_boards();
        let layouts = section_layouts(&app, &app.search_summary());

        assert_eq!(layouts.len(), app.draft.boards.items.len());
    }

    /// The caret guarantee, stated as the layout law it rests on: no keystroke
    /// in a board's text may rebuild the row it lands in.
    #[test]
    fn typing_into_a_board_field_leaves_the_layout_alone() {
        let mut app = app_with_two_boards();
        let summary = app.search_summary();
        let before = section_layouts(&app, &summary);

        let Some(item) = app.draft.boards.items.first_mut() else {
            return;
        };
        item.name = "Half typ".to_string();
        item.id = "half-typ".to_string();
        item.background_color.set_component(0, "0.1".to_string());
        app.color_picker_hex
            .insert(ColorPickerId::BoardBackground(0), "#1A00".to_string());

        let summary = app.search_summary();
        assert_eq!(before, section_layouts(&app, &summary));
        // The values a refresh writes in place carry the edit instead.
        assert_eq!(app.draft.boards.items[0].id, "half-typ");
        assert_eq!(
            app.color_picker_hex
                .get(&ColorPickerId::BoardBackground(0))
                .map(String::as_str),
            Some("#1A00")
        );
    }

    #[test]
    fn toggling_a_board_switch_changes_the_layout() {
        let mut app = app_with_two_boards();
        let summary = app.search_summary();
        let before = section_layouts(&app, &summary);

        let Some(item) = app.draft.boards.items.first_mut() else {
            return;
        };
        item.pinned = !item.pinned;

        let summary = app.search_summary();
        assert_ne!(before, section_layouts(&app, &summary));
    }

    #[test]
    fn collapsing_a_board_changes_the_layout() {
        let mut app = app_with_two_boards();
        let summary = app.search_summary();
        let before = section_layouts(&app, &summary);

        let Some(collapsed) = app.boards_collapsed.first_mut() else {
            return;
        };
        *collapsed = true;

        let summary = app.search_summary();
        assert_ne!(before, section_layouts(&app, &summary));
    }

    /// Search visibility belongs to the layout too: a rebuild is what applies
    /// it now that nothing refreshes a section in place.
    #[test]
    fn a_search_that_hides_a_board_changes_the_layout() {
        let mut app = app_with_two_boards();
        let Some(item) = app.draft.boards.items.first_mut() else {
            return;
        };
        item.name = "zqxwvu".to_string();
        let summary = app.search_summary();
        let before = section_layouts(&app, &summary);

        app.search_query = SearchQuery::new("zqxwvu");
        let summary = app.search_summary();
        let layouts = section_layouts(&app, &summary);

        assert_ne!(before, layouts);
        let visible: Vec<bool> = layouts.iter().map(|layout| layout.visible).collect();
        assert_eq!(visible.first(), Some(&true));
        assert!(visible.iter().skip(1).all(|visible| !visible));
    }
}
