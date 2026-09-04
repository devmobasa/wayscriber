use super::*;
use crate::draw::Color;
use crate::input::Tool;
use crate::input::state::test_support::make_test_input_state;

fn state() -> InputState {
    let mut state = make_test_input_state();
    state.open_radial_menu(250.0, 250.0);
    state.update_radial_menu_layout(500, 500);
    state
}

fn paint(
    engine: &UiTextEngine,
    cache: &mut RadialBaseCache,
    state: &InputState,
    theme: &theme::Theme,
    scale: f64,
) -> Vec<u8> {
    let mut output = ImageSurface::create(
        Format::ARgb32,
        (500.0 * scale) as i32,
        (500.0 * scale) as i32,
    )
    .unwrap();
    {
        let ctx = Context::new(&output).unwrap();
        ctx.scale(scale, scale);
        cache.paint_base(
            engine,
            &ctx,
            state,
            &state.radial_menu.layout().unwrap(),
            theme,
            &state.radial_ring_swatches(),
        );
    }
    output.data().unwrap().to_vec()
}

fn assert_replacement(
    engine: &UiTextEngine,
    cache: &mut RadialBaseCache,
    state: &InputState,
    theme: &theme::Theme,
    scale: f64,
) -> Vec<u8> {
    let prior = cache.cached.as_ref().map(|entry| entry.surface.clone());
    let actual = paint(engine, cache, state, theme, scale);
    if let Some(prior) = prior {
        assert_ne!(
            prior.to_raw_none(),
            cache.cached.as_ref().unwrap().surface.to_raw_none(),
            "changed input must replace the surface"
        );
    }
    assert!(
        actual == paint(engine, &mut RadialBaseCache::default(), state, theme, scale),
        "fresh base pixels differ"
    );
    actual
}

#[test]
fn theme_and_density_replacement_matches_fresh_pixels_and_hits_reuse_surfaces() {
    let engine = UiTextEngine::default();
    let state = state();
    let mut cache = RadialBaseCache::default();
    for scale in [1.0, 2.0] {
        let mut previous = None;
        for theme in [
            theme::Theme::dark(),
            theme::Theme::light(),
            theme::Theme::dark(),
        ] {
            let actual = assert_replacement(&engine, &mut cache, &state, &theme, scale);
            assert!(actual.iter().any(|byte| *byte != 0));
            if let Some(previous) = previous {
                assert!(actual != previous, "theme switch must change pixels");
            }
            let surface = cache.cached.as_ref().unwrap().surface.clone();
            assert_eq!(surface.device_scale(), (scale, scale));
            assert!(paint(&engine, &mut cache, &state, &theme, scale) == actual);
            assert_eq!(
                surface.to_raw_none(),
                cache.cached.as_ref().unwrap().surface.to_raw_none(),
                "unchanged input must reuse the allocation"
            );
            previous = Some(actual);
        }
    }
}

#[test]
fn independent_owners_keep_their_own_surfaces() {
    let engine = UiTextEngine::default();
    let second_engine = UiTextEngine::default();
    let state = state();
    let mut first = RadialBaseCache::default();
    let mut second = RadialBaseCache::default();
    let dark = theme::Theme::dark();
    let light = theme::Theme::light();
    let pixels = paint(&engine, &mut first, &state, &dark, 1.0);
    let surface = first.cached.as_ref().unwrap().surface.clone();
    paint(&second_engine, &mut second, &state, &light, 1.0);
    paint(&second_engine, &mut second, &state, &dark, 2.0);
    paint(&second_engine, &mut second, &state, &light, 1.0);
    assert_ne!(
        surface.to_raw_none(),
        second.cached.as_ref().unwrap().surface.to_raw_none()
    );
    assert!(paint(&engine, &mut first, &state, &dark, 1.0) == pixels);
    assert_eq!(
        surface.to_raw_none(),
        first.cached.as_ref().unwrap().surface.to_raw_none()
    );
}

#[test]
fn palette_recents_and_active_changes_replace_rendered_base() {
    let engine = UiTextEngine::default();
    let mut state = state();
    let theme = theme::Theme::dark();
    let mut cache = RadialBaseCache::default();
    paint(&engine, &mut cache, &state, &theme, 1.0);
    state.set_quick_colors(crate::config::QuickColorPalette::from_entries(vec![
        crate::config::QuickColorPaletteEntry {
            label: "Transparent".into(),
            color: Color::new(1.0, 0.0, 0.0, 0.0),
        },
    ]));
    assert_replacement(&engine, &mut cache, &state, &theme, 1.0);
    state
        .style
        .record_recent_color(Color::new(0.123, 0.456, 0.789, 1.0));
    assert_replacement(&engine, &mut cache, &state, &theme, 1.0);
    assert!(state.set_tool_override(Some(Tool::Eraser)));
    assert_replacement(&engine, &mut cache, &state, &theme, 1.0);
    assert!(state.set_color(Color::new(0.9, 0.1, 0.2, 1.0)));
    assert_replacement(&engine, &mut cache, &state, &theme, 1.0);
}

#[test]
fn bindings_replace_rendered_base() {
    let engine = UiTextEngine::default();
    use crate::config::{Action, Shortcut};
    use crate::input::state::test_support::make_test_input_state_with_action_bindings;
    let state = state();
    let mut bindings = crate::config::KeybindingsConfig::default()
        .build_action_bindings()
        .unwrap();
    bindings.insert(Action::SelectPenTool, vec![Shortcut::parse("F9").unwrap()]);
    let mut rebound = make_test_input_state_with_action_bindings(bindings);
    rebound.open_radial_menu(250.0, 250.0);
    rebound.update_radial_menu_layout(500, 500);
    let mut cache = RadialBaseCache::default();
    let theme = theme::Theme::dark();
    let before = paint(&engine, &mut cache, &state, &theme, 1.0);
    let after = assert_replacement(&engine, &mut cache, &rebound, &theme, 1.0);
    assert!(before != after);
}

#[test]
fn transparent_swatches_keep_checkerboard_across_themes_and_densities() {
    let engine = UiTextEngine::default();
    let mut state = state();
    state.set_quick_colors(crate::config::QuickColorPalette::from_entries(
        (0..8)
            .map(|index| crate::config::QuickColorPaletteEntry {
                label: format!("Clear {index}"),
                color: Color::new(1.0, 0.0, 0.0, 0.0),
            })
            .collect(),
    ));
    let layout = state.radial_menu.layout().unwrap();
    let radius = (layout.color_inner + layout.color_outer) / 2.0;
    let mut cache = RadialBaseCache::default();
    for scale in [1.0, 2.0] {
        for theme in [theme::Theme::dark(), theme::Theme::light()] {
            let pixels = paint(&engine, &mut cache, &state, &theme, scale);
            let stride = (500.0 * scale) as usize * 4;
            let covered = (0..64)
                .filter(|index| {
                    let angle = std::f64::consts::TAU * f64::from(*index) / 64.0;
                    let x = ((layout.center_x + radius * angle.cos()) * scale).round() as usize;
                    let y = ((layout.center_y + radius * angle.sin()) * scale).round() as usize;
                    let offset = y * stride + x * 4;
                    u32::from_ne_bytes(pixels[offset..offset + 4].try_into().unwrap()) >> 24 > 0
                })
                .count();
            assert!(
                covered > 48,
                "transparent swatches must paint their checkerboard at {scale}x"
            );
        }
    }
}

#[test]
fn density_comes_from_matrix_x_scale_including_reflection() {
    let surface = ImageSurface::create(Format::ARgb32, 1, 1).unwrap();
    surface.set_device_scale(3.0, 3.0);
    let ctx = Context::new(&surface).unwrap();
    assert_eq!(
        base_scale(&ctx),
        1.0,
        "surface device density is not the matrix scale"
    );
    ctx.scale(-1.5, 2.0);
    assert_eq!(
        base_scale(&ctx),
        1.5,
        "preserve absolute xx even for nonuniform transforms"
    );
}

#[test]
fn failed_allocation_paints_directly_without_replacing_cache_and_can_retry() {
    let input = state();
    let engine = UiTextEngine::default();
    let mut cache = RadialBaseCache::default();
    paint(&engine, &mut cache, &input, &theme::Theme::dark(), 1.0);
    let retained = cache.cached.as_ref().unwrap().surface.clone();
    let theme = theme::Theme::light();
    let layout = input.radial_menu.layout().unwrap();
    let swatches = input.radial_ring_swatches();
    let mut failed = ImageSurface::create(Format::ARgb32, 500, 500).unwrap();
    let mut direct = ImageSurface::create(Format::ARgb32, 500, 500).unwrap();
    {
        let ctx = Context::new(&failed).unwrap();
        cache
            .paint_base_with_allocator(&engine, &ctx, &input, &layout, &theme, &swatches, |_| None);
        let ctx = Context::new(&direct).unwrap();
        super::super::draw_static_base(
            &engine,
            &ctx,
            &input,
            &theme,
            layout.center_x,
            layout.center_y,
            &layout,
            &swatches,
        );
    }
    let actual = failed.data().unwrap().to_vec();
    assert!(actual.iter().any(|&byte| byte != 0));
    assert!(
        actual == direct.data().unwrap().to_vec(),
        "allocation fallback must run the direct static painter"
    );
    assert_eq!(
        cache.cached.as_ref().unwrap().surface.to_raw_none(),
        retained.to_raw_none()
    );
    assert_replacement(&engine, &mut cache, &input, &theme, 1.0);
}

fn paint_full(
    engine: &UiTextEngine,
    caches: &mut crate::ui::UiRenderCaches,
    input: &InputState,
    theme: &theme::Theme,
    scale: f64,
) -> Vec<u8> {
    let mut output = ImageSurface::create(
        Format::ARgb32,
        (500.0 * scale) as i32,
        (500.0 * scale) as i32,
    )
    .unwrap();
    {
        let ctx = Context::new(&output).unwrap();
        ctx.scale(scale, scale);
        super::super::render_radial_menu_with_context(
            engine,
            &mut crate::ui::UiRenderCtx {
                cairo: &ctx,
                theme,
                caches,
            },
            input,
            500,
            500,
        );
    }
    output.data().unwrap().to_vec()
}

#[test]
fn dynamic_hover_sub_ring_and_center_labels_match_fresh_owners() {
    let engine = UiTextEngine::default();
    let mut caches = crate::ui::UiRenderCaches::default();
    let mut input = state();
    let layout = input.radial_menu.layout().unwrap();
    let parent = crate::input::state::RADIAL_COMPASS_SLICES
        .iter()
        .position(|slice| matches!(slice.kind, crate::input::state::RadialSliceKind::Parent(_)))
        .unwrap();
    let (segment, offset, _) = super::super::compass_geometry(&layout);
    let (x, y) = super::super::compass_wedge_midpoint(
        &layout,
        layout.center_x,
        layout.center_y,
        offset + parent as f64 * segment,
        segment,
    );
    let dark = theme::Theme::dark();
    let collapsed = paint_full(&engine, &mut caches, &input, &dark, 1.0);
    input.update_radial_menu_hover(x, y);
    let expanded = paint_full(&engine, &mut caches, &input, &dark, 1.0);
    assert!(expanded.iter().any(|&byte| byte != 0));
    assert!(
        expanded != collapsed,
        "expanded hover must paint its dynamic content"
    );
    assert!(matches!(
        input.radial_menu.state(),
        crate::input::state::RadialMenuState::Open {
            expanded_sub_ring: Some(_),
            ..
        }
    ));
    for scale in [1.0, 2.0, 1.0] {
        for theme in [theme::Theme::dark(), theme::Theme::light()] {
            let actual = paint_full(&engine, &mut caches, &input, &theme, scale);
            let expected = paint_full(
                &UiTextEngine::default(),
                &mut crate::ui::UiRenderCaches::default(),
                &input,
                &theme,
                scale,
            );
            assert!(
                actual == expected,
                "dynamic frame differs at byte {:?}",
                actual.iter().zip(&expected).position(|(a, b)| a != b)
            );
        }
    }
    input.update_radial_menu_hover(layout.center_x, layout.center_y);
    let theme = theme::Theme::dark();
    let centered = paint_full(&engine, &mut caches, &input, &theme, 1.0);
    assert!(
        centered != expanded,
        "center hover must replace expanded content"
    );
    assert!(
        paint_full(&engine, &mut caches, &input, &theme, 1.0)
            == paint_full(
                &UiTextEngine::default(),
                &mut crate::ui::UiRenderCaches::default(),
                &input,
                &theme,
                1.0
            )
    );
}
