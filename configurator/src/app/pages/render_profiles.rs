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

// ---- Sections ----------------------------------------------------------

/// One section's refresh: built beside its row, so it owns that row's typed
/// widget handles and the signal handler ids guarding each write.
type ProfileRowRefresh = Box<dyn Fn(&ProfileValues<'_>)>;

struct BoundProfileSection {
    layout: SectionLayout,
    refresh: ProfileRowRefresh,
}

/// One profile's section: the list box, and the closure that writes the
/// values the layout deliberately left out.
struct ProfileSection {
    section: gtk::ListBox,
    refresh: ProfileRowRefresh,
}

fn rebuild_sections(
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

// ---- Header row --------------------------------------------------------

fn build_header_row(
    index: usize,
    title: &gtk::Label,
    sender: &ComponentSender<ConfiguratorApp>,
) -> gtk::ListBoxRow {
    let content = row_content_box();
    content.append(title);

    let duplicate = gtk::Button::builder()
        .label("Duplicate")
        .valign(gtk::Align::Center)
        .build();
    connect_button(&duplicate, Message::RenderProfileDuplicate(index), sender);
    content.append(&duplicate);

    let remove = gtk::Button::builder()
        .label("Delete")
        .valign(gtk::Align::Center)
        .css_classes(["destructive-action"])
        .build();
    connect_button(&remove, Message::RenderProfileRemove(index), sender);
    content.append(&remove);

    plain_row(&content)
}

// ---- Mapping row -------------------------------------------------------

/// One side of a mapping: the hex field the Iced view had, plus the native
/// color dialog standing in for its popup picker.
struct ColorField {
    hex: gtk::Entry,
    hex_handler: SignalHandlerId,
    swatch: gtk::ColorDialogButton,
    swatch_handler: SignalHandlerId,
}

impl ColorField {
    fn refresh(&self, hex: &str) {
        set_text_blocked(&self.hex, &self.hex_handler, hex);
        // The same predicate the save gate counts with, so a field styled
        // clean can never be one the save refuses.
        mark_hex_error(&self.hex, hex);

        let Some((rgb, _)) = parse_hex(hex) else {
            // Half-typed hex: leave the swatch on the last color that parsed
            // rather than flashing it to black on every keystroke.
            return;
        };
        let rgba = gtk::gdk::RGBA::new(rgb[0] as f32, rgb[1] as f32, rgb[2] as f32, 1.0);
        set_swatch_blocked(&self.swatch, &self.swatch_handler, &rgba);
    }
}

struct MappingRow {
    row: gtk::ListBoxRow,
    from: ColorField,
    to: ColorField,
}

fn build_mapping_row(
    index: usize,
    mapping: usize,
    sender: &ComponentSender<ConfiguratorApp>,
) -> MappingRow {
    let content = row_content_box();

    content.append(&side_label("From"));
    let from = build_color_field(index, mapping, RenderProfileMappingSide::From, sender);
    content.append(&from.hex);
    content.append(&from.swatch);

    content.append(&side_label("\u{2192}"));
    content.append(&side_label("To"));
    let to = build_color_field(index, mapping, RenderProfileMappingSide::To, sender);
    content.append(&to.hex);
    content.append(&to.swatch);

    let remove = gtk::Button::builder()
        .label("Remove")
        .valign(gtk::Align::Center)
        .halign(gtk::Align::End)
        .hexpand(true)
        .build();
    connect_button(
        &remove,
        Message::RenderProfileMappingRemove(index, mapping),
        sender,
    );
    content.append(&remove);

    MappingRow {
        row: plain_row(&content),
        from,
        to,
    }
}

fn build_color_field(
    index: usize,
    mapping: usize,
    side: RenderProfileMappingSide,
    sender: &ComponentSender<ConfiguratorApp>,
) -> ColorField {
    let hex = gtk::Entry::builder()
        .placeholder_text("#RRGGBB")
        .width_chars(9)
        .max_width_chars(9)
        .build();
    let hex_handler = {
        let sender = sender.clone();
        hex.connect_changed(move |entry| {
            sender.input(Message::RenderProfileMappingColorChanged(
                index,
                mapping,
                side,
                entry.text().to_string(),
            ));
        })
    };

    let swatch =
        gtk::ColorDialogButton::new(Some(gtk::ColorDialog::builder().with_alpha(false).build()));
    swatch.set_valign(gtk::Align::Center);
    let id = match side {
        RenderProfileMappingSide::From => ColorPickerId::RenderProfileMappingFrom(index, mapping),
        RenderProfileMappingSide::To => ColorPickerId::RenderProfileMappingTo(index, mapping),
    };
    let swatch_handler = {
        let sender = sender.clone();
        swatch.connect_rgba_notify(move |button| {
            sender.input(Message::ColorPickerHexChanged(
                id,
                dialog_hex(&button.rgba()),
            ));
        })
    };

    ColorField {
        hex,
        hex_handler,
        swatch,
        swatch_handler,
    }
}

// ---- Add-mapping row ---------------------------------------------------

fn build_add_mapping_row(
    index: usize,
    sender: &ComponentSender<ConfiguratorApp>,
) -> gtk::ListBoxRow {
    let content = row_content_box();
    let button = gtk::Button::builder()
        .label("Add mapping")
        .halign(gtk::Align::Start)
        .build();
    connect_button(&button, Message::RenderProfileMappingAdd(index), sender);
    content.append(&button);
    plain_row(&content)
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
    field: RenderProfileTextField,
    sender: &ComponentSender<ConfiguratorApp>,
) -> TextRow {
    let row = adw::EntryRow::builder().title(title).build();
    let handler = {
        let sender = sender.clone();
        row.connect_changed(move |row| {
            sender.input(Message::RenderProfileTextChanged(
                index,
                field,
                row.text().to_string(),
            ));
        })
    };
    TextRow { row, handler }
}

fn connect_button(
    button: &gtk::Button,
    message: Message,
    sender: &ComponentSender<ConfiguratorApp>,
) {
    let sender = sender.clone();
    button.connect_clicked(move |_| sender.input(message.clone()));
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

fn selected_string(row: &adw::ComboRow) -> Option<String> {
    let item = row
        .selected_item()
        .and_then(|item| item.downcast::<gtk::StringObject>().ok())?;
    Some(item.string().to_string())
}

fn row_content_box() -> gtk::Box {
    gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
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

fn side_label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .valign(gtk::Align::Center)
        .css_classes(["dim-label", "caption"])
        .build()
}

/// The picker's editing text, falling back to the stored mapping value the
/// way the Iced hex field did.
fn picker_hex<'a>(app: &'a ConfiguratorApp, id: ColorPickerId, value: &'a str) -> &'a str {
    app.color_picker_hex.get(&id).map_or(value, String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SearchQuery;

    fn app_with_a_profile() -> ConfiguratorApp {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        let profile = app.draft.render_profiles.new_profile();
        app.draft.render_profiles.profiles.push(profile);
        app
    }

    /// The law the binding rests on: one layout per profile, and a row's
    /// values carry exactly the mappings that layout built rows for.
    #[test]
    fn a_row_carries_one_value_per_mapping_row_the_layout_built() {
        let app = app_with_a_profile();
        let layouts = section_layouts(&app, &app.search_summary());

        assert_eq!(layouts.len(), app.draft.render_profiles.profiles.len());
        for (profile, layout) in app
            .draft
            .render_profiles
            .profiles
            .iter()
            .zip(layouts.iter())
        {
            assert_eq!(profile.mappings.len(), layout.mappings.len());
        }
    }

    /// The caret guarantee, stated as the layout law it rests on: no keystroke
    /// in a profile's text may rebuild the row it lands in.
    #[test]
    fn typing_into_a_profile_field_leaves_the_layout_alone() {
        let mut app = app_with_a_profile();
        let before = section_layouts(&app, &app.search_summary());

        let Some(profile) = app.draft.render_profiles.profiles.first_mut() else {
            return;
        };
        profile.name = "Half typ".to_string();
        profile.id = "half-typ".to_string();
        if let Some(mapping) = profile.mappings.first_mut() {
            mapping.from = "#00FF0".to_string();
        }

        assert_eq!(before, section_layouts(&app, &app.search_summary()));
        assert_eq!(app.draft.render_profiles.profiles[0].id, "half-typ");
    }

    #[test]
    fn adding_a_mapping_changes_the_layout() {
        let mut app = app_with_a_profile();
        let before = section_layouts(&app, &app.search_summary());

        let Some(profile) = app.draft.render_profiles.profiles.first_mut() else {
            return;
        };
        let Some(mapping) = profile.mappings.first().cloned() else {
            return;
        };
        profile.mappings.push(mapping);

        assert_ne!(before, section_layouts(&app, &app.search_summary()));
    }

    /// Search visibility belongs to the layout too: a rebuild is what applies
    /// it now that nothing refreshes a section in place.
    #[test]
    fn a_search_that_hides_a_profile_changes_the_layout() {
        let mut app = app_with_a_profile();
        let second = app.draft.render_profiles.new_profile();
        app.draft.render_profiles.profiles.push(second);
        let Some(profile) = app.draft.render_profiles.profiles.first_mut() else {
            return;
        };
        profile.name = "zqxwvu".to_string();
        let before = section_layouts(&app, &app.search_summary());

        app.search_query = SearchQuery::new("zqxwvu");
        let layouts = section_layouts(&app, &app.search_summary());

        assert_ne!(before, layouts);
        let visible: Vec<bool> = layouts.iter().map(|layout| layout.visible).collect();
        assert_eq!(visible.first(), Some(&true));
        assert!(visible.iter().skip(1).all(|visible| !visible));
    }
}
