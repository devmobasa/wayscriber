use super::*;

fn pixels(density: i32, paint: impl FnOnce(&cairo::Context)) -> Vec<u8> {
    let mut surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, 640 * density, 480 * density).unwrap();
    {
        let ctx = cairo::Context::new(&surface).unwrap();
        ctx.scale(f64::from(density), f64::from(density));
        paint(&ctx);
    }
    surface.flush();
    surface.data().unwrap().to_vec()
}

#[test]
fn retained_toast_owner_matches_standalone_across_targets() {
    let engine = UiTextEngine::default();
    let mut state = crate::input::state::test_support::make_test_input_state();
    let mut toast = Toast::info("Saved — Καλημέρα 你好");
    toast.duration_ms = 60_000;
    state.push_toast(ToastPriority::Hint, "paint", toast);
    // The preset clock is held ahead of now so the countdown stays at zero.
    state.preset_slots.set_feedback(
        1,
        PresetFeedbackKind::Save,
        Instant::now() + std::time::Duration::from_secs(60),
    );
    let bounds = ui_toast_geometry_with_engine(&engine, &state, 640, 480).unwrap();
    let preset = preset_toast_geometry_with_engine(&engine, &state, 640, 480).unwrap();
    for density in [1, 2, 1] {
        let retained = pixels(density, |ctx| {
            let geometry = render_ui_toast_with_engine(&engine, ctx, &state, 640, 480).unwrap();
            assert_eq!(geometry.0, bounds);
            render_preset_toast_with_engine(&engine, ctx, &state, 640, 480);
        });
        let standalone = pixels(density, |ctx| {
            render_ui_toast(ctx, &state, 640, 480).unwrap();
            render_preset_toast(ctx, &state, 640, 480);
        });
        assert!(
            retained == standalone,
            "toast pixels differ at density {density}"
        );
        assert!(retained.iter().any(|b| *b != 0));
        assert_eq!(
            ui_toast_geometry_with_engine(&engine, &state, 640, 480),
            Some(bounds)
        );
        assert_eq!(
            preset_toast_geometry_with_engine(&engine, &state, 640, 480),
            Some(preset)
        );
    }
    state.ui_visibility.show_preset_toasts = false;
    assert!(preset_toast_geometry_with_engine(&engine, &state, 640, 480).is_none());
}

#[test]
fn explicit_toast_action_layout_handoff_preserves_release_target() {
    use crate::input::state::ToastCommand;
    let engine = UiTextEngine::default();
    let mut state = crate::input::state::test_support::make_test_input_state();
    state.push_toast(
        ToastPriority::Hint,
        "actions",
        Toast::info("A long message with Unicode words Καλημέρα that must fit beside both buttons")
            .action("Open", Action::OpenCaptureFolder)
            .secondary_action("Help", Action::ToggleHelp),
    );
    let layout = ui_toast_layout(&engine, &state, 360, 480).unwrap();
    assert!(layout.message.ends_with('…'));
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 360, 480).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    let (bounds, [first, second]) =
        render_ui_toast_with_engine(&engine, &ctx, &state, 360, 480).unwrap();
    assert_eq!(bounds, layout.bounds);
    assert_eq!([first, second], layout.action_bounds);
    state.set_toast_geometry(Some(bounds), [first, second]);
    let center = |rect: (f64, f64, f64, f64)| {
        (
            (rect.0 + rect.2 / 2.0) as i32,
            (rect.1 + rect.3 / 2.0) as i32,
        )
    };
    let a = center(first.unwrap());
    let b = center(second.unwrap());
    let pressed = state.toast_press_at(a.0, a.1).unwrap();
    assert_eq!(state.resolve_toast_release(pressed, b.0, b.1).1, None);
    let pressed = state.toast_press_at(b.0, b.1).unwrap();
    assert_eq!(
        state.resolve_toast_release(pressed, b.0, b.1),
        (true, Some(ToastCommand::Dispatch(Action::ToggleHelp)))
    );
}
