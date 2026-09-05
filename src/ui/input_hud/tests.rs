use super::*;
use crate::config::InputHudConfig;
use crate::input::state::{InputHudSettings, test_support::make_test_input_state};
use crate::input::{Key, Modifiers};

fn state_with(config: InputHudConfig) -> InputState {
    let mut state = make_test_input_state();
    state.init_input_hud_from_config(InputHudSettings::from(&config));
    state
}

fn enabled_state() -> InputState {
    state_with(InputHudConfig {
        enabled: true,
        ..InputHudConfig::default()
    })
}

#[test]
fn hidden_hud_has_no_geometry() {
    let state = state_with(InputHudConfig::default());
    assert!(input_hud_geometry(&state, 1920, 1080).is_none());

    let empty = enabled_state();
    assert!(
        input_hud_geometry(&empty, 1920, 1080).is_none(),
        "an enabled but empty HUD draws nothing"
    );
}

#[test]
fn bottom_center_row_is_centered_and_bottom_anchored() {
    let mut state = enabled_state();
    state.note_input_hud_key(Key::Char('a'), Modifiers::new());
    let (x, y, width, height) = input_hud_geometry(&state, 1920, 1080).expect("row geometry");

    assert!((x + width / 2.0 - 960.0).abs() < 1e-6, "row is centered");
    assert!(
        (y + height - (1080.0 - INPUT_HUD_EDGE_INSET)).abs() < 1e-6,
        "row sits one inset above the bottom edge"
    );
}

#[test]
fn anchors_place_the_row_on_the_requested_edges() {
    for position in [
        InputHudPosition::TopLeft,
        InputHudPosition::TopCenter,
        InputHudPosition::TopRight,
        InputHudPosition::CenterLeft,
        InputHudPosition::Center,
        InputHudPosition::CenterRight,
        InputHudPosition::BottomLeft,
        InputHudPosition::BottomRight,
    ] {
        let mut state = state_with(InputHudConfig {
            enabled: true,
            position,
            ..InputHudConfig::default()
        });
        state.note_input_hud_key(Key::Char('a'), Modifiers::new());
        let (x, y, width, height) = input_hud_geometry(&state, 1920, 1080).expect("row geometry");

        if position.is_right() {
            assert!((x + width - (1920.0 - INPUT_HUD_EDGE_INSET)).abs() < 1e-6);
        } else if position.is_center() {
            assert!((x + width / 2.0 - 960.0).abs() < 1e-6, "row is centered");
        } else {
            assert!((x - INPUT_HUD_EDGE_INSET).abs() < 1e-6);
        }
        if position.is_top() {
            assert!((y - INPUT_HUD_EDGE_INSET).abs() < 1e-6);
        } else if position.is_middle() {
            assert!(
                (y + height / 2.0 - 540.0).abs() < 1e-6,
                "middle anchors sit on the vertical center line"
            );
        } else {
            assert!(y > 540.0, "bottom anchors stay in the lower half");
        }
    }
}

/// A valid-but-large font on a narrow output can make a single chip wider
/// than the inset span. The newest chip must clamp to it (rendering clips
/// the label at the box edge) so the row never leaves the surface.
#[test]
fn the_newest_chip_clamps_to_the_available_width() {
    let mut state = state_with(InputHudConfig {
        enabled: true,
        font_size: 72.0,
        ..InputHudConfig::default()
    });
    let mut modifiers = Modifiers::new();
    modifiers.ctrl = true;
    modifiers.shift = true;
    state.note_input_hud_key(Key::Backspace, modifiers);

    let screen_width = 160_u32;
    let available = screen_width as f64 - INPUT_HUD_EDGE_INSET * 2.0;
    let layout = compute_input_hud_layout(&UiTextEngine::default(), &state, screen_width, 1080)
        .expect("layout");
    assert_eq!(layout.chips.len(), 1);
    assert!(
        layout.width <= available,
        "row width {} must not exceed the available span {available}",
        layout.width
    );
    assert!(layout.x >= 0.0);
    assert!(layout.x + layout.width <= screen_width as f64);

    // A surface no wider than its insets has no drawable span at all.
    let no_span = (INPUT_HUD_EDGE_INSET * 2.0) as u32;
    assert!(compute_input_hud_layout(&UiTextEngine::default(), &state, no_span, 1080).is_none());
}

#[test]
fn repeat_counter_is_appended_to_the_chip_text() {
    let mut state = enabled_state();
    for _ in 0..7 {
        state.note_input_hud_key(Key::Backspace, Modifiers::new());
    }
    let layout =
        compute_input_hud_layout(&UiTextEngine::default(), &state, 1920, 1080).expect("layout");
    assert_eq!(layout.chips.len(), 1);
    assert_eq!(layout.chips[0].text, "Backspace \u{00d7}7");
}

#[test]
fn mouse_chips_keep_the_pill_chrome() {
    let mut state = enabled_state();
    state.note_input_hud_mouse("Click", Modifiers::new());
    state.note_input_hud_scroll(true, Modifiers::new());
    let layout =
        compute_input_hud_layout(&UiTextEngine::default(), &state, 1920, 1080).expect("layout");
    assert_eq!(layout.chips.len(), 2);
    assert_eq!(layout.chips[0].kind, InputHudEntryKind::Mouse);
    assert_eq!(layout.chips[1].kind, InputHudEntryKind::Scroll);
}

/// The row never runs off screen: a narrow surface keeps only the newest
/// chips that fit inside the inset-reduced width.
#[test]
fn overlong_rows_drop_their_oldest_chips() {
    let mut state = enabled_state();
    for label in ['a', 'b', 'c', 'd', 'e', 'f'] {
        state.note_input_hud_key(Key::Char(label), Modifiers::new());
    }
    let layout =
        compute_input_hud_layout(&UiTextEngine::default(), &state, 120, 1080).expect("layout");

    assert!(layout.chips.len() < 6, "narrow screens shed older chips");
    assert!(layout.x >= 0.0);
    assert!(layout.x + layout.width <= 120.0 + 1e-6);
    assert_eq!(
        layout.chips.last().map(|chip| chip.text.as_str()),
        Some("F"),
        "the newest chip always survives"
    );
}

/// Chips share one row height so labels with different ascenders and
/// descenders still align.
#[test]
fn chips_share_a_single_row_height() {
    let mut state = enabled_state();
    state.note_input_hud_key(Key::Backspace, Modifiers::new());
    state.note_input_hud_key(Key::Escape, Modifiers::new());
    let layout =
        compute_input_hud_layout(&UiTextEngine::default(), &state, 1920, 1080).expect("layout");

    assert_eq!(layout.chips.len(), 2);
    for chip in &layout.chips {
        let (_, natural) = keycap_box_size(
            &UiTextEngine::default(),
            &chip.text,
            state.input_hud_font_size(),
        )
        .expect("chip measurement");
        assert!(natural <= layout.height + 1e-6);
    }
}

fn paint(engine: &UiTextEngine, layout: &InputHudLayout, density: i32) -> Vec<u8> {
    let mut surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, 360 * density, 180 * density).unwrap();
    {
        let ctx = cairo::Context::new(&surface).unwrap();
        ctx.scale(f64::from(density), f64::from(density));
        paint_input_hud_layout(engine, &ctx, &StatusBarStyle::default(), 18.0, layout);
    }
    surface.data().unwrap().to_vec()
}

#[test]
fn retained_owner_matches_fresh_geometry_and_faded_clipped_pixels_across_densities() {
    let engine = UiTextEngine::default();
    let mut state = state_with(InputHudConfig {
        enabled: true,
        font_size: 18.0,
        ..InputHudConfig::default()
    });
    state.note_input_hud_key(Key::Backspace, Modifiers::new());
    state.note_input_hud_mouse("Click", Modifiers::new());
    state.note_input_hud_scroll(true, Modifiers::new());
    for (width, density) in [(360, 1), (80, 2), (360, 1)] {
        let fresh = UiTextEngine::default();
        assert_eq!(
            input_hud_geometry_with_engine(&engine, &state, width, 180),
            input_hud_geometry_with_engine(&fresh, &state, width, 180)
        );
        let mut actual_layout = compute_input_hud_layout(&engine, &state, width, 180).unwrap();
        let mut fresh_layout = compute_input_hud_layout(&fresh, &state, width, 180).unwrap();
        assert_eq!(actual_layout.chips.len(), fresh_layout.chips.len());
        for (actual, expected) in actual_layout.chips.iter_mut().zip(&mut fresh_layout.chips) {
            assert_eq!(
                (&actual.text, actual.kind, actual.x, actual.width),
                (&expected.text, expected.kind, expected.x, expected.width)
            );
            // Resolve identical frame alpha: elapsed wall time is not part of owner parity.
            actual.alpha = 0.4;
            expected.alpha = 0.4;
        }
        let actual = paint(&engine, &actual_layout, density);
        assert!(actual.iter().any(|&byte| byte != 0));
        assert!(
            actual == paint(&fresh, &fresh_layout, density),
            "retained HUD pixels differ"
        );
        for chip in &mut fresh_layout.chips {
            chip.alpha = 1.0;
        }
        assert!(
            actual != paint(&fresh, &fresh_layout, density),
            "resolved fade changes output"
        );
        let stride = 360 * density as usize * 4;
        for (index, _) in actual
            .as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .filter(|(_, p)| p[3] != 0)
        {
            let x = (index * 4 % stride) as f64 / (4.0 * f64::from(density));
            let y = (index * 4 / stride) as f64 / f64::from(density);
            assert!(
                x >= actual_layout.x.floor() && x < (actual_layout.x + actual_layout.width).ceil()
            );
            assert!(
                y >= actual_layout.y.floor() && y < (actual_layout.y + actual_layout.height).ceil()
            );
        }
    }
}
