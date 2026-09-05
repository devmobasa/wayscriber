use super::*;

#[test]
fn pill_width_stays_capped_without_prefix() {
    let style = StatusBarStyle::default();

    let measurement = measure(&style, "", CLUSTER_WIDTH, 1280);
    assert_eq!(measurement.prefix_width, 0.0);
    assert!(measurement.pill_width <= 1280.0 * STATUS_BAR_MAX_WIDTH_FRACTION + 1e-6);
}

#[test]
fn oversized_cluster_reports_overflow_for_piece_shedding() {
    let style = StatusBarStyle::default();
    let max_width = 1280.0 * STATUS_BAR_MAX_WIDTH_FRACTION - style.padding * 2.0;

    // Without a prefix the budget binds only when the cluster itself
    // exceeds it.
    assert!(measure(&style, "", max_width + 1.0, 1280).overflow);
    // With a prefix, the reserved prefix floor binds earlier.
    let floor = max_width * MIN_PREFIX_BUDGET_FRACTION;
    assert!(measure(&style, LONG_PREFIX, max_width - floor + 1.0, 1280).overflow);
    assert!(!measure(&style, LONG_PREFIX, 200.0, 1280).overflow);
}

#[test]
fn wrapped_prefix_grows_pill_height() {
    let style = StatusBarStyle::default();

    let narrow = measure(&style, LONG_PREFIX, CLUSTER_WIDTH, 1280);
    let wide = measure(&style, "FROZEN", CLUSTER_WIDTH, 3840);
    // The wrapped prefix block must be accounted for in the pill height so
    // extra lines never spill past the background.
    assert!(narrow.prefix_height >= CLUSTER_LINE_HEIGHT);
    assert!(narrow.pill_height >= narrow.prefix_height + style.padding);
    assert!(narrow.pill_height > wide.pill_height);
}

#[test]
fn pill_height_covers_min_interactive_hit_target() {
    let style = StatusBarStyle {
        font_size: 8.0,
        padding: 2.0,
        dot_radius: 2.0,
        ..StatusBarStyle::default()
    };

    let measurement =
        measure_status_bar(&UiTextEngine::default(), &style, "", 100.0, 9.0, 4.0, 1920).unwrap();
    assert!(measurement.pill_height >= MIN_INTERACTIVE_HEIGHT);
}

#[test]
fn pill_origin_stays_on_screen_for_all_corners() {
    let inset = STATUS_BAR_EDGE_INSET;
    let (screen_width, screen_height) = (1280.0, 720.0);
    // Wider than the screen: right-aligned corners would go negative
    // without clamping.
    let (pill_width, pill_height) = (1500.0, 60.0);

    for position in [
        StatusPosition::TopLeft,
        StatusPosition::TopRight,
        StatusPosition::BottomLeft,
        StatusPosition::BottomRight,
    ] {
        let (bx, by) = pill_origin(
            position,
            screen_width,
            screen_height,
            pill_width,
            pill_height,
        );
        assert!(bx >= inset, "bx {} below inset for {:?}", bx, position);
        assert!(by >= inset, "by {} below inset for {:?}", by, position);
        assert!(by <= screen_height - inset - pill_height);
    }

    // A pill that fits keeps its requested corner alignment.
    let (bx, by) = pill_origin(
        StatusPosition::BottomRight,
        screen_width,
        screen_height,
        400.0,
        60.0,
    );
    assert_eq!(bx, screen_width - inset - 400.0);
    assert_eq!(by, screen_height - inset - 60.0);
}

#[test]
fn layout_places_core_segments_inside_the_pill_with_min_hit_height() {
    let state = make_state();
    let style = StatusBarStyle::default();
    let layout = compute_status_hud_layout(&state, StatusPosition::BottomLeft, &style, 1920, 1080)
        .expect("layout");

    for kind in [
        StatusHudSegmentKind::Color,
        StatusHudSegmentKind::Tool,
        StatusHudSegmentKind::Help,
    ] {
        let segment = layout
            .segments
            .iter()
            .find(|segment| segment.kind == kind)
            .unwrap_or_else(|| panic!("segment {kind:?} missing"));
        assert!(
            segment.height >= MIN_INTERACTIVE_HEIGHT,
            "{kind:?} too short"
        );
        assert!(
            segment.width >= MIN_INTERACTIVE_WIDTH,
            "{kind:?} too narrow"
        );
        assert!(segment.x >= layout.pill_x);
        assert!(segment.x + segment.width <= layout.pill_x + layout.pill_width + 1e-6);
        assert!(
            layout.pill_contains(
                segment.x + segment.width / 2.0,
                segment.y + segment.height / 2.0
            ),
            "{kind:?} center outside pill"
        );
    }

    // Segments are laid out left-to-right without overlap.
    for pair in layout.segments.windows(2) {
        assert!(
            pair[0].x + pair[0].width <= pair[1].x + 1e-6,
            "segments {:?} and {:?} overlap",
            pair[0].kind,
            pair[1].kind
        );
    }

    let max_pill_width = 1920.0 * STATUS_BAR_MAX_WIDTH_FRACTION;
    assert!(layout.pill_width <= max_pill_width + 1e-6);
}

/// With small user font/dot sizes the color dot's natural target drops
/// below the floor; the layout widens it to `MIN_INTERACTIVE_WIDTH`
/// (centered, clamped inside the pill) while rects stay disjoint.
#[test]
fn narrow_hit_targets_widen_to_min_width() {
    let state = make_state();
    let style = StatusBarStyle {
        font_size: 9.0,
        padding: 4.0,
        dot_radius: 2.0,
        ..StatusBarStyle::default()
    };
    let layout = compute_status_hud_layout(&state, StatusPosition::BottomLeft, &style, 1920, 1080)
        .expect("layout");

    for segment in &layout.segments {
        assert!(
            segment.width >= MIN_INTERACTIVE_WIDTH - 1e-6,
            "{:?} hit rect narrower than the floor: {}",
            segment.kind,
            segment.width
        );
        assert!(segment.x >= layout.pill_x - 1e-6);
        assert!(segment.x + segment.width <= layout.pill_x + layout.pill_width + 1e-6);
    }
    // Widening cedes neighbor space instead of overlapping it.
    for pair in layout.segments.windows(2) {
        assert!(
            pair[0].x + pair[0].width <= pair[1].x + 1e-6,
            "segments {:?} and {:?} overlap after widening",
            pair[0].kind,
            pair[1].kind
        );
    }
}

#[test]
fn segment_at_maps_hits_and_misses() {
    let state = make_state();
    let style = StatusBarStyle::default();
    let layout = compute_status_hud_layout(&state, StatusPosition::BottomLeft, &style, 1920, 1080)
        .expect("layout");

    let tool = layout
        .segments
        .iter()
        .find(|segment| segment.kind == StatusHudSegmentKind::Tool)
        .expect("tool segment");
    assert_eq!(
        layout.segment_at(tool.x + tool.width / 2.0, tool.y + tool.height / 2.0),
        Some(StatusHudSegmentKind::Tool)
    );
    assert_eq!(
        layout.segment_at(layout.pill_x - 10.0, layout.pill_y - 10.0),
        None
    );
}

#[test]
fn mode_badges_stack_above_bottom_hud_and_below_top_hud() {
    let mut state = make_state();
    state.set_frozen_active(true);
    state.set_zoom_status(true, false, 2.5, (0.0, 0.0));
    // Zoom actions off: the HUD-stacked ZOOM badge is the zoom indicator
    // here (with zoom actions on the bottom-right chip owns it instead).
    state.ui_visibility.show_zoom_actions = false;
    let style = StatusBarStyle::default();

    let bottom = compute_status_hud_layout(&state, StatusPosition::BottomLeft, &style, 1920, 1080)
        .expect("bottom layout");
    assert_eq!(bottom.badges.len(), 2);
    assert_eq!(bottom.badges[0].label, "FROZEN");
    assert_eq!(bottom.badges[1].label, "ZOOM 250%");
    // Stacked upward, closest badge first, no overlap with the pill.
    assert!(bottom.badges[0].y + bottom.badges[0].height <= bottom.pill_y);
    assert!(bottom.badges[1].y + bottom.badges[1].height <= bottom.badges[0].y);
    // Union bounds cover the badges.
    let (bx, by, bw, bh) = bottom.bounds;
    assert!(by <= bottom.badges[1].y);
    assert!(bx <= bottom.pill_x && bw >= bottom.pill_width);
    assert!(by + bh >= bottom.pill_y + bottom.pill_height);

    let top = compute_status_hud_layout(&state, StatusPosition::TopRight, &style, 1920, 1080)
        .expect("top layout");
    assert!(top.badges[0].y >= top.pill_y + top.pill_height);
    assert!(top.badges[1].y >= top.badges[0].y + top.badges[0].height);
    // Right-aligned to the pill edge.
    for badge in &top.badges {
        assert!((badge.x + badge.width - (top.pill_x + top.pill_width)).abs() < 1e-6);
    }
}

/// Reconciliation (M8): with zoom actions enabled the HUD-stacked ZOOM
/// badge is suppressed (the bottom-right zoom chip is the canonical zoom
/// indicator), so the percentage never shows in two places at once. Other
/// mode badges are unaffected.
#[test]
fn zoom_badge_suppressed_when_zoom_actions_enabled() {
    let mut state = make_state();
    state.set_frozen_active(true);
    state.set_zoom_status(true, false, 2.5, (0.0, 0.0));
    assert!(
        state.ui_visibility.show_zoom_actions,
        "default enables zoom actions"
    );
    let style = StatusBarStyle::default();

    let layout = compute_status_hud_layout(&state, StatusPosition::BottomLeft, &style, 1920, 1080)
        .expect("layout");
    assert!(
        !layout
            .badges
            .iter()
            .any(|badge| badge.label.contains("ZOOM")),
        "ZOOM badge must be suppressed when the zoom chip owns the display"
    );
    // The unrelated FROZEN badge still stacks normally.
    assert!(layout.badges.iter().any(|badge| badge.label == "FROZEN"));

    // Hiding the chip through ToggleZoomChip while zoom actions
    // stay on must hand the display back to the HUD badge — otherwise a
    // zoomed session with a visible status bar has NO zoom indicator.
    state.ui_visibility.show_zoom_chip = false;
    let chip_hidden =
        compute_status_hud_layout(&state, StatusPosition::BottomLeft, &style, 1920, 1080)
            .expect("layout");
    assert!(
        chip_hidden
            .badges
            .iter()
            .any(|badge| badge.label == "ZOOM 250%"),
        "HUD ZOOM badge must return when the chip is master-hidden"
    );
    state.ui_visibility.show_zoom_chip = true;

    // With zoom actions off the badge returns.
    state.ui_visibility.show_zoom_actions = false;
    let restored =
        compute_status_hud_layout(&state, StatusPosition::BottomLeft, &style, 1920, 1080)
            .expect("layout");
    assert!(
        restored
            .badges
            .iter()
            .any(|badge| badge.label == "ZOOM 250%")
    );
}

#[test]
fn frozen_badge_keeps_literal_red_tint() {
    let mut state = make_state();
    state.set_frozen_active(true);
    let style = StatusBarStyle::default();
    let layout = compute_status_hud_layout(&state, StatusPosition::BottomLeft, &style, 1920, 1080)
        .expect("layout");
    let frozen = layout
        .badges
        .iter()
        .find(|badge| badge.label == "FROZEN")
        .expect("frozen badge");
    assert_eq!(frozen.tint, FROZEN_BADGE_TINT);
    assert_eq!(FROZEN_BADGE_TINT, [0.82, 0.22, 0.2, 0.9]);
}

#[test]
fn status_hud_geometry_reads_the_cached_layout_for_matching_screens() {
    let mut state = make_state();
    assert_eq!(status_hud_geometry(&state, 1920, 1080), None);

    state.update_status_hud_layout(
        StatusPosition::BottomLeft,
        &StatusBarStyle::default(),
        1920,
        1080,
    );
    let bounds = status_hud_geometry(&state, 1920, 1080).expect("bounds");
    assert!(bounds.2 > 0.0 && bounds.3 > 0.0);
    // A stale layout for another screen size is not reported.
    assert_eq!(status_hud_geometry(&state, 1280, 720), None);

    state.clear_status_hud_layout();
    assert_eq!(status_hud_geometry(&state, 1920, 1080), None);
}
