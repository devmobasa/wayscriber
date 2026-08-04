use relm4::{ComponentSender, adw, gtk};

use adw::prelude::*;

use wayscriber::config::{QUICK_COLOR_RENDER_LIMIT, QuickColorSlot};

use crate::messages::Message;
use crate::models::{ColorMode, ColorPickerId, NamedColorOption, TextField};

use super::super::super::search::SearchArea;
use super::super::super::state::ConfiguratorApp;
use super::super::color_rows::{ResolvedColor, dialog_hex, mark_hex_error, set_swatch_blocked};
use super::super::{PageBuilder, set_selected_blocked, set_text_blocked};
use super::{
    COLOR_MODES, boxed_list, color_dialog_button, combo_row_widget, connect_combo, icon_button,
    named_color_labels, resolved, select_if_changed, set_error_if_changed, set_visible_if_changed,
};

pub(super) fn build(page: &mut PageBuilder) {
    page.group_in_area("Quick colors", SearchArea::DrawingColor);

    // An activatable ActionRow rather than AdwButtonRow: ButtonRow needs
    // libadwaita 1.6 and the crate's feature floor is 1.4 (Ubuntu 24.04).
    let add = adw::ActionRow::builder()
        .title("Add color")
        .activatable(true)
        .build();
    add.add_prefix(&gtk::Image::from_icon_name("list-add-symbolic"));
    {
        let sender = page.sender();
        add.connect_activated(move |_| sender.input(Message::QuickColorAdded));
    }
    page.custom(&add);

    let warning = gtk::Label::builder()
        .label(format!(
            "Only the first {QUICK_COLOR_RENDER_LIMIT} quick colors are shown in toolbar and radial menus."
        ))
        .wrap(true)
        .xalign(0.0)
        .margin_top(6)
        .css_classes(["warning"])
        .build();
    page.custom(&warning);
    page.bind(move |app, _summary| {
        let over_limit = app.draft.drawing_quick_colors.entries.len() > QUICK_COLOR_RENDER_LIMIT;
        set_visible_if_changed(&warning, over_limit);
    });

    let list = boxed_list();
    list.set_margin_top(6);
    page.custom(&list);
    let sender = page.sender();
    // Each built row owns the layout that produced it and its typed refresh.
    let mut rows: Vec<BoundQuickColorRow> = Vec::new();
    page.bind(move |app, _summary| {
        let layouts = quick_color_layouts(app);
        if !rows
            .iter()
            .map(|row| row.layout)
            .eq(layouts.iter().copied())
        {
            rows = rebuild_quick_colors(&list, &layouts, &sender);
        }
        for ((index, entry), row) in app
            .draft
            .drawing_quick_colors
            .entries
            .iter()
            .enumerate()
            .zip(rows.iter())
        {
            let values = QuickColorValues {
                label: &entry.label,
                name: &entry.color.name,
                hex: picker_hex(app, ColorPickerId::QuickColor(index)),
                named: entry.color.selected_named,
                preview: resolved(entry.color.preview_color()),
                summary: entry.color.summary(),
            };
            (row.refresh)(&values);
        }
    });
}

/// Everything a quick color row shows before any text is written into it.
///
/// This is the whole rebuild trigger: the binding compares the list of these
/// it built against the list the model asks for now, so a control added here
/// rebuilds on its own value. Everything the user types — and the named-color
/// choice, which a typed name moves on its own — stays out, because rebuilding
/// a row takes the caret with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct QuickColorLayout {
    mode: ColorMode,
    can_move_up: bool,
    can_move_down: bool,
    can_remove: bool,
}

fn quick_color_layouts(app: &ConfiguratorApp) -> Vec<QuickColorLayout> {
    let count = app.draft.drawing_quick_colors.entries.len();
    app.draft
        .drawing_quick_colors
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| QuickColorLayout {
            mode: entry.color.mode,
            can_move_up: index > 0,
            can_move_down: index + 1 < count,
            // The draft refuses to drop below the eight slots the shortcuts
            // bind.
            can_remove: count > QuickColorSlot::ALL.len(),
        })
        .collect()
}

/// One quick color row's values: everything the layout deliberately left out.
struct QuickColorValues<'a> {
    label: &'a str,
    name: &'a str,
    hex: &'a str,
    named: NamedColorOption,
    /// The color the entry resolves to, `None` when it resolves to nothing —
    /// which is also what makes the name field an error.
    preview: ResolvedColor,
    summary: String,
}

/// One row's refresh: built beside its row, so it owns that row's typed widget
/// handles and the signal handler ids guarding each write.
type QuickColorRowRefresh = Box<dyn Fn(&QuickColorValues<'_>)>;

struct BoundQuickColorRow {
    layout: QuickColorLayout,
    refresh: QuickColorRowRefresh,
}

fn rebuild_quick_colors(
    list: &gtk::ListBox,
    layouts: &[QuickColorLayout],
    sender: &ComponentSender<ConfiguratorApp>,
) -> Vec<BoundQuickColorRow> {
    // Draining the list, not walking it for a control: the rows that replace
    // these carry their own refresh closures.
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    let mut rows = Vec::with_capacity(layouts.len());
    for (index, layout) in layouts.iter().enumerate() {
        let built = build_quick_color_row(index, *layout, sender);
        list.append(&built.row);
        rows.push(BoundQuickColorRow {
            layout: *layout,
            refresh: built.refresh,
        });
    }
    rows
}

/// One quick color entry, built with everything its layout decides already in
/// place, and paired with the closure that writes the rest.
///
/// The closure owns the row's widgets and the ids of their handlers, so it
/// writes them blocked: the hex path reports a programmatic write as a user
/// pick, and that pick rewrites the entry's components and wipes the status
/// line the user was reading.
struct QuickColorRow {
    row: adw::ExpanderRow,
    refresh: QuickColorRowRefresh,
}

fn build_quick_color_row(
    index: usize,
    layout: QuickColorLayout,
    sender: &ComponentSender<ConfiguratorApp>,
) -> QuickColorRow {
    let row = adw::ExpanderRow::builder()
        .title(format!("Color {}", index + 1))
        .build();

    let up = icon_button("go-up-symbolic", "Move up");
    up.set_sensitive(layout.can_move_up);
    {
        let sender = sender.clone();
        up.connect_clicked(move |_| sender.input(Message::QuickColorMoved(index, -1)));
    }
    let down = icon_button("go-down-symbolic", "Move down");
    down.set_sensitive(layout.can_move_down);
    {
        let sender = sender.clone();
        down.connect_clicked(move |_| sender.input(Message::QuickColorMoved(index, 1)));
    }
    let remove = icon_button("user-trash-symbolic", "Remove");
    remove.set_sensitive(layout.can_remove);
    {
        let sender = sender.clone();
        remove.connect_clicked(move |_| sender.input(Message::QuickColorRemoved(index)));
    }
    row.add_suffix(&up);
    row.add_suffix(&down);
    row.add_suffix(&remove);

    let label = adw::EntryRow::builder().title("Label").build();
    let label_handler = {
        let sender = sender.clone();
        label.connect_changed(move |row| {
            sender.input(Message::TextChanged(
                TextField::QuickColorLabel(index),
                row.text().to_string(),
            ));
        })
    };
    row.add_row(&label);

    // The mode is part of the fingerprint, so it is selected before the
    // handler exists and never written again behind the user's back.
    let mode = combo_row_widget(
        "Color mode",
        &["Named or hex".to_string(), "RGB".to_string()],
    );
    select_if_changed(&mode, &COLOR_MODES, layout.mode);
    connect_combo(&mode, sender.clone(), COLOR_MODES.to_vec(), move |value| {
        Message::QuickColorModeChanged(index, value)
    });
    row.add_row(&mode);

    let named_options = NamedColorOption::list();
    let named = combo_row_widget("Named color", &named_color_labels());
    named.set_visible(layout.mode == ColorMode::Named);
    let named_handler = {
        let sender = sender.clone();
        let options = named_options.clone();
        named.connect_selected_notify(move |row| {
            if let Some(option) = options.get(row.selected() as usize) {
                sender.input(Message::QuickNamedColorSelected(index, *option));
            }
        })
    };
    row.add_row(&named);

    let name = adw::EntryRow::builder()
        .title("Color name or #RRGGBB[AA]")
        .build();
    name.set_visible(layout.mode == ColorMode::Named);
    let name_handler = {
        let sender = sender.clone();
        name.connect_changed(move |row| {
            sender.input(Message::TextChanged(
                TextField::QuickColorName(index),
                row.text().to_string(),
            ));
        })
    };
    row.add_row(&name);

    let picker = ColorPickerId::QuickColor(index);
    let hex = adw::EntryRow::builder().title("Custom color").build();
    hex.set_visible(layout.mode == ColorMode::Rgb);
    let hex_handler = {
        let sender = sender.clone();
        hex.connect_changed(move |row| {
            sender.input(Message::ColorPickerHexChanged(
                picker,
                row.text().to_string(),
            ));
        })
    };
    let swatch = color_dialog_button();
    let swatch_handler = {
        let sender = sender.clone();
        swatch.connect_rgba_notify(move |button| {
            sender.input(Message::ColorPickerHexChanged(
                picker,
                dialog_hex(&button.rgba()),
            ));
        })
    };
    hex.add_suffix(&swatch);
    row.add_row(&hex);

    let expander = row.clone();
    let refresh: QuickColorRowRefresh = Box::new(move |values| {
        let subtitle = format!("{} / {}", values.label.trim(), values.summary);
        if expander.subtitle() != subtitle {
            expander.set_subtitle(&subtitle);
        }

        set_text_blocked(&label, &label_handler, values.label);
        set_text_blocked(&name, &name_handler, values.name);
        set_error_if_changed(&name, name_error(values));
        set_text_blocked(&hex, &hex_handler, values.hex);
        // The same predicate the save gate counts with, so a field styled
        // clean can never be one the save refuses.
        mark_hex_error(&hex, values.hex);

        if let Some(position) = named_options
            .iter()
            .position(|option| *option == values.named)
        {
            set_selected_blocked(&named, &named_handler, position as u32);
        }

        if let Some((r, g, b, a)) = values.preview {
            let rgba = gtk::gdk::RGBA::new(r as f32, g as f32, b as f32, a as f32);
            set_swatch_blocked(&swatch, &swatch_handler, &rgba);
        }
    });

    QuickColorRow { row, refresh }
}

/// The error the old view showed under a quick color name that resolves to
/// nothing, read off the values the row was handed.
fn name_error(values: &QuickColorValues<'_>) -> Option<String> {
    let unresolved = values.preview.is_none() && !values.name.trim().is_empty();
    unresolved.then(|| "Use a known color name, #RRGGBB, or #RRGGBBAA for alpha".to_string())
}

fn picker_hex(app: &ConfiguratorApp, id: ColorPickerId) -> &str {
    app.color_picker_hex.get(&id).map_or("", String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    /// The law the binding rests on: one layout per quick color, so the rows
    /// it builds and the values it hands them are indexed by the same thing.
    #[test]
    fn there_is_one_layout_per_quick_color() {
        let (app, _effects) = ConfiguratorApp::new_app();
        let layouts = quick_color_layouts(&app);

        assert_eq!(layouts.len(), app.draft.drawing_quick_colors.entries.len());
    }

    /// The caret guarantee, stated as the layout law it rests on: typing a
    /// color name moves the named-color choice with it, so neither may be
    /// part of what rebuilds the row.
    #[test]
    fn typing_a_quick_color_name_leaves_the_layout_alone() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        let before = quick_color_layouts(&app);

        app.draft
            .set_text(TextField::QuickColorName(0), "gree".to_string());

        assert_eq!(before, quick_color_layouts(&app));
        // The choice the model moved rides the row's values instead, where a
        // blocked write can put it in the combo without a rebuild.
        assert_eq!(
            app.draft.drawing_quick_colors.entries[0]
                .color
                .selected_named,
            NamedColorOption::Custom
        );
    }

    #[test]
    fn switching_a_quick_color_to_rgb_changes_the_layout() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        let before = quick_color_layouts(&app);

        let Some(entry) = app.draft.drawing_quick_colors.get_mut(0) else {
            return;
        };
        entry.color.mode = ColorMode::Rgb;

        assert_ne!(before, quick_color_layouts(&app));
    }

    #[test]
    fn a_quick_color_name_that_resolves_to_nothing_is_an_error() {
        let values = QuickColorValues {
            label: "Red",
            name: "red",
            hex: "#FF0000",
            named: NamedColorOption::Red,
            preview: Some((1.0, 0.0, 0.0, 1.0)),
            summary: "Red".to_string(),
        };
        assert_eq!(name_error(&values), None);

        let unresolved = QuickColorValues {
            preview: None,
            name: "nope",
            summary: values.summary.clone(),
            ..values
        };
        assert!(name_error(&unresolved).is_some());

        let empty = QuickColorValues {
            preview: None,
            name: "  ",
            summary: values.summary.clone(),
            ..values
        };
        assert_eq!(name_error(&empty), None);
    }
}
