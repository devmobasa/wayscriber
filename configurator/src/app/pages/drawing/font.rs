use std::rc::Rc;

use relm4::{ComponentSender, adw, gtk};

use adw::prelude::*;

use crate::messages::Message;
use crate::models::{FontStyleOption, FontWeightOption, TextField};
use wayscriber::draw::{families_match, family_is_installed, system_font_families};

use super::super::super::search::SearchArea;
use super::super::super::state::ConfiguratorApp;
use super::super::{PageBuilder, set_selected_blocked};
use super::{boxed_list, conditional_section, icon_button, section_entry_row};

pub(super) fn build(page: &mut PageBuilder) {
    page.group_in_area("Font", SearchArea::DrawingFont)
        .entry_row_validated(
            "Font family",
            |app| app.draft.drawing_font_family.clone(),
            |value| Message::TextChanged(TextField::DrawingFontFamily, value),
            |app| validate_installed_family(&app.draft.drawing_font_family),
        )
        .combo_row(
            "Font weight",
            "",
            FontWeightOption::list(),
            FontWeightOption::list()
                .iter()
                .map(|option| option.label().to_string())
                .collect(),
            |app| app.draft.drawing_font_weight_option,
            Message::FontWeightOptionSelected,
        )
        .entry_row(
            "Custom or numeric weight",
            |app| app.draft.drawing_font_weight.clone(),
            |value| Message::TextChanged(TextField::DrawingFontWeight, value),
        )
        .combo_row(
            "Font style",
            "",
            FontStyleOption::list(),
            FontStyleOption::list()
                .iter()
                .map(|option| option.label().to_string())
                .collect(),
            |app| app.draft.drawing_font_style_option,
            Message::FontStyleOptionSelected,
        );

    let custom_style = conditional_section(page, |app| {
        app.draft.drawing_font_style_option == FontStyleOption::Custom
    });
    section_entry_row(
        page,
        &custom_style,
        "Custom style",
        |app| app.draft.drawing_font_style.clone(),
        |value| Message::TextChanged(TextField::DrawingFontStyle, value),
        |_app| None,
    );

    build_font_cycle(page);
}

/// `GTK_INVALID_LIST_POSITION`: what a `GtkSingleSelection` reads as "nothing
/// selected". This preserves a missing configured family instead of making
/// the first installed family look selected.
const NO_SELECTION: u32 = u32::MAX;

/// The ordered list `Shift+T` walks, as one row per family.
///
/// It used to be a comma-separated line, which asked for a family name spelled
/// exactly right and could not express one containing a comma. A row per entry
/// removes both, and lets each row offer every installed family rather than
/// asking the user to know what is installed.
fn build_font_cycle(page: &mut PageBuilder) {
    page.group_in_area("Font cycle", SearchArea::DrawingFont);

    // An activatable ActionRow rather than AdwButtonRow: ButtonRow needs
    // libadwaita 1.6 and the crate's feature floor is 1.4 (Ubuntu 24.04).
    let add = adw::ActionRow::builder()
        .title("Add font")
        .subtitle("Shift+T steps through this list in order")
        .activatable(true)
        .build();
    add.add_prefix(&gtk::Image::from_icon_name("list-add-symbolic"));
    {
        let sender = page.sender();
        add.connect_activated(move |_| sender.input(Message::FontCycleAdded));
    }
    page.custom(&add);

    let empty = gtk::Label::builder()
        .label("No fonts listed: Shift+T does nothing until one is added.")
        .wrap(true)
        .xalign(0.0)
        .margin_top(6)
        .css_classes(["dim-label"])
        .build();
    page.custom(&empty);
    page.bind({
        let empty = empty.clone();
        move |app, _summary| {
            empty.set_visible(app.draft.drawing_font_cycle.is_empty());
        }
    });

    let list = boxed_list();
    list.set_margin_top(6);
    page.custom(&list);

    // One model for every row. 269 families is a list worth building once, and
    // sharing it means a row added later costs nothing to populate.
    let installed: Rc<[String]> = system_font_families().to_vec().into();
    let installed_names: Vec<&str> = installed.iter().map(String::as_str).collect();
    let families = gtk::StringList::new(&installed_names);

    let sender = page.sender();
    let mut rows: Vec<BoundFontRow> = Vec::new();
    page.bind(move |app, _summary| {
        let layouts = font_cycle_layouts(app);
        if !rows
            .iter()
            .map(|row| row.layout)
            .eq(layouts.iter().copied())
        {
            rows = rebuild_font_cycle(&list, &families, Rc::clone(&installed), &layouts, &sender);
        }
        for (family, row) in app
            .draft
            .drawing_font_cycle
            .entries()
            .iter()
            .zip(rows.iter())
        {
            (row.refresh)(family);
        }
    });
}

/// Everything a row shows before the family is written into it.
///
/// The rebuild trigger, as with quick colors: the binding compares what it
/// built against what the model asks for now. The chosen family stays out —
/// rebuilding a row would close the dropdown the user is searching in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FontCycleLayout {
    index: usize,
    can_move_up: bool,
    can_move_down: bool,
}

fn font_cycle_layouts(app: &ConfiguratorApp) -> Vec<FontCycleLayout> {
    let count = app.draft.drawing_font_cycle.len();
    (0..count)
        .map(|index| FontCycleLayout {
            index,
            can_move_up: index > 0,
            can_move_down: index + 1 < count,
        })
        .collect()
}

type FontRowRefresh = Box<dyn Fn(&str)>;

struct BoundFontRow {
    layout: FontCycleLayout,
    refresh: FontRowRefresh,
}

fn rebuild_font_cycle(
    list: &gtk::ListBox,
    families: &gtk::StringList,
    family_names: Rc<[String]>,
    layouts: &[FontCycleLayout],
    sender: &ComponentSender<ConfiguratorApp>,
) -> Vec<BoundFontRow> {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    layouts
        .iter()
        .map(|layout| {
            let (row, refresh) =
                build_font_cycle_row(*layout, families, Rc::clone(&family_names), sender);
            list.append(&row);
            BoundFontRow {
                layout: *layout,
                refresh,
            }
        })
        .collect()
}

fn build_font_cycle_row(
    layout: FontCycleLayout,
    families: &gtk::StringList,
    family_names: Rc<[String]>,
    sender: &ComponentSender<ConfiguratorApp>,
) -> (adw::ComboRow, FontRowRefresh) {
    let index = layout.index;
    let row = adw::ComboRow::builder()
        .title(format!("{}.", index + 1))
        .model(families)
        // 269 families is past what scrolling a menu can find. The search box
        // needs an expression to know which property to match on.
        .enable_search(true)
        .expression(gtk::PropertyExpression::new(
            gtk::StringObject::static_type(),
            None::<gtk::Expression>,
            "string",
        ))
        .build();
    row.set_list_factory(Some(&family_preview_factory()));

    let up = icon_button("go-up-symbolic", "Move up");
    up.set_sensitive(layout.can_move_up);
    {
        let sender = sender.clone();
        up.connect_clicked(move |_| sender.input(Message::FontCycleMoved(index, -1)));
    }
    let down = icon_button("go-down-symbolic", "Move down");
    down.set_sensitive(layout.can_move_down);
    {
        let sender = sender.clone();
        down.connect_clicked(move |_| sender.input(Message::FontCycleMoved(index, 1)));
    }
    let remove = icon_button("user-trash-symbolic", "Remove");
    {
        let sender = sender.clone();
        remove.connect_clicked(move |_| sender.input(Message::FontCycleRemoved(index)));
    }
    row.add_suffix(&up);
    row.add_suffix(&down);
    row.add_suffix(&remove);

    let handler = {
        let sender = sender.clone();
        row.connect_selected_notify(move |row| {
            let Some(family) = selected_family(row) else {
                return;
            };
            sender.input(Message::FontCycleChanged(index, family));
        })
    };

    let refresh_row = row.clone();
    let refresh: FontRowRefresh = Box::new(move |family: &str| {
        let position = family_position(&family_names, family);
        set_selected_blocked(&refresh_row, &handler, position);
        // A family in the file that this machine does not have still shows its
        // name, rather than silently reading as whatever sits at position zero.
        refresh_row.set_subtitle(&missing_family_note(family).unwrap_or_default());
    });

    (row, refresh)
}

/// A list factory that draws every family in its own face.
///
/// The point of the whole control: nobody picks a typeface by reading its name.
/// The same reason the in-overlay picker lays each of its rows out in the font
/// it names.
fn family_preview_factory() -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let label = gtk::Label::builder().xalign(0.0).build();
        item.set_child(Some(&label));
    });
    factory.connect_bind(|_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let Some(label) = item.child().and_downcast::<gtk::Label>() else {
            return;
        };
        let Some(family) = item
            .item()
            .and_downcast::<gtk::StringObject>()
            .map(|object| object.string().to_string())
        else {
            return;
        };
        label.set_label(&family);
        let attributes = gtk::pango::AttrList::new();
        attributes.insert(gtk::pango::AttrFontDesc::new(
            &gtk::pango::FontDescription::from_string(&family),
        ));
        label.set_attributes(Some(&attributes));
    });
    factory
}

/// The family the row is showing, if the model has one at that position.
fn selected_family(row: &adw::ComboRow) -> Option<String> {
    row.selected_item()
        .and_downcast::<gtk::StringObject>()
        .map(|object| object.string().to_string())
}

/// Where `family` sits in the installed catalog, or no selection when the
/// catalog does not hold it.
fn family_position(families: &[String], family: &str) -> u32 {
    families
        .iter()
        .position(|installed| families_match(installed, family))
        .and_then(|position| u32::try_from(position).ok())
        .unwrap_or(NO_SELECTION)
}

/// Warn about a family the font system cannot find.
///
/// Pango resolves an unknown family to whatever fontconfig substitutes, with no
/// error anywhere, so a typo renders in a different face and looks like the
/// setting was ignored. Naming it here is the only place the user finds out.
///
/// A blank field is not an error: it means "leave the built-in default".
fn validate_installed_family(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || family_is_installed(trimmed) {
        return None;
    }
    Some(format!(
        "\"{trimmed}\" is not installed; text falls back to another font"
    ))
}

/// The same check for one row of the cycle list.
///
/// A configuration written on another machine can name a family this one does
/// not have. The dropdown cannot show it — its model is what is installed — so
/// without this the row would fall back to position zero and read as a font the
/// file never asked for.
fn missing_family_note(family: &str) -> Option<String> {
    let trimmed = family.trim();
    if trimmed.is_empty() || family_is_installed(trimmed) {
        return None;
    }
    Some(format!("\"{trimmed}\" is not installed on this system"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installed() -> String {
        wayscriber::draw::system_font_families()
            .first()
            .expect("at least one family")
            .clone()
    }

    #[test]
    fn an_installed_family_raises_no_warning_and_a_blank_field_is_allowed() {
        assert_eq!(validate_installed_family(&installed()), None);
        assert_eq!(validate_installed_family(""), None);
        assert_eq!(validate_installed_family("   "), None);
    }

    #[test]
    fn a_missing_family_is_named_so_a_typo_is_findable() {
        let message = validate_installed_family("Wayscriber No Such Font 9000")
            .expect("a missing family warns");

        assert!(message.contains("Wayscriber No Such Font 9000"));
    }

    #[test]
    fn a_cycle_row_naming_a_font_this_machine_lacks_says_so() {
        // The dropdown's model is what is installed, so it cannot show the
        // family a config written elsewhere asked for. Without the note the row
        // falls back to position zero and reads as a font nobody chose.
        assert_eq!(missing_family_note(&installed()), None);
        assert_eq!(missing_family_note(""), None);

        let note = missing_family_note("Wayscriber No Such Font 9000").expect("missing warns");
        assert!(note.contains("Wayscriber No Such Font 9000"));
    }

    #[test]
    fn a_missing_cycle_family_is_unselected_instead_of_impersonating_the_first_font() {
        let families = vec!["Sans".to_string(), "Serif".to_string()];

        assert_eq!(family_position(&families, "sans"), 0);
        assert_eq!(
            family_position(&families, "Wayscriber No Such Font 9000"),
            NO_SELECTION
        );
    }
}
