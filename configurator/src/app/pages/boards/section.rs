use relm4::{ComponentSender, adw, gtk};

use adw::prelude::*;

use crate::messages::Message;
use crate::models::{
    BoardBackgroundOption, BoardItemTextField, BoardItemToggleField, ColorPickerId,
};

use super::super::super::state::ConfiguratorApp;
use super::super::set_text_blocked;
use super::color::build_color_row;
use super::header::build_header_row;
use super::rows::{build_kind_row, build_text_row, build_toggle_row};
use super::{BoardValues, SectionLayout};

/// One section's refresh: built beside its row, so it owns that row's typed
/// widget handles and the signal handler ids guarding each write. No
/// positional lookup, and no widget the refresh can silently miss.
pub(super) type BoardRowRefresh = Box<dyn Fn(&BoardValues<'_>)>;

pub(super) struct BoundBoardSection {
    pub(super) layout: SectionLayout,
    pub(super) refresh: BoardRowRefresh,
}

/// One board's section: the list box, and the closure that writes the values
/// the layout deliberately left out.
struct BoardSection {
    section: gtk::ListBox,
    refresh: BoardRowRefresh,
}

pub(super) fn rebuild_sections(
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
