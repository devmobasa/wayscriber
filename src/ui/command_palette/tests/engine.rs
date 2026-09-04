use super::*;

fn pixels(density: i32, paint: impl FnOnce(&cairo::Context)) -> Vec<u8> {
    let mut surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, 800 * density, 600 * density).unwrap();
    surface.set_device_scale(density as f64, density as f64);
    let ctx = cairo::Context::new(&surface).unwrap();
    paint(&ctx);
    drop(ctx);
    surface.data().unwrap().to_vec()
}

#[test]
fn retained_palette_engine_matches_public_paint_and_geometry_across_targets() {
    let engine = UiTextEngine::default();
    let mut input = crate::input::state::test_support::make_test_input_state();
    input.toggle_command_palette();
    for query in ["", "capture", "No match 測試 🖌️"] {
        input.command_palette.query = query.into();
        for density in [1, 2, 1] {
            let bounds =
                command_palette_visual_geometry_with_engine(&engine, &input, 800, 600).unwrap();
            assert_eq!(
                Some(bounds),
                command_palette_visual_geometry(&input, 800, 600)
            );
            assert!(bounds.2 > 0.0 && bounds.2 < 800.0);
            let actual = pixels(density, |ctx| {
                render_command_palette_with_engine(&engine, ctx, &input, 800, 600)
            });
            let expected = pixels(density, |ctx| render_command_palette(ctx, &input, 800, 600));
            assert!(
                actual == expected,
                "palette query {query:?}, density {density}"
            );
            assert_eq!(
                Some(bounds),
                command_palette_visual_geometry_with_engine(&engine, &input, 800, 600)
            );
        }
    }
}

#[test]
fn retained_palette_tooltip_geometry_contains_pixels_and_unicode_highlight_is_visible() {
    use crate::ui::text_highlight::{HighlightStyle, draw_highlight_with_engine, find_match_range};
    let engine = UiTextEngine::default();
    let text = "Edit 測試 🖌️ binding";
    for density in [1, 2, 1] {
        let bounds =
            command_palette_action_tooltip_geometry(&engine, text, 740.0, 560.0, 800.0, 600.0)
                .unwrap();
        let actual = pixels(density, |ctx| {
            draw_command_palette_action_tooltip(&engine, ctx, text, 740.0, 560.0, 800.0, 600.0)
        });
        let expected = pixels(density, |ctx| {
            draw_command_palette_action_tooltip(
                &UiTextEngine::default(),
                ctx,
                text,
                740.0,
                560.0,
                800.0,
                600.0,
            )
        });
        assert!(actual == expected, "tooltip density {density}");
        let mut painted = 0;
        for (index, pixel) in actual.as_chunks::<4>().0.iter().enumerate() {
            if pixel.iter().any(|byte| *byte != 0) {
                painted += 1;
                let x = (index % (800 * density) as usize) as f64 / density as f64;
                let y = (index / (800 * density) as usize) as f64 / density as f64;
                assert!(x >= bounds.0 - 2.0 && x <= bounds.0 + bounds.2 + 2.0);
                assert!(y >= bounds.1 - 2.0 && y <= bounds.1 + bounds.3 + 2.0);
            }
        }
        assert!(painted > 0);
        let style = HighlightStyle {
            font_family: "Sans",
            font_size: 14.0,
            font_weight: cairo::FontWeight::Normal,
            color: [1.0, 0.0, 0.0, 1.0],
        };
        let range = find_match_range(text, "測試").unwrap();
        let highlight = pixels(density, |ctx| {
            draw_highlight_with_engine(&engine, ctx, 30.0, 40.0, text, range, &style)
        });
        let fresh = pixels(density, |ctx| {
            draw_highlight_with_engine(
                &UiTextEngine::default(),
                ctx,
                30.0,
                40.0,
                text,
                range,
                &style,
            )
        });
        assert!(highlight == fresh, "highlight density {density}");
        assert!(highlight.iter().any(|byte| *byte != 0));
        let invalid = pixels(density, |ctx| {
            draw_highlight_with_engine(
                &engine,
                ctx,
                30.0,
                40.0,
                text,
                (range.0 + 1, range.1),
                &style,
            )
        });
        assert!(invalid.iter().all(|byte| *byte == 0));
    }
}
