use super::*;
use wayscriber::config::{ColorSpec, Config};

use crate::app::state::ConfiguratorApp;
use crate::models::{ColorMode, ColorPickerId, NamedColorOption};

#[test]
fn quick_color_mode_change_to_rgb_materializes_named_hex_preview() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let entry = &mut app.draft.drawing_quick_colors.entries[1];
    entry.color.mode = ColorMode::Named;
    entry.color.selected_named = NamedColorOption::Custom;
    entry.color.name = "#123456".to_string();

    let _ = app.handle_quick_color_mode_changed(1, ColorMode::Rgb);

    assert_eq!(
        app.draft.drawing_quick_colors.entries[1].color.rgb,
        ["18", "52", "86"]
    );

    let saved = app
        .draft
        .to_config(&Config::default())
        .expect("expected quick color RGB to save");

    assert_eq!(
        saved.drawing.quick_colors.entries[1].color,
        ColorSpec::Rgb([18, 52, 86])
    );
}

/// Deleting a quick color takes its picker's editing text with it.
///
/// Without the prune the removed slot's text stays in the map with no row
/// left to show it, and text the save gate refuses keeps Save disabled
/// over a field the user can no longer reach.
#[test]
fn removing_the_last_quick_color_drops_the_hex_text_it_left_behind() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let _ = app.handle_quick_color_added();
    let last = app.draft.drawing_quick_colors.entries.len() - 1;
    let _ =
        app.handle_color_picker_hex_changed(ColorPickerId::QuickColor(last), "#12zz".to_string());
    assert_eq!(app.invalid_color_hex_count(), 1);

    let _ = app.handle_quick_color_removed(last);

    assert!(
        !app.color_picker_hex
            .contains_key(&ColorPickerId::QuickColor(last)),
        "the removed slot's picker text must go with the row"
    );
    assert_eq!(app.invalid_color_hex_count(), 0);
}

#[test]
fn removing_a_different_quick_color_preserves_a_half_typed_hex() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let _ = app.handle_quick_color_added();
    let last = app.draft.drawing_quick_colors.entries.len() - 1;
    let _ = app.handle_color_picker_hex_changed(ColorPickerId::QuickColor(0), "#12zz".to_string());

    let _ = app.handle_quick_color_removed(last);

    assert_eq!(
        app.color_picker_hex
            .get(&ColorPickerId::QuickColor(0))
            .map(String::as_str),
        Some("#12zz")
    );
    assert_eq!(app.invalid_color_hex_count(), 1);
}

/// Reordering remaps every surviving slot, so the editing text follows the
/// row it was moved with rather than staying at its old position.
#[test]
fn moving_a_quick_color_resyncs_the_pickers_that_survive() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    app.draft.drawing_quick_colors.entries[0].color.mode = ColorMode::Rgb;
    app.draft.drawing_quick_colors.entries[0].color.rgb =
        ["1".to_string(), "2".to_string(), "3".to_string()];
    app.draft.drawing_quick_colors.entries[1].color.mode = ColorMode::Rgb;
    app.draft.drawing_quick_colors.entries[1].color.rgb =
        ["4".to_string(), "5".to_string(), "6".to_string()];
    app.sync_all_color_picker_hex();
    let first = app
        .color_picker_hex
        .get(&ColorPickerId::QuickColor(0))
        .cloned();

    let _ = app.handle_quick_color_moved(0, 1);

    assert_eq!(
        app.color_picker_hex.get(&ColorPickerId::QuickColor(1)),
        first.as_ref()
    );
}

#[test]
fn adding_a_quick_color_preserves_a_half_typed_surviving_hex() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let _ = app.handle_color_picker_hex_changed(ColorPickerId::QuickColor(0), "#12zz".to_string());

    let _ = app.handle_quick_color_added();

    assert_eq!(
        app.color_picker_hex
            .get(&ColorPickerId::QuickColor(0))
            .map(String::as_str),
        Some("#12zz")
    );
    assert_eq!(app.invalid_color_hex_count(), 1);
}

#[test]
fn moving_a_quick_color_carries_its_half_typed_hex_with_it() {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let _ = app.handle_color_picker_hex_changed(ColorPickerId::QuickColor(0), "#12zz".to_string());

    let _ = app.handle_quick_color_moved(0, 1);

    assert_eq!(
        app.color_picker_hex
            .get(&ColorPickerId::QuickColor(1))
            .map(String::as_str),
        Some("#12zz")
    );
    assert_ne!(
        app.color_picker_hex
            .get(&ColorPickerId::QuickColor(0))
            .map(String::as_str),
        Some("#12zz")
    );
    assert_eq!(app.invalid_color_hex_count(), 1);
}

#[test]
fn quick_color_label_edit_does_not_change_slot_colors() {
    let (mut app, _effects) = ConfiguratorApp::new_app();

    let _ = app.handle_text_changed(TextField::QuickColorLabel(0), "RedNew".to_string());

    // The built-in defaults are named colors resolving to the tuned
    // palette, so the slots stay on their named values after a label edit.
    assert_eq!(app.draft.drawing_quick_colors.entries[0].color.name, "red");
    assert_eq!(
        app.draft.drawing_quick_colors.entries[1].color.name,
        "green"
    );
    assert_eq!(app.draft.drawing_quick_colors.entries[2].color.name, "blue");
    assert_eq!(
        app.draft.drawing_quick_colors.entries[0]
            .color
            .selected_named,
        NamedColorOption::Red
    );
}
