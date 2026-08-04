use super::mapping::{MappingRow, build_add_mapping_row, build_mapping_row};
use super::rows::{build_header_row, build_text_row};
use relm4::{ComponentSender, gtk};

use gtk::prelude::*;

use crate::models::RenderProfileTextField;

use super::super::super::state::ConfiguratorApp;
use super::super::set_text_blocked;
use super::{ProfileValues, SectionLayout};

/// One section's refresh: built beside its row, so it owns that row's typed
/// widget handles and the signal handler ids guarding each write.
type ProfileRowRefresh = Box<dyn Fn(&ProfileValues<'_>)>;

pub(super) struct BoundProfileSection {
    pub(super) layout: SectionLayout,
    pub(super) refresh: ProfileRowRefresh,
}

/// One profile's section: the list box, and the closure that writes the
/// values the layout deliberately left out.
struct ProfileSection {
    section: gtk::ListBox,
    refresh: ProfileRowRefresh,
}

pub(super) fn rebuild_sections(
    container: &gtk::Box,
    layouts: &[SectionLayout],
    sender: &ComponentSender<ConfiguratorApp>,
) -> Vec<BoundProfileSection> {
    // Draining the container, not walking it for a control: the sections that
    // replace these carry their own refresh closures.
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }

    let mut sections = Vec::with_capacity(layouts.len());
    for (index, layout) in layouts.iter().enumerate() {
        let built = build_section(index, layout, sender);
        container.append(&built.section);
        sections.push(BoundProfileSection {
            layout: layout.clone(),
            refresh: built.refresh,
        });
    }
    sections
}

fn build_section(
    index: usize,
    layout: &SectionLayout,
    sender: &ComponentSender<ConfiguratorApp>,
) -> ProfileSection {
    let section = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .visible(layout.visible)
        .build();

    let title = gtk::Label::builder()
        .xalign(0.0)
        .hexpand(true)
        .css_classes(["heading"])
        .build();
    section.append(&build_header_row(index, &title, sender));

    let id = build_text_row("Profile id", index, RenderProfileTextField::Id, sender);
    id.row.set_visible(layout.controls);
    section.append(&id.row);

    let name = build_text_row("Display name", index, RenderProfileTextField::Name, sender);
    name.row.set_visible(layout.controls);
    section.append(&name.row);

    let mut mappings: Vec<MappingRow> = Vec::with_capacity(layout.mappings.len());
    for (mapping, visible) in layout.mappings.iter().enumerate() {
        let row = build_mapping_row(index, mapping, sender);
        row.row.set_visible(*visible);
        section.append(&row.row);
        mappings.push(row);
    }

    let add = build_add_mapping_row(index, sender);
    add.set_visible(layout.controls);
    section.append(&add);

    let refresh: ProfileRowRefresh = Box::new(move |values| {
        let heading = if values.name.trim().is_empty() {
            "Profile"
        } else {
            values.name.trim()
        };
        if title.text() != heading {
            title.set_text(heading);
        }

        set_text_blocked(&id.row, &id.handler, values.id);
        set_text_blocked(&name.row, &name.handler, values.name);

        for (row, hex) in mappings.iter().zip(values.mappings.iter()) {
            row.from.refresh(hex.from);
            row.to.refresh(hex.to);
        }
    });

    ProfileSection { section, refresh }
}
