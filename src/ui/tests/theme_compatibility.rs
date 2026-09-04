use crate::config::{StatusBarStyle, StatusPosition};
use crate::input::InputState;
use crate::ui::theme::{Theme, ThemeMode};
use crate::ui::*;

const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;

fn state() -> InputState {
    let mut state = crate::input::state::test_support::make_test_input_state();
    let style = StatusBarStyle::default();
    state.set_zoom_status(true, true, 2.0, (0.0, 0.0));
    state.update_status_hud_layout(StatusPosition::BottomLeft, &style, WIDTH, HEIGHT);
    state.update_zoom_chip_layout(&style, WIDTH, HEIGHT);
    state
}

fn pixels(draw: impl FnOnce(&cairo::Context)) -> Vec<u8> {
    let mut surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, WIDTH as i32, HEIGHT as i32).unwrap();
    {
        let cairo = cairo::Context::new(&surface).unwrap();
        draw(&cairo);
        assert_eq!(cairo.status(), Ok(()));
    }
    surface.flush();
    surface.data().unwrap().to_vec()
}

fn equal_pixels(actual: &[u8], expected: &[u8], label: &str) {
    assert_eq!(actual.len(), expected.len(), "{label}: length");
    assert!(
        actual == expected,
        "{label}: first differing byte {:?}",
        actual.iter().zip(expected).position(|(a, b)| a != b)
    );
}

#[test]
fn explicit_status_and_zoom_theme_changes_pixels_without_changing_geometry() {
    let state = state();
    let style = StatusBarStyle::default();
    let status_bounds = status_hud_geometry(&state, WIDTH, HEIGHT).unwrap();
    let zoom_bounds = zoom_chip_geometry(&state, WIDTH, HEIGHT).unwrap();
    let dark = Theme::dark();
    let light = Theme::light();
    let status = |theme: &Theme| {
        pixels(|ctx| render_status_bar_with_theme(ctx, theme, &state, &style, WIDTH, HEIGHT))
    };
    let zoom = |theme: &Theme| {
        pixels(|ctx| render_zoom_chip_with_theme(ctx, theme, &state, &style, WIDTH, HEIGHT))
    };
    assert!(
        status(&dark) != status(&light),
        "status hairline must use supplied theme"
    );
    assert!(
        zoom(&dark) != zoom(&light),
        "zoom hairline must use supplied theme"
    );
    let mut accent = dark.clone();
    accent.accent = (1.0, 0.0, 1.0, 1.0);
    assert!(
        zoom(&dark) != zoom(&accent),
        "active Lock text must use supplied accent"
    );
    assert_eq!(
        status_hud_geometry(&state, WIDTH, HEIGHT),
        Some(status_bounds)
    );
    assert_eq!(zoom_chip_geometry(&state, WIDTH, HEIGHT), Some(zoom_bounds));
}

#[test]
fn legacy_light_wrappers_match_explicit_light_in_isolated_process() {
    const CHILD: &str = "WAYSCRIBER_TEST_LEGACY_LIGHT_THEME";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "ui::theme_compatibility::legacy_light_wrappers_match_explicit_light_in_isolated_process", "--nocapture"])
            .env(CHILD, "1")
            .output().unwrap();
        assert!(
            output.status.success(),
            "isolated legacy theme test failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    // Invisible calls used to return before consulting the process theme.
    // Keep that behavior so a later configured Light theme can still install.
    let hidden = crate::input::state::test_support::make_test_input_state();
    let hidden_style = StatusBarStyle::default();
    pixels(|ctx| {
        render_status_bar(ctx, &hidden, &hidden_style, WIDTH, HEIGHT);
        render_zoom_chip(ctx, &hidden, &hidden_style, WIDTH, HEIGHT);
        render_radial_menu(ctx, &hidden, WIDTH, HEIGHT);
    });
    theme::init(ThemeMode::Light);
    assert_eq!(theme::current(), &Theme::light());
    let mut state = state();
    let style = StatusBarStyle::default();
    let light = Theme::light();
    let dark = Theme::dark();
    let legacy = pixels(|ctx| render_status_bar(ctx, &state, &style, WIDTH, HEIGHT));
    let explicit =
        pixels(|ctx| render_status_bar_with_theme(ctx, &light, &state, &style, WIDTH, HEIGHT));
    equal_pixels(&legacy, &explicit, "legacy status");
    assert!(
        legacy
            != pixels(|ctx| render_status_bar_with_theme(
                ctx, &dark, &state, &style, WIDTH, HEIGHT
            ))
    );
    let legacy = pixels(|ctx| render_zoom_chip(ctx, &state, &style, WIDTH, HEIGHT));
    let explicit =
        pixels(|ctx| render_zoom_chip_with_theme(ctx, &light, &state, &style, WIDTH, HEIGHT));
    equal_pixels(&legacy, &explicit, "legacy zoom");
    assert!(
        legacy
            != pixels(|ctx| render_zoom_chip_with_theme(ctx, &dark, &state, &style, WIDTH, HEIGHT))
    );
    state.open_radial_menu(400.0, 300.0);
    state.update_radial_menu_layout(WIDTH, HEIGHT);
    let legacy = pixels(|ctx| render_radial_menu(ctx, &state, WIDTH, HEIGHT));
    let engine = crate::ui_text::UiTextEngine::default();
    let explicit = |theme: &Theme| {
        pixels(|ctx| {
            let mut caches = UiRenderCaches::default();
            let mut render = UiRenderCtx {
                cairo: ctx,
                theme,
                caches: &mut caches,
            };
            render_radial_menu_with_context(&engine, &mut render, &state, WIDTH, HEIGHT);
        })
    };
    equal_pixels(&legacy, &explicit(&light), "legacy radial");
    assert!(legacy != explicit(&dark));
}
