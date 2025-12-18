use cairo::{Context, ImageSurface};
use wayscriber::config::{HelpOverlayStyle, KeybindingsConfig, StatusBarStyle, StatusPosition};
use wayscriber::draw::Color;
use wayscriber::input::{ClickHighlightSettings, InputState};

fn make_input_state() -> InputState {
    let keybindings = KeybindingsConfig::default();
    let action_map = keybindings.build_action_map().unwrap();
    InputState::with_defaults(
        Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        4.0,
        4.0,
        0.32,
        false,
        32.0,
        wayscriber::draw::FontDescriptor::default(),
        false,
        20.0,
        30.0,
        false,
        true,
        wayscriber::config::BoardConfig::default(),
        action_map,
        usize::MAX,
        ClickHighlightSettings::disabled(),
        0,
        0,
        true,
        0,
        0,
        5,
        5,
    )
}

fn surface_with_context(width: i32, height: i32) -> (ImageSurface, Context) {
    let surface = ImageSurface::create(cairo::Format::ARgb32, width, height).unwrap();
    let ctx = Context::new(&surface).unwrap();
    (surface, ctx)
}

fn surface_has_pixels(surface: &mut ImageSurface) -> bool {
    surface
        .data()
        .map(|data| data.iter().any(|byte| *byte != 0))
        .unwrap_or(false)
}

#[test]
fn render_status_bar_draws_for_all_positions() {
    let mut input = make_input_state();
    input.update_screen_dimensions(800, 480);
    let style = StatusBarStyle::default();
    let positions = [
        StatusPosition::TopLeft,
        StatusPosition::TopRight,
        StatusPosition::BottomLeft,
        StatusPosition::BottomRight,
    ];

    for position in positions {
        let (mut surface, ctx) = surface_with_context(400, 200);
        wayscriber::ui::render_status_bar(&ctx, &input, position, &style, 400, 200);
        drop(ctx);
        assert!(
            surface_has_pixels(&mut surface),
            "status bar should render pixels for {:?}",
            position
        );
    }
}

#[test]
fn render_help_overlay_draws_content() {
    let style = HelpOverlayStyle::default();
    let (mut surface, ctx) = surface_with_context(800, 600);
    wayscriber::ui::render_help_overlay(&ctx, &style, 800, 600, true);
    drop(ctx);
    assert!(surface_has_pixels(&mut surface));
}

#[test]
fn render_status_bar_draws_in_board_modes() {
    let mut input = make_input_state();
    input.update_screen_dimensions(800, 480);
    let style = StatusBarStyle::default();

    let modes = [
        wayscriber::input::BoardMode::Whiteboard,
        wayscriber::input::BoardMode::Blackboard,
    ];

    for mode in modes {
        input.switch_board_mode(mode);
        let (mut surface, ctx) = surface_with_context(400, 200);
        wayscriber::ui::render_status_bar(
            &ctx,
            &input,
            StatusPosition::BottomLeft,
            &style,
            400,
            200,
        );
        drop(ctx);
        assert!(
            surface_has_pixels(&mut surface),
            "status bar should render pixels for mode {:?}",
            mode
        );
    }
}

#[test]
fn render_help_overlay_without_frozen_shortcuts_draws_content() {
    let style = HelpOverlayStyle::default();
    let (mut surface, ctx) = surface_with_context(800, 600);
    wayscriber::ui::render_help_overlay(&ctx, &style, 800, 600, false);
    drop(ctx);
    assert!(surface_has_pixels(&mut surface));
}

#[test]
fn render_frozen_badge_draws_pixels() {
    let (mut surface, ctx) = surface_with_context(400, 200);
    wayscriber::ui::render_frozen_badge(&ctx, 400, 200);
    drop(ctx);
    assert!(surface_has_pixels(&mut surface));
}
