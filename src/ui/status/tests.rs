use super::*;
use crate::config::{StatusBarStyle, StatusPosition};
use crate::input::InputState;
use crate::ui::theme::Theme;
use crate::ui_text::UiTextEngine;

fn state() -> InputState {
    crate::input::state::test_support::make_test_input_state()
}

fn update(engine: &UiTextEngine, state: &mut InputState, style: &StatusBarStyle, focused: bool) {
    state.update_status_hud_layout_for_pointer_with_engine(
        engine,
        StatusPosition::BottomLeft,
        style,
        1280,
        720,
        focused,
    );
    state.update_zoom_chip_layout_for_pointer_with_engine(engine, style, 1280, 720, focused);
}

fn paint(
    engine: &UiTextEngine,
    state: &InputState,
    style: &StatusBarStyle,
    theme: &Theme,
    density: i32,
    standalone: bool,
) -> Vec<u8> {
    let mut surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, 1280 * density, 720 * density).unwrap();
    {
        let ctx = cairo::Context::new(&surface).unwrap();
        ctx.scale(f64::from(density), f64::from(density));
        if standalone {
            render_status_bar_with_theme(&ctx, theme, state, style, 1280, 720);
            render_zoom_chip_with_theme(&ctx, theme, state, style, 1280, 720);
        } else {
            render_status_bar_with_resources(engine, &ctx, theme, state, style, 1280, 720);
            render_zoom_chip_with_resources(engine, &ctx, theme, state, style, 1280, 720);
        }
    }
    surface.flush();
    surface.data().unwrap().to_vec()
}

#[test]
fn explicit_status_and_zoom_frames_match_standalone_after_target_rebinding() {
    let engine = UiTextEngine::default();
    let mut input = state();
    for (font_size, zoom, density) in [(10.0, 1.0, 1), (18.0, 2.0, 2), (10.0, 1.0, 1)] {
        let style = StatusBarStyle {
            font_size,
            ..StatusBarStyle::default()
        };
        input.set_zoom_status(zoom != 1.0, false, zoom, (0.0, 0.0));
        update(&engine, &mut input, &style, true);
        let hud = input.status_hud_layout().unwrap();
        let chip = input.zoom_chip_layout().unwrap();
        // Compare the complete numeric layout, including runs and hit targets.
        assert_eq!(
            format!("{hud:?}"),
            format!(
                "{:?}",
                compute_status_hud_layout(&input, StatusPosition::BottomLeft, &style, 1280, 720)
                    .unwrap()
            )
        );
        assert_eq!(
            format!("{chip:?}"),
            format!(
                "{:?}",
                compute_zoom_chip_layout(&input, &style, 1280, 720).unwrap()
            )
        );
        for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            let pixels = paint(&engine, &input, &style, &theme, density, false);
            assert!(
                pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|pixel| pixel.iter().any(|byte| *byte != 0))
                    .count()
                    > 1000
            );
            assert!(
                pixels
                    == paint(
                        &UiTextEngine::default(),
                        &input,
                        &style,
                        &theme,
                        density,
                        false
                    ),
                "fresh-owner paint differs: theme={theme_name}, density={density}, font={font_size}, zoom={zoom}"
            );
            assert!(
                pixels == paint(&engine, &input, &style, &theme, density, true),
                "standalone paint differs: theme={theme_name}, density={density}, font={font_size}, zoom={zoom}"
            );
        }
        // Painting on a scaled target must not contaminate the next headless layout.
        let before = (
            format!("{:?}", input.status_hud_layout()),
            format!("{:?}", input.zoom_chip_layout()),
        );
        update(&engine, &mut input, &style, true);
        assert_eq!(
            before,
            (
                format!("{:?}", input.status_hud_layout()),
                format!("{:?}", input.zoom_chip_layout())
            )
        );
    }
}

#[test]
fn explicit_frame_layout_rehits_stationary_pointer_and_clears_unfocused_hover() {
    let engine = UiTextEngine::default();
    let style = StatusBarStyle::default();
    let mut input = state();
    update(&engine, &mut input, &style, true);
    let help = input
        .status_hud_layout()
        .unwrap()
        .segments
        .iter()
        .find(|s| s.kind == StatusHudSegmentKind::Help)
        .unwrap();
    let (x, y) = (
        (help.x + help.width / 2.0).round() as i32,
        (help.y + help.height / 2.0).round() as i32,
    );
    input.on_mouse_motion(x, y);
    assert_eq!(input.status_hud.hover(), Some(StatusHudSegmentKind::Help));
    update(&engine, &mut input, &style, false);
    assert_eq!(input.status_hud.hover(), None);
    update(&engine, &mut input, &style, true);
    assert_eq!(input.status_hud.hover(), Some(StatusHudSegmentKind::Help));

    input.set_zoom_status(true, false, 2.0, (0.0, 0.0));
    update(&engine, &mut input, &style, true);
    let fit = input
        .zoom_chip_layout()
        .unwrap()
        .buttons
        .iter()
        .find(|b| b.kind == ZoomChipButtonKind::Fit)
        .unwrap();
    let (x, y) = (
        (fit.x + fit.width / 2.0).round() as i32,
        (fit.y + fit.height / 2.0).round() as i32,
    );
    input.on_mouse_motion_with_canvas(x, y, x, y);
    assert_eq!(input.zoom_chip.hover(), Some(ZoomChipButtonKind::Fit));
    input.set_zoom_status(false, false, 1.0, (0.0, 0.0));
    update(&engine, &mut input, &style, true);
    let expected = input
        .zoom_chip_layout()
        .unwrap()
        .button_at(f64::from(x), f64::from(y));
    assert_ne!(expected, Some(ZoomChipButtonKind::Fit));
    assert_eq!(input.zoom_chip.hover(), expected);
    update(&engine, &mut input, &style, false);
    assert_eq!(input.zoom_chip.hover(), None);

    input.ui_visibility.show_status_bar = false;
    input.ui_visibility.show_zoom_chip = false;
    update(&engine, &mut input, &style, true);
    assert!(input.status_hud_layout().is_none());
    assert!(input.zoom_chip_layout().is_none());
}

#[test]
fn explicit_status_prefix_and_stacked_badge_paint_match_standalone() {
    let engine = UiTextEngine::default();
    let mut input = state();
    input.ui_visibility.show_zoom_chip = false;
    input.ui_visibility.show_active_output_badge = true;
    input.set_active_output_label(Some("DP-3 Dell UltraSharp U2723QE 3840x2160@60".into()));
    input.set_zoom_status(true, false, 2.0, (0.0, 0.0));
    let style = StatusBarStyle {
        font_size: 28.0,
        ..StatusBarStyle::default()
    };
    update(&engine, &mut input, &style, true);
    let layout = input.status_hud_layout().unwrap();
    assert!(
        layout
            .prefix
            .as_ref()
            .is_some_and(|prefix| prefix.height > 0.0)
    );
    assert!(
        layout
            .badges
            .iter()
            .any(|badge| badge.label.contains("ZOOM"))
    );
    assert!(input.zoom_chip_layout().is_none());
    let theme = Theme::dark();
    for density in [1, 2, 1] {
        assert!(
            paint(&engine, &input, &style, &theme, density, false)
                == paint(&engine, &input, &style, &theme, density, true),
            "prefix/badge paint differs: theme=dark, density={density}"
        );
    }
}
