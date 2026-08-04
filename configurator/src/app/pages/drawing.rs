//! Drawing page: default color, quick colors, drawing defaults, per-button
//! drag tool mapping, and font controls.
//!
//! Two shapes here go past the plain row helpers. A section that only applies
//! in one mode — named versus RGB color, the open drag button — is a boxed
//! list added to its area's group, so the group keeps owning search
//! visibility while the section's own binding answers only the model
//! question. Quick colors are a dynamic list: rows are rebuilt when the entry
//! count changes and refreshed in place otherwise, which keeps the row the
//! user is typing in alive.

mod default_color;
mod defaults;
mod drag_mapping;
mod font;
mod quick_colors;

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

    default_color::build(&mut page);
    quick_colors::build(&mut page);
    defaults::build(&mut page);
    drag_mapping::build(&mut page);
    font::build(&mut page);

    page.finish()
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

fn named_color_labels() -> Vec<String> {
    NamedColorOption::list()
        .iter()
        .map(|option| option.label().to_string())
        .collect()
}

fn resolved(color: Option<Color>) -> ResolvedColor {
    color.map(|color| (color.r, color.g, color.b, color.a))
}

#[cfg(test)]
mod tests {
    use super::*;

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
