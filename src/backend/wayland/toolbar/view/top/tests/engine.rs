use super::*;

#[test]
fn retained_planner_preserves_wrapped_notices_surface_size_and_input_regions() {
    let engine = UiTextEngine::default();
    let mut snapshot = snapshot();
    snapshot.settings_popover_open = true;
    snapshot.top_available_height = Some(1800.0);
    let mut notice_heights = Vec::new();
    for path in [
        "/tmp/ui.toml",
        "/tmp/設定/Καλημέρα/long-directory-name/another-long-directory-name/runtime-ui.toml",
        "/tmp/ui.toml",
    ] {
        snapshot.runtime_ui_persistence = Some(crate::ui::toolbar::RuntimeUiPersistenceSnapshot {
            path: path.into(),
            mode: crate::ui::toolbar::RuntimeUiPersistenceMode::Supported,
            detail: None,
            recovery_artifacts: Vec::new(),
        });
        let size = top_size(&engine, &snapshot);
        let tree = build_top_view(&engine, &snapshot, size.0 as f64, size.1 as f64);
        let fresh = UiTextEngine::default();
        assert_eq!(size, top_size(&fresh, &snapshot));
        assert!(
            tree.nodes() == build_top_view(&fresh, &snapshot, size.0 as f64, size.1 as f64).nodes(),
            "retained planner nodes for {path}"
        );
        let rects = top_input_rects(&engine, &snapshot, size.0 as f64, size.1 as f64).unwrap();
        assert_eq!(
            Some(rects.clone()),
            top_input_rects(&fresh, &snapshot, size.0 as f64, size.1 as f64)
        );
        assert_eq!(
            top_popover_scroll_bounds(&engine, &snapshot),
            top_popover_scroll_bounds(&fresh, &snapshot)
        );
        let text = format!("Runtime state: {path}");
        let notice = tree
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, WidgetKind::Label(label) if label.text == text))
            .expect("runtime-path notice");
        notice_heights.push(notice.rect.3);
        let (x, y, w, h) = notice.rect;
        assert!(
            rects.iter().any(|&(rx, ry, rw, rh)| x >= rx
                && y >= ry
                && x + w <= rx + rw
                && y + h <= ry + rh),
            "notice remains inside surface input panel"
        );
        // Rebind the measured notice layout to scaled paint targets, then repeat
        // pre-target sizing with the same owner.
        for density in [1.0, 2.0, 1.0] {
            let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 8, 8).unwrap();
            let ctx = cairo::Context::new(&surface).unwrap();
            ctx.scale(density, density);
            engine.layout(
                &ctx,
                crate::ui_text::UiTextStyle {
                    family: crate::ui::theme::toolbar::FONT_FAMILY_DEFAULT,
                    slant: cairo::FontSlant::Normal,
                    weight: cairo::FontWeight::Normal,
                    size: super::super::menus::MENU_META_FONT,
                },
                &text,
                Some(super::super::menus::MENU_CONTENT_W),
            );
            assert_eq!(top_size(&engine, &snapshot), size);
        }
    }
    assert!(
        notice_heights[1] > notice_heights[0],
        "long Unicode path must wrap into more height"
    );
    assert_eq!(notice_heights[0], notice_heights[2]);
}

#[test]
fn retained_planner_matches_fresh_under_width_pressure_and_scroll_bounds() {
    let engine = UiTextEngine::default();
    let mut snapshot = snapshot();
    snapshot.settings_popover_open = true;
    snapshot.customize_items_open = true;
    snapshot.customize_items_group =
        Some(crate::ui::toolbar::ToolbarItemCustomizeGroup::TopControls);
    snapshot.top_available_height = Some(360.0);
    for width in [1200.0, 420.0, 1200.0] {
        snapshot.top_viewport_max = Some(width);
        let size = top_size(&engine, &snapshot);
        let fresh = UiTextEngine::default();
        assert_eq!(size, top_size(&fresh, &snapshot));
        let (natural, viewport) = top_popover_scroll_bounds(&engine, &snapshot).unwrap();
        assert!(natural > viewport);
        assert_eq!(
            Some((natural, viewport)),
            top_popover_scroll_bounds(&fresh, &snapshot)
        );
        let actual = build_top_view(&engine, &snapshot, size.0 as f64, size.1 as f64);
        let expected = build_top_view(&fresh, &snapshot, size.0 as f64, size.1 as f64);
        assert!(actual.nodes() == expected.nodes(), "width pressure {width}");
        assert!(!actual.to_hit_regions().is_empty());
    }
}
