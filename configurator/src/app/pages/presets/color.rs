use super::rows::{SlotBuilder, color_swatch, labels_of};
use relm4::adw::prelude::*;

use crate::messages::Message;
use crate::models::{ColorMode, NamedColorOption, PresetTextField};

use super::super::validate_u32_range;

/// The slot's color block: a mode chooser with a live preview, then the
/// rows the chosen mode edits.
pub(super) fn build_color_rows(rows: &mut SlotBuilder<'_>, slot: usize) {
    let mode_row = rows.combo_row(
        "Color mode",
        vec![ColorMode::Named, ColorMode::Rgb],
        vec!["Named".to_string(), "RGB".to_string()],
        move |app| app.draft.presets.slot(slot).map(|draft| draft.color.mode),
        Message::PresetColorModeChanged,
    );
    let swatch = color_swatch();
    mode_row.add_suffix(&swatch);
    // The color the swatch is currently drawing, owned by the binding that
    // draws it: a new draw function is what redraws the widget, so it is only
    // installed when the color actually moved.
    let mut shown: Option<[f64; 3]> = None;
    rows.page.bind(move |app, _search| {
        let color = app
            .draft
            .presets
            .slot(slot)
            .and_then(|draft| draft.color.preview_color())
            .map(|color| [color.r, color.g, color.b]);
        if shown == color {
            return;
        }
        shown = color;
        match color {
            Some(rgb) => swatch.set_draw_func(move |_, context, width, height| {
                context.set_source_rgb(rgb[0], rgb[1], rgb[2]);
                context.rectangle(0.0, 0.0, f64::from(width), f64::from(height));
                // A failed fill only leaves the swatch blank.
                let _ = context.fill();
            }),
            // A slot whose color resolves to nothing draws nothing.
            None => swatch.set_draw_func(|_, _, _, _| {}),
        }
    });

    let named = NamedColorOption::list();
    let named_labels = labels_of(&named, NamedColorOption::label);
    let named_row = rows.combo_row(
        "Named color",
        named,
        named_labels,
        move |app| {
            app.draft
                .presets
                .slot(slot)
                .map(|draft| draft.color.selected_named)
        },
        Message::PresetNamedColorSelected,
    );
    rows.visible_when(&named_row, move |app| {
        app.draft
            .presets
            .slot(slot)
            .is_some_and(|draft| draft.color.mode == ColorMode::Named)
    });

    let custom_name = rows.entry_row_validated(
        "Custom color name",
        move |app| {
            app.draft
                .presets
                .slot(slot)
                .map(|draft| draft.color.name.clone())
        },
        move |slot, value| Message::PresetTextChanged(slot, PresetTextField::ColorName, value),
        move |app| {
            let color = &app.draft.presets.slot(slot)?.color;
            if color.mode != ColorMode::Named {
                return None;
            }
            (color.preview_color().is_none() && !color.name.trim().is_empty())
                .then(|| "Unknown color name.".to_string())
        },
    );
    rows.visible_when(&custom_name, move |app| {
        app.draft.presets.slot(slot).is_some_and(|draft| {
            draft.color.mode == ColorMode::Named && draft.color.selected_named_is_custom()
        })
    });

    for (component, title) in [
        (0usize, "Red (0-255)"),
        (1, "Green (0-255)"),
        (2, "Blue (0-255)"),
    ] {
        let row = rows.entry_row_validated(
            title,
            move |app| {
                app.draft
                    .presets
                    .slot(slot)
                    .and_then(|draft| draft.color.rgb.get(component).cloned())
            },
            move |slot, value| Message::PresetColorComponentChanged(slot, component, value),
            move |app| {
                let color = &app.draft.presets.slot(slot)?.color;
                if color.mode != ColorMode::Rgb {
                    return None;
                }
                validate_u32_range(color.rgb.get(component)?, 0, 255)
            },
        );
        rows.visible_when(&row, move |app| {
            app.draft
                .presets
                .slot(slot)
                .is_some_and(|draft| draft.color.mode == ColorMode::Rgb)
        });
    }
}
