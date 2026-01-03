use super::*;
use crate::config::{BoardConfig, KeybindingsConfig, PresenterModeConfig};
use crate::draw::{Color, FontDescriptor};
use crate::input::{ClickHighlightSettings, EraserMode};

fn create_test_input_state() -> InputState {
    let action_map = KeybindingsConfig::default()
        .build_action_map()
        .expect("action map");

    InputState::with_defaults(
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
    )
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
