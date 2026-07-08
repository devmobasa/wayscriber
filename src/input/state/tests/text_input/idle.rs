use super::super::*;
use crate::config::{QuickColorPalette, QuickColorPaletteEntry};

#[test]
fn test_idle_mode_plain_letters_trigger_color_actions() {
    let mut state = create_test_input_state();

    // Should be in Idle mode
    assert!(matches!(state.state, DrawingState::Idle));

    let original_color = state.current_color;

    // Press 'g' for green
    state.on_key_press(Key::Char('g'));

    // Color should have changed
    assert_ne!(state.current_color, original_color);
    assert_eq!(
        state.current_color,
        QuickColorPalette::default().color_for_index(1).unwrap()
    );
}

#[test]
fn idle_mode_plain_letters_use_configured_quick_palette() {
    let mut state = create_test_input_state();
    let configured_green = Color {
        r: 0.18,
        g: 0.28,
        b: 0.38,
        a: 1.0,
    };
    state.set_quick_colors(QuickColorPalette::from_entries(vec![
        QuickColorPaletteEntry {
            label: "First".to_string(),
            color: crate::draw::color::RED,
        },
        QuickColorPaletteEntry {
            label: "Configured".to_string(),
            color: configured_green,
        },
    ]));

    state.on_key_press(Key::Char('g'));

    assert_eq!(state.current_color, configured_green);
}
