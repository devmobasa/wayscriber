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
fn retained_color_popup_engine_shares_tooltip_damage_and_paint_across_targets() {
    let engine = UiTextEngine::default();
    let mut input = crate::input::state::test_support::make_test_input_state();
    input.open_color_picker_popup();
    input.update_color_picker_popup_layout(800, 600);
    let layout = input.color_picker_popup_layout().unwrap();
    let anchor = (
        layout.eyedropper_btn_x + layout.action_btn_size / 2.0,
        layout.eyedropper_btn_y + layout.action_btn_size / 2.0,
    );
    for hovered in [false, true, false] {
        input.color_picker_popup_set_hover(hovered.then_some(anchor));
        for density in [1, 2, 1] {
            let bounds =
                color_picker_popup_visual_geometry_with_engine(&engine, &input, 800, 600).unwrap();
            assert_eq!(
                Some(bounds),
                color_picker_popup_visual_geometry(&input, 800, 600)
            );
            assert!(bounds.2 >= layout.width && bounds.3 >= layout.height);
            if hovered {
                let (text, x, y) = layout.action_tooltip_anchor_at(anchor.0, anchor.1).unwrap();
                let tip = action_tooltip_geometry(&engine, text, x, y, 800.0, 600.0).unwrap();
                assert!(bounds.0 <= tip.0 && bounds.1 <= tip.1);
                assert!(bounds.0 + bounds.2 >= tip.0 + tip.2 + TOOLTIP_SHADOW_OFFSET);
                assert!(bounds.1 + bounds.3 >= tip.1 + tip.3 + TOOLTIP_SHADOW_OFFSET);
            }
            let actual = pixels(density, |ctx| {
                render_color_picker_popup_with_engine(&engine, ctx, &input, 800, 600)
            });
            let expected = pixels(density, |ctx| {
                render_color_picker_popup(ctx, &input, 800, 600)
            });
            assert!(
                actual == expected,
                "color popup hovered {hovered}, density {density}"
            );
            assert_eq!(
                Some(bounds),
                color_picker_popup_visual_geometry_with_engine(&engine, &input, 800, 600)
            );
        }
    }
}
