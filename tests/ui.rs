use cairo::{Context, ImageSurface};
use wayscriber::config::{
    Action, HelpOverlayStyle, KeybindingsConfig, PresenterModeConfig, StatusBarStyle,
    StatusPosition,
};
use wayscriber::draw::{Color, Shape};
use wayscriber::input::{
    BOARD_ID_BLACKBOARD, BOARD_ID_WHITEBOARD, ClickHighlightSettings, EraserMode, InputState,
};

fn make_input_state() -> InputState {
    let keybindings = KeybindingsConfig::default();
    let action_map = keybindings.build_action_map().unwrap();
    let action_bindings = keybindings.build_action_bindings().unwrap();
    let mut input = InputState::with_defaults(
        Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        4.0,
        4.0,
        EraserMode::Brush,
        0.32,
        false,
        32.0,
        wayscriber::draw::FontDescriptor::default(),
        false,
        20.0,
        30.0,
        false,
        true,
        wayscriber::config::BoardsConfig::default(),
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
        PresenterModeConfig::default(),
    );
    input.set_action_bindings(action_bindings);
    input
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

fn alpha_at(surface: &mut ImageSurface, x: i32, y: i32) -> u8 {
    let stride = surface.stride() as usize;
    let offset = y as usize * stride + x as usize * 4 + 3;
    surface.data().unwrap()[offset]
}

fn alpha_bounds(surface: &mut ImageSurface) -> Option<(i32, i32, i32, i32)> {
    let width = surface.width();
    let height = surface.height();
    let stride = surface.stride() as usize;
    let data = surface.data().ok()?;
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = -1;
    let mut max_y = -1;

    for y in 0..height {
        for x in 0..width {
            let alpha = data[y as usize * stride + x as usize * 4 + 3];
            if alpha == 0 {
                continue;
            }
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }

    (max_x >= 0).then_some((min_x, min_y, max_x, max_y))
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
        input.update_status_hud_layout(position, &style, 400, 200);
        wayscriber::ui::render_status_bar(&ctx, &input, &style, 400, 200);
        drop(ctx);
        assert!(
            surface_has_pixels(&mut surface),
            "status bar should render pixels for {position:?}"
        );
    }
}

#[test]
fn render_help_overlay_draws_content() {
    let style = HelpOverlayStyle::default();
    let (mut surface, ctx) = surface_with_context(800, 600);
    let input = make_input_state();
    let bindings = wayscriber::ui::HelpOverlayBindings::from_input_state(&input);
    wayscriber::ui::render_help_overlay(
        &ctx, &style, 800, 600, true, 0, &bindings, "", false, true, true, 0.0, false,
    );
    drop(ctx);
    assert!(surface_has_pixels(&mut surface));
}

#[test]
fn render_status_bar_draws_in_board_modes() {
    let mut input = make_input_state();
    input.update_screen_dimensions(800, 480);
    let style = StatusBarStyle::default();

    let board_ids = [BOARD_ID_WHITEBOARD, BOARD_ID_BLACKBOARD];

    for board_id in board_ids {
        input.switch_board(board_id);
        let (mut surface, ctx) = surface_with_context(400, 200);
        input.update_status_hud_layout(StatusPosition::BottomLeft, &style, 400, 200);
        wayscriber::ui::render_status_bar(&ctx, &input, &style, 400, 200);
        drop(ctx);
        assert!(
            surface_has_pixels(&mut surface),
            "status bar should render pixels for board {board_id}"
        );
    }
}

#[test]
fn render_help_overlay_without_frozen_shortcuts_draws_content() {
    let style = HelpOverlayStyle::default();
    let (mut surface, ctx) = surface_with_context(800, 600);
    let input = make_input_state();
    let bindings = wayscriber::ui::HelpOverlayBindings::from_input_state(&input);
    wayscriber::ui::render_help_overlay(
        &ctx, &style, 800, 600, false, 0, &bindings, "", false, true, true, 0.0, false,
    );
    drop(ctx);
    assert!(surface_has_pixels(&mut surface));
}

#[test]
fn render_command_palette_with_query_draws_content() {
    // Exercises the match-highlight path: an open palette with a query that
    // literally appears in some command labels must render without panicking
    // and draw pixels (highlight boxes + rows).
    let (mut surface, ctx) = surface_with_context(900, 700);
    let mut input = make_input_state();
    input.command_palette_open = true;
    input.command_palette_query = "tool".to_string();

    wayscriber::ui::render_command_palette(&ctx, &input, 900, 700);

    drop(ctx);
    assert!(surface_has_pixels(&mut surface));
}

#[test]
fn render_keybinding_capture_draws_content() {
    let (mut surface, ctx) = surface_with_context(800, 600);
    let mut input = make_input_state();
    input.keybinding_capture_action = Some(Action::SelectPenTool);

    wayscriber::ui::render_command_palette(&ctx, &input, 800, 600);

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

#[test]
fn render_shape_ellipse_does_not_connect_to_existing_current_path() {
    let (mut surface, ctx) = surface_with_context(120, 120);
    let magenta = Color {
        r: 1.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    ctx.move_to(10.0, 90.0);
    wayscriber::draw::render_shape(
        &ctx,
        &Shape::Ellipse {
            cx: 80,
            cy: 20,
            rx: 20,
            ry: 10,
            fill: false,
            color: magenta,
            thick: 6.0,
        },
    );

    drop(ctx);
    assert_eq!(
        alpha_at(&mut surface, 48, 60),
        0,
        "ellipse rendering must not connect to a path left by prior drawing"
    );
    assert!(
        alpha_at(&mut surface, 100, 20) > 0,
        "ellipse stroke should still render"
    );
}

#[test]
fn render_onboarding_card_tiny_surface_does_not_panic() {
    let (mut surface, ctx) = surface_with_context(200, 40);
    let card = wayscriber::ui::OnboardingCard {
        eyebrow: "First-run onboarding".to_string(),
        title: "Draw one mark".to_string(),
        body: "Make one quick stroke to start.".to_string(),
        items: vec![wayscriber::ui::OnboardingChecklistItem {
            label: "Draw a stroke".to_string(),
            done: false,
        }],
        footer: "Shift+Escape to skip".to_string(),
    };

    wayscriber::ui::render_onboarding_card(&ctx, 200, 40, &card);
    drop(ctx);
    assert!(surface_has_pixels(&mut surface));
}

#[test]
fn render_onboarding_card_without_checklist_stays_compact() {
    let (mut surface, ctx) = surface_with_context(700, 500);
    let compact_card = wayscriber::ui::OnboardingCard {
        eyebrow: "Step 1 / 5".to_string(),
        title: "Enable background mode?".to_string(),
        body: "Keeps Wayscriber ready in the background for quick overlay access.".to_string(),
        items: Vec::new(),
        footer: "Y = set up now • N = skip • Shift+Escape = skip onboarding".to_string(),
    };

    wayscriber::ui::render_onboarding_card(&ctx, 700, 500, &compact_card);
    drop(ctx);
    let (_, compact_min_y, _, compact_max_y) =
        alpha_bounds(&mut surface).expect("rendered compact onboarding card");
    let compact_height = compact_max_y - compact_min_y;

    let (mut checklist_surface, checklist_ctx) = surface_with_context(700, 500);
    let checklist_card = wayscriber::ui::OnboardingCard {
        items: vec![wayscriber::ui::OnboardingChecklistItem {
            label: "Open the configurator".to_string(),
            done: false,
        }],
        ..compact_card
    };
    wayscriber::ui::render_onboarding_card(&checklist_ctx, 700, 500, &checklist_card);
    drop(checklist_ctx);
    let (_, checklist_min_y, _, checklist_max_y) =
        alpha_bounds(&mut checklist_surface).expect("rendered checklist onboarding card");
    let checklist_height = checklist_max_y - checklist_min_y;

    assert!(
        compact_height <= 190,
        "zero-item onboarding card should remain compact; compact_height={compact_height}"
    );
    assert!(
        checklist_height >= compact_height + 20,
        "zero-item onboarding card should not reserve checklist space; compact_height={compact_height}, checklist_height={checklist_height}"
    );
}
