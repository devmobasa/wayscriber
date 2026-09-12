use super::{
    SectionLayout,
    rows::{TextRow, build_text_row},
};
use crate::app::state::ConfiguratorApp;
use crate::messages::Message;
use crate::models::{BoardBackgroundOption, BoardItemTextField};
use adw::prelude::*;
use relm4::{ComponentSender, adw, gtk};
use wayscriber::domain::BoardGridKind;

pub(super) fn build(
    index: usize,
    layout: SectionLayout,
    sender: &ComponentSender<ConfiguratorApp>,
) -> (adw::ComboRow, TextRow) {
    let labels: Vec<_> = BoardGridKind::ALL.iter().map(|kind| kind.label()).collect();
    let grid = adw::ComboRow::builder()
        .title("Paper pattern")
        .model(&gtk::StringList::new(&labels))
        .visible(layout.expanded)
        .build();
    grid.set_selected(
        BoardGridKind::ALL
            .iter()
            .position(|kind| *kind == layout.grid_kind)
            .unwrap_or(0) as u32,
    );
    let solid = layout.background_kind != BoardBackgroundOption::Transparent;
    grid.set_sensitive(solid);
    if !solid {
        grid.set_subtitle("Paper patterns require a solid board background");
    }
    let grid_sender = sender.clone();
    grid.connect_selected_notify(move |row| {
        if let Some(kind) = BoardGridKind::ALL.get(row.selected() as usize) {
            grid_sender.input(Message::BoardsGridKindChanged(index, *kind));
        }
    });
    let spacing = build_text_row(
        "Grid spacing (8–200 logical pixels)",
        index,
        BoardItemTextField::GridSpacing,
        sender,
    );
    spacing.row.set_visible(layout.expanded);
    spacing.row.set_sensitive(solid);
    (grid, spacing)
}
