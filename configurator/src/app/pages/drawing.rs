//! Drawing page: default color, quick colors, drawing defaults, per-button
//! drag tool mapping, and font controls.
//!
//! Three shapes here go past the plain row helpers. A section that only
//! applies in one mode — named versus RGB color, the open drag button — is a
//! boxed list added to its area's group, so the group keeps owning search
//! visibility while the section's own binding answers only the model
//! question. Quick colors are a dynamic list: rows are rebuilt when the entry
//! count changes and refreshed in place otherwise, which keeps the row the
//! user is typing in alive. The named/RGB mode row comes from
//! [`chrome`](super::super::chrome), which picks the widget this build's
//! libadwaita floor allows; the page adds the row it gets back to the group
//! like any other and never asks which channel it is on.

use relm4::prelude::*;
use relm4::{adw, gtk};

use adw::prelude::*;
use gtk::glib;

use wayscriber::config::{DragButtonConfig, QUICK_COLOR_RENDER_LIMIT, QuickColorSlot};
use wayscriber::draw::Color;

use crate::messages::Message;
use crate::models::color::ColorInput;
use crate::models::util::format_float;
use crate::models::{
    ColorMode, ColorPickerId, DragColorOption, DragMouseButton, DragToolField, DragToolOption,
    EraserModeOption, FontStyleOption, FontWeightOption, NamedColorOption, TabId, TextField,
    ToggleField,
};

use super::super::chrome;
use super::super::search::{AppSearchSummary, SearchArea};
use super::super::state::ConfiguratorApp;
use super::color_rows::{ResolvedColor, color_row, dialog_hex, mark_hex_error, set_swatch_blocked};
use super::{BuiltPage, PageBuilder, set_selected_blocked, set_text_blocked};

/// Mouse buttons that carry a drag mapping section, in the order the old
/// view listed them.
const DRAG_BUTTONS: [DragMouseButton; 3] = [
    DragMouseButton::Left,
    DragMouseButton::Right,
    DragMouseButton::Middle,
];

/// Modifier combinations each drag mapping section binds.
const DRAG_FIELDS: [DragToolField; 5] = [
    DragToolField::Drag,
    DragToolField::ShiftDrag,
    DragToolField::CtrlDrag,
    DragToolField::CtrlShiftDrag,
    DragToolField::TabDrag,
];

const COLOR_MODES: [ColorMode; 2] = [ColorMode::Named, ColorMode::Rgb];

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let mut page = PageBuilder::new(sender, TabId::Drawing);

    build_default_color(&mut page);
    build_quick_colors(&mut page);
    build_defaults(&mut page);
    build_drag_mapping(&mut page);
    build_font(&mut page);

    page.finish()
}

// ---------------------------------------------------------------------------
// Default color
// ---------------------------------------------------------------------------

fn build_default_color(page: &mut PageBuilder) {
    let (mode_row, mode_binding) = chrome::mode_toggle(
        page.sender(),
        "Color mode",
        "A palette name or hex string, or explicit RGB components.",
        COLOR_MODES.to_vec(),
        vec!["Named color".to_string(), "RGB color".to_string()],
        |app| app.draft.drawing_color.mode,
        Message::ColorModeChanged,
    );
    page.group_in_area("Default color", SearchArea::DrawingColor)
        .chrome_row(&mode_row, mode_binding);

    let named = conditional_section(page, |app| app.draft.drawing_color.mode == ColorMode::Named);
    section_combo_row(
        page,
        &named,
        "Named color",
        NamedColorOption::list(),
        named_color_labels(),
        |app| app.draft.drawing_color.selected_named,
        Message::NamedColorSelected,
    );
    section_entry_row(
        page,
        &named,
        "Custom color name",
        |app| app.draft.drawing_color.name.clone(),
        |value| Message::TextChanged(TextField::DrawingColorName, value),
        |app| color_name_error(&app.draft.drawing_color, "Unknown color name"),
    );

    // Only in RGB mode: in Named mode the save serializes the named value,
    // so a visible RGB edit here would be silently discarded.
    page.group_in_area_when("RGB color", SearchArea::DrawingColor, |app| {
        app.draft.drawing_color.mode == ColorMode::Rgb
    });
    color_row(page, "Custom color", ColorPickerId::DrawingColor, |app| {
        resolved(app.draft.drawing_color.preview_color())
    });
}

/// The error the old view showed under a custom color name that resolves to
/// nothing, `None` while the field is empty or usable.
fn color_name_error(color: &ColorInput, message: &str) -> Option<String> {
    let unresolved = color.preview_color().is_none() && !color.name.trim().is_empty();
    unresolved.then(|| message.to_string())
}

fn named_color_labels() -> Vec<String> {
    NamedColorOption::list()
        .iter()
        .map(|option| option.label().to_string())
        .collect()
}

fn resolved(color: Option<Color>) -> ResolvedColor {
    color.map(|color| (color.r, color.g, color.b, color.a))
}

// ---------------------------------------------------------------------------
// Quick colors
// ---------------------------------------------------------------------------

fn build_quick_colors(page: &mut PageBuilder) {
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

// ---------------------------------------------------------------------------
// Drawing defaults
// ---------------------------------------------------------------------------

fn build_defaults(page: &mut PageBuilder) {
    page.group_in_area("Drawing defaults", SearchArea::DrawingDefaults)
        .entry_row_validated(
            "Thickness (px)",
            |app| app.draft.drawing_default_thickness.clone(),
            |value| Message::TextChanged(TextField::DrawingThickness, value),
            |app| validate_f64_range(&app.draft.drawing_default_thickness, 1.0, 50.0),
        )
        .entry_row_validated(
            "Font size (pt)",
            |app| app.draft.drawing_default_font_size.clone(),
            |value| Message::TextChanged(TextField::DrawingFontSize, value),
            |app| validate_f64_range(&app.draft.drawing_default_font_size, 8.0, 72.0),
        )
        .entry_row_validated(
            "Polygon sides",
            |app| app.draft.drawing_polygon_sides.clone(),
            |value| Message::TextChanged(TextField::DrawingPolygonSides, value),
            |app| validate_usize_range(&app.draft.drawing_polygon_sides, 3, 12),
        )
        .entry_row_validated(
            "Eraser size (px)",
            |app| app.draft.drawing_default_eraser_size.clone(),
            |value| Message::TextChanged(TextField::DrawingEraserSize, value),
            |app| validate_f64_range(&app.draft.drawing_default_eraser_size, 1.0, 50.0),
        )
        .combo_row(
            "Eraser mode",
            "",
            EraserModeOption::list(),
            EraserModeOption::list()
                .iter()
                .map(|option| option.label().to_string())
                .collect(),
            |app| app.draft.drawing_default_eraser_mode,
            Message::EraserModeChanged,
        )
        .entry_row_validated(
            "Marker opacity (0.05-0.9)",
            |app| app.draft.drawing_marker_opacity.clone(),
            |value| Message::TextChanged(TextField::DrawingMarkerOpacity, value),
            |app| validate_f64_range(&app.draft.drawing_marker_opacity, 0.05, 0.9),
        )
        .entry_row_validated(
            "Undo stack limit",
            |app| app.draft.drawing_undo_stack_limit.clone(),
            |value| Message::TextChanged(TextField::DrawingUndoStackLimit, value),
            |app| validate_usize_range(&app.draft.drawing_undo_stack_limit, 10, 1000),
        )
        .entry_row_validated(
            "Hit-test tolerance (px)",
            |app| app.draft.drawing_hit_test_tolerance.clone(),
            |value| Message::TextChanged(TextField::DrawingHitTestTolerance, value),
            |app| validate_f64_range(&app.draft.drawing_hit_test_tolerance, 1.0, 20.0),
        )
        .entry_row_validated(
            "Hit-test threshold",
            |app| app.draft.drawing_hit_test_linear_threshold.clone(),
            |value| Message::TextChanged(TextField::DrawingHitTestThreshold, value),
            |app| validate_usize_min(&app.draft.drawing_hit_test_linear_threshold, 1),
        )
        .switch_row(
            "Enable text background",
            "",
            |app| app.draft.drawing_text_background_enabled,
            |value| Message::ToggleChanged(ToggleField::DrawingTextBackground, value),
        )
        .switch_row(
            "Start shapes filled",
            "",
            |app| app.draft.drawing_default_fill_enabled,
            |value| Message::ToggleChanged(ToggleField::DrawingFillEnabled, value),
        );
}

// ---------------------------------------------------------------------------
// Drag tool mapping
// ---------------------------------------------------------------------------

fn build_drag_mapping(page: &mut PageBuilder) {
    page.group_in_area("Drag tool mapping", SearchArea::DrawingDragTools);

    let switcher = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .halign(gtk::Align::Start)
        .css_classes(["linked"])
        .build();
    for button in DRAG_BUTTONS {
        let toggle = gtk::Button::with_label(button.label());
        {
            let sender = page.sender();
            toggle.connect_clicked(move |_| {
                sender.input(Message::DrawingDragMappingSectionToggled(button));
            });
        }
        switcher.append(&toggle);
        page.bind(move |app, _summary| {
            let open = app.active_drawing_drag_button == Some(button);
            if toggle.has_css_class("suggested-action") != open {
                if open {
                    toggle.add_css_class("suggested-action");
                } else {
                    toggle.remove_css_class("suggested-action");
                }
            }
        });
    }
    page.custom(&switcher);

    for button in DRAG_BUTTONS {
        build_drag_button_section(page, button);
    }
}

fn build_drag_button_section(page: &mut PageBuilder, button: DragMouseButton) {
    let section = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(12)
        .build();
    section.append(
        &gtk::Label::builder()
            .label(button.label())
            .xalign(0.0)
            .css_classes(["heading"])
            .build(),
    );
    let list = boxed_list();
    section.append(&list);
    page.custom(&section);
    page.bind(move |app, summary| {
        set_visible_if_changed(&section, drag_section_visible(app, summary, button));
    });

    let tools = DragToolOption::list_for_button(button);
    let tool_labels: Vec<String> = tools
        .iter()
        .map(|option| option.label().to_string())
        .collect();
    let colors = DragColorOption::list();
    let color_labels: Vec<String> = colors
        .iter()
        .map(|option| option.label().to_string())
        .collect();

    for field in DRAG_FIELDS {
        section_combo_row(
            page,
            &list,
            field.label(),
            tools.clone(),
            tool_labels.clone(),
            move |app| drag_tool_for_field(drag_button_config(app, button), field),
            move |option| Message::DrawingMouseDragToolChanged(button, field, option),
        );
        section_combo_row(
            page,
            &list,
            &format!("{} color", field.label()),
            colors.clone(),
            color_labels.clone(),
            move |app| drag_color_for_field(drag_button_config(app, button), field),
            move |option| Message::DrawingMouseDragColorChanged(button, field, option),
        );
    }
}

/// Mirrors the old view's `visible_drag_mapping_buttons`: a search that
/// matched the drag area on its own opens every button, otherwise only the
/// section the user picked is open.
fn drag_section_visible(
    app: &ConfiguratorApp,
    summary: &AppSearchSummary,
    button: DragMouseButton,
) -> bool {
    let matched_by_search = summary.tab(TabId::Drawing).is_some_and(|search| {
        !search.show_all() && search.area_matches(SearchArea::DrawingDragTools)
    });
    matched_by_search || app.active_drawing_drag_button == Some(button)
}

fn drag_button_config(app: &ConfiguratorApp, button: DragMouseButton) -> &DragButtonConfig {
    match button {
        DragMouseButton::Left => &app.draft.drawing_drag_tools.left,
        DragMouseButton::Right => &app.draft.drawing_drag_tools.right,
        DragMouseButton::Middle => &app.draft.drawing_drag_tools.middle,
    }
}

fn drag_tool_for_field(config: &DragButtonConfig, field: DragToolField) -> DragToolOption {
    match field {
        DragToolField::Drag => DragToolOption::from_drag_tool(config.drag_tool),
        DragToolField::ShiftDrag => DragToolOption::from_drag_tool(config.shift_drag_tool),
        DragToolField::CtrlDrag => DragToolOption::from_drag_tool(config.ctrl_drag_tool),
        DragToolField::CtrlShiftDrag => DragToolOption::from_drag_tool(config.ctrl_shift_drag_tool),
        DragToolField::TabDrag => DragToolOption::from_drag_tool(config.tab_drag_tool),
    }
}

fn drag_color_for_field(config: &DragButtonConfig, field: DragToolField) -> DragColorOption {
    match field {
        DragToolField::Drag => DragColorOption::from_color(config.drag_color.as_ref()),
        DragToolField::ShiftDrag => DragColorOption::from_color(config.shift_drag_color.as_ref()),
        DragToolField::CtrlDrag => DragColorOption::from_color(config.ctrl_drag_color.as_ref()),
        DragToolField::CtrlShiftDrag => {
            DragColorOption::from_color(config.ctrl_shift_drag_color.as_ref())
        }
        DragToolField::TabDrag => DragColorOption::from_color(config.tab_drag_color.as_ref()),
    }
}

// ---------------------------------------------------------------------------
// Font
// ---------------------------------------------------------------------------

fn build_font(page: &mut PageBuilder) {
    page.group_in_area("Font", SearchArea::DrawingFont)
        .entry_row(
            "Font family",
            |app| app.draft.drawing_font_family.clone(),
            |value| Message::TextChanged(TextField::DrawingFontFamily, value),
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
}

// ---------------------------------------------------------------------------
// Sections and rows
// ---------------------------------------------------------------------------

/// A boxed list added to the current group, shown only while the model
/// condition holds.
///
/// Search visibility stays with the group, which hides the section along with
/// everything else it owns, so this binding answers one question only.
fn conditional_section(
    page: &mut PageBuilder,
    visible: impl Fn(&ConfiguratorApp) -> bool + 'static,
) -> gtk::ListBox {
    let list = boxed_list();
    list.set_margin_top(6);
    page.custom(&list);
    let handle = list.clone();
    page.bind(move |app, _summary| set_visible_if_changed(&handle, visible(app)));
    list
}

fn section_combo_row<O>(
    page: &mut PageBuilder,
    list: &gtk::ListBox,
    title: &str,
    values: Vec<O>,
    labels: Vec<String>,
    get: impl Fn(&ConfiguratorApp) -> O + 'static,
    to_message: impl Fn(O) -> Message + 'static,
) where
    O: Copy + PartialEq + 'static,
{
    let row = combo_row_widget(title, &labels);
    let handler = connect_combo(&row, page.sender(), values.clone(), to_message);
    list.append(&row);
    page.bind(move |app, _summary| {
        let current = get(app);
        let Some(index) = values.iter().position(|value| *value == current) else {
            return;
        };
        let index = index as u32;
        if row.selected() != index {
            // Blocked: the model chose this, so reporting it back as a user
            // pick only clears the status line.
            row.block_signal(&handler);
            row.set_selected(index);
            row.unblock_signal(&handler);
        }
    });
}

fn section_entry_row(
    page: &mut PageBuilder,
    list: &gtk::ListBox,
    title: &str,
    get: impl Fn(&ConfiguratorApp) -> String + 'static,
    to_message: impl Fn(String) -> Message + 'static,
    validate: impl Fn(&ConfiguratorApp) -> Option<String> + 'static,
) {
    let row = adw::EntryRow::builder().title(title).build();
    let handler = {
        let sender = page.sender();
        row.connect_changed(move |row| sender.input(to_message(row.text().to_string())))
    };
    list.append(&row);
    page.bind(move |app, _summary| {
        set_text_blocked(&row, &handler, &get(app));
        set_error_if_changed(&row, validate(app));
    });
}

fn boxed_list() -> gtk::ListBox {
    gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build()
}

fn combo_row_widget(title: &str, labels: &[String]) -> adw::ComboRow {
    let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    adw::ComboRow::builder()
        .title(title)
        .model(&gtk::StringList::new(&label_refs))
        .build()
}

fn connect_combo<O: Copy + 'static>(
    row: &adw::ComboRow,
    sender: ComponentSender<ConfiguratorApp>,
    values: Vec<O>,
    to_message: impl Fn(O) -> Message + 'static,
) -> glib::SignalHandlerId {
    row.connect_selected_notify(move |row| {
        if let Some(value) = values.get(row.selected() as usize) {
            sender.input(to_message(*value));
        }
    })
}

fn icon_button(icon: &str, tooltip: &str) -> gtk::Button {
    gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build()
}

fn color_dialog_button() -> gtk::ColorDialogButton {
    let button =
        gtk::ColorDialogButton::new(Some(gtk::ColorDialog::builder().with_alpha(true).build()));
    button.set_valign(gtk::Align::Center);
    button
}

// ---------------------------------------------------------------------------
// Echo-guarded widget writes
// ---------------------------------------------------------------------------

fn set_visible_if_changed(widget: &impl IsA<gtk::Widget>, visible: bool) {
    if widget.is_visible() != visible {
        widget.set_visible(visible);
    }
}

fn select_if_changed<O: PartialEq>(row: &adw::ComboRow, values: &[O], current: O) {
    if let Some(index) = values.iter().position(|value| *value == current) {
        let index = index as u32;
        if row.selected() != index {
            row.set_selected(index);
        }
    }
}

fn set_error_if_changed(row: &adw::EntryRow, error: Option<String>) {
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

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Error text for a decimal field constrained to `min..=max`, `None` while
/// the input is acceptable. Ported from the Iced view's shared validators so
/// these fields keep the feedback they had.
fn validate_f64_range(value: &str, min: f64, max: f64) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some("Expected a numeric value".to_string());
    }
    match trimmed.parse::<f64>() {
        Ok(parsed) if (min..=max).contains(&parsed) => None,
        Ok(_) => Some(format!(
            "Range: {}-{}",
            format_float(min),
            format_float(max)
        )),
        Err(_) => Some("Expected a numeric value".to_string()),
    }
}

/// Error text for a whole-number field constrained to `min..=max`.
fn validate_usize_range(value: &str, min: usize, max: usize) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some("Expected a whole number".to_string());
    }
    match trimmed.parse::<usize>() {
        Ok(parsed) if (min..=max).contains(&parsed) => None,
        Ok(_) => Some(format!("Range: {min}-{max}")),
        Err(_) => Some("Expected a whole number".to_string()),
    }
}

/// Error text for a whole-number field with a lower bound only.
fn validate_usize_min(value: &str, min: usize) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some("Expected a whole number".to_string());
    }
    match trimmed.parse::<usize>() {
        Ok(parsed) if parsed >= min => None,
        Ok(_) => Some(format!("Minimum: {min}")),
        Err(_) => Some("Expected a whole number".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SearchQuery;

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

    #[test]
    fn drag_sections_all_open_when_search_matches_the_drag_area() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        app.search_query = SearchQuery::new("shift");
        let summary = app.search_summary();

        assert_eq!(app.active_drawing_drag_button, None);
        for button in DRAG_BUTTONS {
            assert!(drag_section_visible(&app, &summary, button));
        }
    }

    #[test]
    fn drag_sections_follow_the_open_button_without_a_search() {
        let (mut app, _effects) = ConfiguratorApp::new_app();
        app.active_drawing_drag_button = Some(DragMouseButton::Right);
        let summary = app.search_summary();

        assert!(drag_section_visible(&app, &summary, DragMouseButton::Right));
        assert!(!drag_section_visible(&app, &summary, DragMouseButton::Left));
    }

    #[test]
    fn range_validators_report_the_old_view_messages() {
        assert_eq!(validate_f64_range("25", 1.0, 50.0), None);
        assert_eq!(
            validate_f64_range("60", 1.0, 50.0),
            Some("Range: 1-50".to_string())
        );
        assert_eq!(
            validate_usize_range("2", 3, 12),
            Some("Range: 3-12".to_string())
        );
        assert_eq!(validate_usize_min("0", 1), Some("Minimum: 1".to_string()));
    }
}
