use crate::messages::Message;
use crate::models::color::ColorInput;
use crate::models::{ColorMode, ColorPickerId, NamedColorOption, TextField};

use super::super::super::search::SearchArea;
use super::super::PageBuilder;
use super::super::color_rows::color_row;
use super::{
    COLOR_MODES, conditional_section, named_color_labels, resolved, section_combo_row,
    section_entry_row,
};

pub(super) fn build(page: &mut PageBuilder) {
    page.group_in_area("Default color", SearchArea::DrawingColor)
        .combo_row(
            "Color mode",
            "A palette name or hex string, or explicit RGB components.",
            COLOR_MODES.to_vec(),
            vec!["Named color".to_string(), "RGB color".to_string()],
            |app| app.draft.drawing_color.mode,
            Message::ColorModeChanged,
        );

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
