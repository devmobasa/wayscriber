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
        input.command_palette.set_query(query);
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
            draw_command_palette_action_tooltip(
                &crate::ui::theme::Theme::dark(),
                &engine,
                ctx,
                text,
                740.0,
                560.0,
                800.0,
                600.0,
            )
        });
        let expected = pixels(density, |ctx| {
            draw_command_palette_action_tooltip(
                &crate::ui::theme::Theme::dark(),
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

#[test]
fn prepared_palette_paints_without_application_state() {
    let engine = UiTextEngine::default();
    let closed = pixels(1, |ctx| {
        paint_command_palette(
            &crate::ui::theme::Theme::dark(),
            &engine,
            ctx,
            &CommandPaletteView::Closed,
            800,
            600,
        )
    });
    assert!(closed.iter().all(|byte| *byte == 0));
    let view = CommandPaletteView::List(PaletteListView {
        query: "No matching action".into(),
        rows: Vec::new(),
        geometry: (100.0, 100.0, 520.0, 180.0),
        scroll: 0,
        visible_count: 8,
        selected: 0,
        hovered: None,
        bindings: Default::default(),
        tooltip: None,
    });
    let painted = pixels(1, |ctx| {
        paint_command_palette(
            &crate::ui::theme::Theme::dark(),
            &engine,
            ctx,
            &view,
            800,
            600,
        )
    });
    assert!(painted.iter().any(|byte| *byte != 0));
    // The dimmer and the panel have different opacity, so this checks the
    // prepared geometry actually places a panel, beyond merely clearing Cairo.
    let alpha = |x: usize, y: usize| painted[(y * 800 + x) * 4 + 3];
    assert!(alpha(200, 150) > alpha(20, 20));
}

#[test]
fn palette_theme_refresh_covers_list_capture_and_tooltip() {
    let engine = UiTextEngine::default();
    let mut input = crate::input::state::test_support::make_test_input_state();
    input.toggle_command_palette();
    for query in ["", "capture", "No match 測試 🖌️"] {
        input.command_palette.set_query(query);
        let view = CommandPaletteView::prepare(&input, 800, 480);
        let paint = |theme: &crate::ui::theme::Theme| {
            pixels(1, |ctx| {
                paint_command_palette(theme, &engine, ctx, &view, 800, 480);
            })
        };
        let dark = paint(&crate::ui::theme::Theme::dark());
        let light = paint(&crate::ui::theme::Theme::light());
        assert_ne!(dark, light, "query {query}");
        assert_eq!(dark, paint(&crate::ui::theme::Theme::dark()));
        if let Some(dir) = std::env::var_os("WAYSCRIBER_PALETTE_REFERENCES") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            for (name, theme) in [
                ("dark", crate::ui::theme::Theme::dark()),
                ("light", crate::ui::theme::Theme::light()),
            ] {
                let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 800, 480).unwrap();
                let ctx = cairo::Context::new(&surface).unwrap();
                paint_command_palette(&theme, &engine, &ctx, &view, 800, 480);
                let name = format!(
                    "palette-{name}-{}.png",
                    if query.is_empty() {
                        "groups"
                    } else if query == "capture" {
                        "selected"
                    } else {
                        "empty"
                    }
                );
                surface
                    .write_to_png(&mut std::fs::File::create(dir.join(name)).unwrap())
                    .unwrap();
            }
        }
    }
    for capture in [false, true] {
        let paint = |theme: &crate::ui::theme::Theme| {
            pixels(1, |ctx| {
                if capture {
                    render_keybinding_capture(
                        theme,
                        &engine,
                        ctx,
                        &["Ctrl+C".into()],
                        crate::config::Action::CopySelection,
                        800,
                        600,
                    );
                } else {
                    draw_command_palette_action_tooltip(
                        theme,
                        &engine,
                        ctx,
                        "Edit shortcut",
                        100.0,
                        100.0,
                        800.0,
                        600.0,
                    );
                }
            })
        };
        let dark = paint(&crate::ui::theme::Theme::dark());
        let light = paint(&crate::ui::theme::Theme::light());
        assert_ne!(dark, light);
        assert_eq!(dark, paint(&crate::ui::theme::Theme::dark()));
    }
}

/// The shortcut controls' strip on one display row, in screen pixels.
fn row_action_strip(input: &InputState, display_row: usize) -> (i32, i32, i32, i32) {
    use crate::input::state::{
        COMMAND_PALETTE_ROW_ACTION_COUNT, COMMAND_PALETTE_ROW_ACTION_GAP,
        COMMAND_PALETTE_ROW_ACTION_SIZE,
    };

    let rows = input.command_palette_rows();
    let geometry = input.command_palette_geometry_for_rows(800, 600, &rows);
    let right = geometry.x + geometry.inner_x + geometry.inner_width;
    let stride = COMMAND_PALETTE_ROW_ACTION_SIZE + COMMAND_PALETTE_ROW_ACTION_GAP;
    let left = right - stride * COMMAND_PALETTE_ROW_ACTION_COUNT as f64;
    let top = geometry.y
        + geometry.items_top
        + (display_row - input.command_palette.scroll()) as f64 * COMMAND_PALETTE_ITEM_HEIGHT;
    (
        left.ceil() as i32,
        top.ceil() as i32 + 2,
        right.floor() as i32,
        (top + COMMAND_PALETTE_ITEM_HEIGHT).floor() as i32 - 4,
    )
}

fn strip_is_blank(pixels: &[u8], (left, top, right, bottom): (i32, i32, i32, i32)) -> bool {
    let at = |x: i32, y: i32| {
        let offset = (y as usize * 800 + x as usize) * 4;
        &pixels[offset..offset + 4]
    };
    let background = at(left, top);
    (top..bottom).all(|y| (left..right).all(|x| at(x, y) == background))
}

#[test]
fn row_shortcut_controls_show_only_on_the_selected_and_hovered_rows() {
    let engine = UiTextEngine::default();
    let mut input = crate::input::state::test_support::make_test_input_state();
    input.toggle_command_palette();
    input.update_pointer_position(0, 0);
    let rows = input.command_palette_rows();
    let command_rows: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.command_index().is_some())
        .map(|(display, _)| display)
        .collect();
    let (selected_row, other_row) = (command_rows[0], command_rows[2]);

    let idle = pixels(1, |ctx| {
        render_command_palette_with_engine(&engine, ctx, &input, 800, 600)
    });
    assert!(
        !strip_is_blank(&idle, row_action_strip(&input, selected_row)),
        "the selected row keeps its controls"
    );
    assert!(
        strip_is_blank(&idle, row_action_strip(&input, other_row)),
        "other rows draw no trash/edit/reset icons"
    );

    let (left, top, _, bottom) = row_action_strip(&input, other_row);
    input.update_pointer_position(left - 200, (top + bottom) / 2);
    let hovered = pixels(1, |ctx| {
        render_command_palette_with_engine(&engine, ctx, &input, 800, 600)
    });
    assert!(
        !strip_is_blank(&hovered, row_action_strip(&input, other_row)),
        "hovering a row reveals its controls"
    );
}
