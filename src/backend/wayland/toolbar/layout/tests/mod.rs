use super::super::events::HitKind;
use super::*;
use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig};
use crate::draw::{Color, FontDescriptor};
use crate::input::{ClickHighlightSettings, EraserMode, InputState};
use crate::ui::toolbar::{ToolbarBindingHints, ToolbarEvent, ToolbarSnapshot};

fn create_test_input_state() -> InputState {
    let keybindings = KeybindingsConfig::default();
    let action_map = keybindings.build_action_map().unwrap();
    let action_bindings = keybindings.build_action_bindings().unwrap();

    let mut state = InputState::with_defaults(
        Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        3.0,
        12.0,
        EraserMode::Brush,
        0.32,
        false,
        32.0,
        FontDescriptor {
            family: "Sans".to_string(),
            weight: "bold".to_string(),
            style: "normal".to_string(),
        },
        false,
        20.0,
        30.0,
        false,
        true,
        BoardsConfig::default(),
        action_map,
        usize::MAX,
        ClickHighlightSettings::disabled(),
        0,
        0,
        false,
        0,
        0,
        5,
        5,
        PresenterModeConfig::default(),
    );
    state.set_action_bindings(action_bindings);
    state
}

fn snapshot_from_state(state: &InputState) -> ToolbarSnapshot {
    ToolbarSnapshot::from_input_with_bindings(state, ToolbarBindingHints::default())
}

#[test]
fn top_size_respects_icon_mode() {
    let mut state = create_test_input_state();
    state.toolbar_use_icons = true;
    let snapshot = snapshot_from_state(&state);
    assert_eq!(top_size(&snapshot), (863, 72));

    state.toolbar_use_icons = false;
    let snapshot = snapshot_from_state(&state);
    assert_eq!(top_size(&snapshot), (980, 60));
}

#[test]
fn build_top_hits_includes_toggle_and_pin() {
    let mut state = create_test_input_state();
    state.toolbar_use_icons = true;
    let snapshot = snapshot_from_state(&state);
    let mut hits = Vec::new();
    let (w, h) = top_size(&snapshot);
    build_top_hits(w as f64, h as f64, &snapshot, &mut hits);

    assert!(
        hits.iter()
            .any(|hit| matches!(hit.event, ToolbarEvent::ToggleIconMode(false)))
    );
    assert!(
        hits.iter()
            .any(|hit| matches!(hit.event, ToolbarEvent::PinTopToolbar(_)))
    );
    assert!(
        hits.iter()
            .any(|hit| matches!(hit.event, ToolbarEvent::CloseTopToolbar))
    );
}

#[test]
fn build_side_hits_color_picker_height_tracks_palette_mode() {
    let mut state = create_test_input_state();
    state.show_more_colors = false;
    let snapshot = snapshot_from_state(&state);
    let mut hits = Vec::new();
    build_side_hits(260.0, 400.0, &snapshot, &mut hits);
    let picker_height = hits.iter().find_map(|hit| {
        if let HitKind::PickColor { h, .. } = hit.kind {
            Some(h)
        } else {
            None
        }
    });
    assert_eq!(picker_height, Some(24.0));

    state.show_more_colors = true;
    let snapshot = snapshot_from_state(&state);
    let mut hits = Vec::new();
    build_side_hits(260.0, 400.0, &snapshot, &mut hits);
    let picker_height = hits.iter().find_map(|hit| {
        if let HitKind::PickColor { h, .. } = hit.kind {
            Some(h)
        } else {
            None
        }
    });
    assert_eq!(picker_height, Some(54.0));
}

#[test]
fn top_size_scales_with_toolbar_scale() {
    let mut state = create_test_input_state();
    state.toolbar_use_icons = true;
    state.toolbar_scale = 1.0;
    let snapshot = snapshot_from_state(&state);
    let base_size = top_size(&snapshot);

    // Scale 1.5x should increase size proportionally
    state.toolbar_scale = 1.5;
    let snapshot = snapshot_from_state(&state);
    let scaled_size = top_size(&snapshot);
    assert_eq!(
        scaled_size.0,
        (base_size.0 as f64 * 1.5).ceil() as u32,
        "Width should scale by 1.5x"
    );
    assert_eq!(
        scaled_size.1,
        (base_size.1 as f64 * 1.5).ceil() as u32,
        "Height should scale by 1.5x"
    );

    // Scale 0.75x should decrease size
    state.toolbar_scale = 0.75;
    let snapshot = snapshot_from_state(&state);
    let small_size = top_size(&snapshot);
    assert!(
        small_size.0 < base_size.0,
        "Scaled down width should be smaller"
    );
    assert!(
        small_size.1 < base_size.1,
        "Scaled down height should be smaller"
    );
}

#[test]
fn scale_size_handles_non_finite_values() {
    let mut state = create_test_input_state();
    state.toolbar_use_icons = true;
    state.toolbar_scale = 1.0;
    let snapshot = snapshot_from_state(&state);
    let base_size = top_size(&snapshot);

    // NaN should fall back to 1.0
    state.toolbar_scale = f64::NAN;
    let snapshot = snapshot_from_state(&state);
    let nan_size = top_size(&snapshot);
    assert_eq!(nan_size, base_size, "NaN scale should fall back to 1.0");

    // Infinity should fall back to 1.0
    state.toolbar_scale = f64::INFINITY;
    let snapshot = snapshot_from_state(&state);
    let inf_size = top_size(&snapshot);
    assert_eq!(
        inf_size, base_size,
        "Infinity scale should fall back to 1.0"
    );

    // Negative infinity should fall back to 1.0
    state.toolbar_scale = f64::NEG_INFINITY;
    let snapshot = snapshot_from_state(&state);
    let neg_inf_size = top_size(&snapshot);
    assert_eq!(
        neg_inf_size, base_size,
        "Neg infinity scale should fall back to 1.0"
    );
}

#[test]
fn scale_size_clamps_extreme_values() {
    let mut state = create_test_input_state();
    state.toolbar_use_icons = true;

    // Test upper bound clamping (max 3.0)
    state.toolbar_scale = 10.0;
    let snapshot = snapshot_from_state(&state);
    let huge_size = top_size(&snapshot);

    state.toolbar_scale = 3.0;
    let snapshot = snapshot_from_state(&state);
    let max_size = top_size(&snapshot);
    assert_eq!(huge_size, max_size, "Scale > 3.0 should clamp to 3.0");

    // Test lower bound clamping (min 0.5)
    state.toolbar_scale = 0.1;
    let snapshot = snapshot_from_state(&state);
    let tiny_size = top_size(&snapshot);

    state.toolbar_scale = 0.5;
    let snapshot = snapshot_from_state(&state);
    let min_size = top_size(&snapshot);
    assert_eq!(tiny_size, min_size, "Scale < 0.5 should clamp to 0.5");
}
