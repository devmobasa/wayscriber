use super::super::events::HitKind;
use super::*;
use crate::config::{BoardConfig, KeybindingsConfig, PresenterModeConfig};
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
        BoardConfig::default(),
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
    assert_eq!(top_size(&snapshot), (810, 80));

    state.toolbar_use_icons = false;
    let snapshot = snapshot_from_state(&state);
    assert_eq!(top_size(&snapshot), (934, 56));
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
