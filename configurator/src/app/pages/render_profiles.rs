//! Render Profiles page: named color mappings and how they are applied.
//!
//! Profiles and their mappings are both user-sized lists, so the sections
//! cannot be a fixed set of [`PageBuilder`] rows. One container holds one
//! `GtkListBox` per profile, and every section is built together with the
//! closure that refreshes it: that closure owns the section's typed widget
//! handles and the signal handler ids guarding each write, so a refresh never
//! rediscovers a control by position and never reports a programmatic write
//! as a user edit.
//!
//! The binding owns both halves of that arrangement: the [`SectionLayout`]
//! list it last built — each profile's mapping count and everything a search
//! hides — and one typed refresh closure per section. When the layouts differ
//! the two are replaced together, so a section and the values handed to it
//! can never drift apart. A rebuilt row gets its layout before its handlers
//! connect, so the first refresh cannot echo the model straight back at
//! itself.
//!
//! Profile ids, names, and mapping hex stay out of the layout on purpose:
//! they change on every keystroke, and rebuilding would take the caret with
//! it. Those values ride [`ProfileValues`] into the sections' own refresh
//! closures, which write them with their handlers blocked — the swatch
//! reports a pick as `ColorPickerHexChanged`, which rewrites the mapping in
//! canonical form, so an echo would dirty a freshly loaded file.

mod mapping;
mod rows;
mod section;
#[cfg(test)]
mod tests;

use relm4::prelude::*;
use relm4::{adw, gtk};

use adw::prelude::*;
use gtk::glib::SignalHandlerId;

use crate::messages::Message;
use crate::models::color::parse_hex;
use crate::models::{
    ColorPickerId, RenderProfileExportOption, RenderProfileMappingSide,
    RenderProfileSelectionOption, RenderProfileTextField, TabId,
};

use super::super::search::{AppSearchSummary, SearchArea, TabSearchSummary};
use super::super::state::ConfiguratorApp;
use super::color_rows::{dialog_hex, mark_hex_error, set_swatch_blocked};
use super::{BuiltPage, PageBuilder, set_text_blocked};
use rows::{picker_hex, selected_string, sync_combo};
use section::{BoundProfileSection, rebuild_sections};

/// `GTK_INVALID_LIST_POSITION`: what a `GtkSingleSelection` reads as "nothing
/// is selected", which is how the Iced pick list rendered an unknown id.
const NO_SELECTION: u32 = u32::MAX;

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let mut page = PageBuilder::new(sender, TabId::RenderProfiles);

    let export_options = RenderProfileExportOption::list();
    let export_labels: Vec<String> = export_options
        .iter()
        .map(|option| option.label().to_string())
        .collect();

    page.group_in_area("Render Profiles", SearchArea::RenderProfilesGeneral)
        .switch_row(
            "Preview canvas",
            "",
            |app| app.draft.render_profiles.apply_to_canvas,
            Message::RenderProfileApplyCanvasChanged,
        )
        .switch_row(
            "Preview UI",
            "",
            |app| app.draft.render_profiles.apply_to_ui,
            Message::RenderProfileApplyUiChanged,
        );

    add_active_row(&mut page);

    page.combo_row(
        "Canvas export profile",
        "",
        export_options,
        export_labels,
        |app| app.draft.render_profiles.export,
        Message::RenderProfileExportChanged,
    );

    add_export_profile_row(&mut page);
    add_add_button(&mut page);
    add_profile_list(&mut page);

    page.finish()
}

/// The startup profile: `Off` plus one entry per profile id.
fn add_active_row(page: &mut PageBuilder) {
    let row = adw::ComboRow::builder().title("Startup profile").build();
    row.set_model(Some(&gtk::StringList::new(&[])));
    let handler = {
        let sender = page.sender();
        row.connect_selected_notify(move |row| {
            let position = row.selected();
            if position == NO_SELECTION {
                return;
            }
            // Position zero is the `Off` option, whose profile id is empty.
            let value = if position == 0 {
                String::new()
            } else {
                let Some(item) = selected_string(row) else {
                    return;
                };
                item
            };
            sender.input(Message::RenderProfileActiveChanged(value));
        })
    };
    page.custom(&row);
    // The combo's own entries, kept by the binding that writes them.
    let mut shown: Vec<String> = Vec::new();
    page.bind(move |app, _summary| {
        let profiles = &app.draft.render_profiles;
        let ids = profiles.profile_ids();
        let options = RenderProfileSelectionOption::list(&ids);
        let current = RenderProfileSelectionOption::from_active(&profiles.active, &ids);
        let selected = options.iter().position(|option| *option == current);
        let entries: Vec<String> = options.iter().map(ToString::to_string).collect();
        sync_combo(&row, &handler, &mut shown, &entries, selected);
    });
}

/// The named export profile, which the Iced view showed only while the export
/// mode was `Named profile`.
fn add_export_profile_row(page: &mut PageBuilder) {
    let row = adw::ComboRow::builder()
        .title("Named export profile")
        .build();
    row.set_model(Some(&gtk::StringList::new(&[])));
    // Hidden until the export mode asks for it, so it cannot flash on the
    // first frame.
    row.set_visible(false);
    let handler = {
        let sender = page.sender();
        row.connect_selected_notify(move |row| {
            let Some(value) = selected_string(row) else {
                return;
            };
            sender.input(Message::RenderProfileExportProfileChanged(value));
        })
    };
    page.custom(&row);
    // The combo's own entries, kept by the binding that writes them.
    let mut shown: Vec<String> = Vec::new();
    page.bind(move |app, _summary| {
        let profiles = &app.draft.render_profiles;
        let ids = profiles.profile_ids();
        let selected = ids.iter().position(|id| *id == profiles.export_profile);
        sync_combo(&row, &handler, &mut shown, &ids, selected);

        let visible = profiles.export == RenderProfileExportOption::Profile;
        if row.is_visible() != visible {
            row.set_visible(visible);
        }
    });
}

fn add_add_button(page: &mut PageBuilder) {
    let button = gtk::Button::builder()
        .label("Add profile")
        .halign(gtk::Align::Start)
        .margin_top(12)
        .build();
    {
        let sender = page.sender();
        button.connect_clicked(move |_| sender.input(Message::RenderProfileAdd));
    }
    page.custom(&button);
}

/// The per-profile sections, in their own untitled group so search can hide
/// them one at a time instead of as a block.
fn add_profile_list(page: &mut PageBuilder) {
    page.group("");
    let container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .build();
    page.custom(&container);

    let sender = page.sender();
    // Each built section owns the layout that produced it and its typed refresh.
    let mut sections: Vec<BoundProfileSection> = Vec::new();
    page.bind(move |app, summary| {
        let layouts = section_layouts(app, summary);
        if !sections
            .iter()
            .map(|section| &section.layout)
            .eq(layouts.iter())
        {
            sections = rebuild_sections(&container, &layouts, &sender);
        }
        for ((index, profile), section) in app
            .draft
            .render_profiles
            .profiles
            .iter()
            .enumerate()
            .zip(sections.iter())
        {
            let values = ProfileValues {
                id: &profile.id,
                name: &profile.name,
                mappings: profile
                    .mappings
                    .iter()
                    .enumerate()
                    .map(|(mapping, values)| MappingValues {
                        from: picker_hex(
                            app,
                            ColorPickerId::RenderProfileMappingFrom(index, mapping),
                            &values.from,
                        ),
                        to: picker_hex(
                            app,
                            ColorPickerId::RenderProfileMappingTo(index, mapping),
                            &values.to,
                        ),
                    })
                    .collect(),
            };
            (section.refresh)(&values);
        }
    });
}

// ---- What a rebuild depends on -----------------------------------------

/// Everything a profile section shows before any text is written into it:
/// how many mapping rows it has, and which of its rows a search leaves
/// reachable.
///
/// This is the whole rebuild trigger: the binding compares the list of these
/// it built against the list the model asks for now, so a control added here
/// rebuilds on its own value without anyone having to remember to extend a
/// format string.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SectionLayout {
    visible: bool,
    /// Id, name, and the add-mapping button: the profile's own controls.
    controls: bool,
    /// One flag per mapping row, so the count is part of the fingerprint.
    mappings: Vec<bool>,
}

fn section_layouts(app: &ConfiguratorApp, summary: &AppSearchSummary) -> Vec<SectionLayout> {
    let tab = summary.tab(TabId::RenderProfiles);
    let show_all = tab.is_none_or(TabSearchSummary::show_all);
    let matched: &[usize] = tab.map_or(&[], TabSearchSummary::render_profile_indices);
    let mapping_matches: &[(usize, usize)] =
        tab.map_or(&[], TabSearchSummary::render_profile_mapping_indices);

    app.draft
        .render_profiles
        .profiles
        .iter()
        .enumerate()
        .map(|(index, profile)| {
            let controls = tab.is_none_or(|tab| tab.render_profile_controls_visible(index));
            SectionLayout {
                visible: show_all || matched.contains(&index),
                controls,
                mappings: (0..profile.mappings.len())
                    .map(|mapping| controls || mapping_matches.contains(&(index, mapping)))
                    .collect(),
            }
        })
        .collect()
}

/// One profile row's values: everything the layout deliberately left out.
struct ProfileValues<'a> {
    id: &'a str,
    name: &'a str,
    /// One entry per mapping row, in mapping order.
    mappings: Vec<MappingValues<'a>>,
}

/// One mapping row's two editing hex fields.
struct MappingValues<'a> {
    from: &'a str,
    to: &'a str,
}
