use super::*;
use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig};
use crate::draw::{Color, FontDescriptor};
use crate::input::{ClickHighlightSettings, EraserMode};

fn create_test_input_state() -> InputState {
    let keybindings = KeybindingsConfig::default();
    let action_map = keybindings.build_action_map().expect("action map");
    let action_bindings = keybindings
        .build_action_bindings()
        .expect("action bindings");

    let mut state = InputState::from_seed(crate::input::InputStateSeed {
        color: Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thickness: 3.0,
        eraser_size: 12.0,
        eraser_mode: EraserMode::Brush,
        marker_opacity: 0.32,
        fill_enabled: false,
        font_size: 32.0,
        font_descriptor: FontDescriptor {
            family: "Sans".to_string(),
            weight: "bold".to_string(),
            style: "normal".to_string(),
        },
        text_background_enabled: false,
        arrow_length: 20.0,
        arrow_angle: 30.0,
        arrow_head_at_end: false,
        show_status_bar: true,
        boards_config: BoardsConfig::default(),
        action_map: action_map,
        max_shapes_per_frame: usize::MAX,
        click_highlight_settings: ClickHighlightSettings::disabled(),
        undo_all_delay_ms: 0,
        redo_all_delay_ms: 0,
        custom_section_enabled: false,
        custom_undo_delay_ms: 0,
        custom_redo_delay_ms: 0,
        custom_undo_steps: 5,
        custom_redo_steps: 5,
        presenter_mode_config: PresenterModeConfig::default(),
    });
    state.set_action_bindings(action_bindings);
    state
}

#[test]
fn sample_eraser_path_points_densifies_long_segments() {
    let state = create_test_input_state();
    let points = vec![(0, 0), (20, 0)];
    let sampled = state.sample_eraser_path_points(&points);

    assert!(sampled.len() > points.len());
    assert_eq!(sampled.first().copied(), Some((0, 0)));
    assert_eq!(sampled.last().copied(), Some((20, 0)));
}

#[test]
fn sample_eraser_path_points_returns_borrowed_for_single_point() {
    let state = create_test_input_state();
    let points = vec![(5, 7)];
    let sampled = state.sample_eraser_path_points(&points);

    assert!(matches!(sampled, std::borrow::Cow::Borrowed(_)));
    assert_eq!(sampled.as_ref(), points.as_slice());
}

#[test]
fn sample_eraser_path_points_returns_borrowed_for_dense_segments() {
    let state = create_test_input_state();
    let points = vec![(0, 0), (2, 0), (4, 0)];
    let sampled = state.sample_eraser_path_points(&points);

    assert!(matches!(sampled, std::borrow::Cow::Borrowed(_)));
    assert_eq!(sampled.as_ref(), points.as_slice());
}

#[test]
fn sample_eraser_path_points_avoids_duplicate_points_when_sampling() {
    let state = create_test_input_state();
    let points = vec![(0, 0), (0, 20), (0, 20), (0, 40)];
    let sampled = state.sample_eraser_path_points(&points);

    assert!(matches!(sampled, std::borrow::Cow::Owned(_)));
    for pair in sampled.windows(2) {
        assert_ne!(pair[0], pair[1]);
    }
    assert_eq!(sampled.first().copied(), Some((0, 0)));
    assert_eq!(sampled.last().copied(), Some((0, 40)));
}
