use super::*;

#[test]
fn configurable_core_segments_keep_fixed_order_and_split_tool_from_size() {
    let state = make_state();
    let pieces = build_cluster_pieces(&state);
    let kinds: Vec<_> = pieces.iter().filter_map(|piece| piece.kind).collect();
    assert_eq!(
        kinds,
        vec![
            StatusHudSegmentKind::Board,
            StatusHudSegmentKind::Page,
            StatusHudSegmentKind::Color,
            StatusHudSegmentKind::Tool,
            StatusHudSegmentKind::Size,
            StatusHudSegmentKind::Help,
            StatusHudSegmentKind::About,
        ]
    );

    let tool = pieces
        .iter()
        .find(|piece| piece.kind == Some(StatusHudSegmentKind::Tool))
        .expect("tool piece");
    let size = pieces
        .iter()
        .find(|piece| piece.kind == Some(StatusHudSegmentKind::Size))
        .expect("size piece");
    assert_eq!(tool.text.as_deref(), Some("Pen"));
    assert_eq!(size.text.as_deref(), Some("4px"));
}

#[test]
fn each_core_content_flag_removes_only_its_segment() {
    let cases = [
        (StatusBarItem::Board, StatusHudSegmentKind::Board),
        (StatusBarItem::Page, StatusHudSegmentKind::Page),
        (StatusBarItem::Color, StatusHudSegmentKind::Color),
        (StatusBarItem::Tool, StatusHudSegmentKind::Tool),
        (StatusBarItem::Size, StatusHudSegmentKind::Size),
        (StatusBarItem::Help, StatusHudSegmentKind::Help),
        (StatusBarItem::About, StatusHudSegmentKind::About),
    ];

    for (item, kind) in cases {
        let mut state = make_state();
        assert!(state.set_status_bar_item_visible(item, false));
        let pieces = build_cluster_pieces(&state);
        assert!(
            !pieces.iter().any(|piece| piece.kind == Some(kind)),
            "{item:?} should remove {kind:?}"
        );
        for (_, other_kind) in cases {
            if other_kind != kind {
                assert!(
                    pieces.iter().any(|piece| piece.kind == Some(other_kind)),
                    "disabling {item:?} unexpectedly removed {other_kind:?}"
                );
            }
        }
    }
}

#[test]
fn prefix_content_keeps_output_before_selection_and_honors_both_flags() {
    let mut state = make_state();
    state.ui_visibility.show_active_output_badge = true;
    assert!(state.set_active_output_label(Some("DP-3".to_string())));
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 30,
        h: 40,
        fill: false,
        color: state.style.current_color,
        thick: state.style.current_thickness,
    });
    state.set_selection(vec![shape_id]);

    assert_eq!(
        build_prefix_text(&state).as_deref(),
        Some("Output: DP-3 · 34×44px")
    );

    state.set_status_bar_item_visible(StatusBarItem::ActiveOutput, false);
    assert_eq!(build_prefix_text(&state).as_deref(), Some("34×44px"));
    state.set_status_bar_item_visible(StatusBarItem::SelectionInfo, false);
    assert_eq!(build_prefix_text(&state), None);
}

#[test]
fn context_indicator_flag_gates_transient_status_text() {
    let mut state = make_state();
    assert!(state.toggle_click_highlight());
    let label = action_display_label(Action::ToggleClickHighlight);
    assert!(
        build_cluster_pieces(&state)
            .iter()
            .any(|piece| piece.text.as_deref() == Some(label))
    );

    state.set_status_bar_item_visible(StatusBarItem::ContextIndicators, false);
    assert!(
        !build_cluster_pieces(&state)
            .iter()
            .any(|piece| piece.text.as_deref() == Some(label))
    );
}

#[test]
fn shedding_the_last_optional_piece_does_not_leave_an_empty_pill() {
    let mut state = make_state();
    for item in StatusBarItem::ALL {
        state.set_status_bar_item_visible(item, false);
    }
    state.set_status_bar_item_visible(StatusBarItem::About, true);

    assert!(
        compute_status_hud_layout(
            &state,
            StatusPosition::BottomLeft,
            &StatusBarStyle::default(),
            80,
            60,
        )
        .is_none(),
        "a width-shed optional item must not leave padding-only HUD chrome"
    );
}

/// The M0 unconditional cap: even worst-case mandatory content on small
/// screens must never widen the pill past the width budget, at any
/// position, and every hit rect stays clamped inside the pill.
#[test]
fn pill_width_never_exceeds_max_fraction_of_screen() {
    let state = make_worst_case_state();
    let style = StatusBarStyle::default();

    for (screen_width, screen_height) in [(640_u32, 480_u32), (800, 600), (1280, 720)] {
        for position in [
            StatusPosition::TopLeft,
            StatusPosition::TopRight,
            StatusPosition::BottomLeft,
            StatusPosition::BottomRight,
        ] {
            let layout =
                compute_status_hud_layout(&state, position, &style, screen_width, screen_height)
                    .expect("layout");
            let max_pill_width = screen_width as f64 * STATUS_BAR_MAX_WIDTH_FRACTION;
            assert!(
                layout.pill_width <= max_pill_width + 1e-6,
                "pill width {} exceeds cap {} on {}x{} at {:?}",
                layout.pill_width,
                max_pill_width,
                screen_width,
                screen_height,
                position
            );
            assert!(layout.pill_x >= 0.0, "pill off-screen at {:?}", position);
            assert!(
                layout.pill_x + layout.pill_width <= screen_width as f64 + 1e-6,
                "pill runs off a {}px screen at {:?}",
                screen_width,
                position
            );
            for segment in &layout.segments {
                assert!(
                    segment.x >= layout.pill_x - 1e-6,
                    "{:?} left of pill",
                    segment.kind
                );
                assert!(
                    segment.x + segment.width <= layout.pill_x + layout.pill_width + 1e-6,
                    "{:?} hit rect outside the pill",
                    segment.kind
                );
            }
        }
    }
}

/// The measurement stays within the budget for realistic content on
/// common screens (the pre-ladder fast path).
#[test]
fn measured_pill_width_stays_within_budget_for_realistic_cluster() {
    let style = StatusBarStyle::default();

    for screen_width in [1280_u32, 1366, 1920] {
        let measurement = measure(&style, LONG_PREFIX, CLUSTER_WIDTH, screen_width);
        let max_pill_width = screen_width as f64 * STATUS_BAR_MAX_WIDTH_FRACTION;
        assert!(
            !measurement.overflow,
            "cluster {} should fit {}px screen",
            CLUSTER_WIDTH, screen_width
        );
        assert!(
            measurement.pill_width <= max_pill_width + 1e-6,
            "pill width {} exceeds cap {} on {}px screen",
            measurement.pill_width,
            max_pill_width,
            screen_width
        );
    }
}

/// Degradation ladder order: a comfortable screen keeps the plan-mock
/// "{name} {i}/{N}" board label; a tight budget degrades it down to the
/// compact "Board i/N" and drops the help chip before the unconditional
/// backstop clamps the pill.
#[test]
fn width_budget_degrades_board_label_then_drops_help() {
    let mut state = make_state();
    let active = state.boards.active_index();
    state.boards.board_states_mut()[active].spec.name = LONG_BOARD_NAME.to_string();
    let style = StatusBarStyle::default();
    let index = state.boards.active_index() + 1;
    let count = state.boards.board_count().max(1);

    let first_text = |layout: &StatusHudLayout| match &layout.runs[0] {
        StatusHudRun::Text { text, .. } => text.clone(),
        StatusHudRun::Dot { .. } => panic!("expected the board text run first"),
    };

    // Comfortable budget: full 20-char truncation, name then index/count.
    let wide = compute_status_hud_layout(&state, StatusPosition::BottomLeft, &style, 3840, 2160)
        .expect("wide layout");
    assert_eq!(
        first_text(&wide),
        format!(
            "{} {}/{}",
            crate::util::truncate_with_ellipsis(LONG_BOARD_NAME, BOARD_NAME_MAX_CHARS),
            index,
            count
        )
    );
    assert!(
        wide.segments
            .iter()
            .any(|s| s.kind == StatusHudSegmentKind::Help)
    );

    // Tight budget: the board label reaches the compact form and the
    // help chip is dropped, yet the cap still holds.
    let narrow = compute_status_hud_layout(&state, StatusPosition::BottomLeft, &style, 400, 300)
        .expect("narrow layout");
    assert_eq!(first_text(&narrow), format!("Board {}/{}", index, count));
    assert!(
        !narrow
            .segments
            .iter()
            .any(|s| s.kind == StatusHudSegmentKind::Help),
        "help chip should shed under a tight budget"
    );
    assert!(narrow.pill_width <= 400.0 * STATUS_BAR_MAX_WIDTH_FRACTION + 1e-6);
}

/// The hidden-toolbar hint chip appears only while no toolbar surface is
/// visible, carries the toggle binding, and never shows while presenter
/// mode owns toolbar visibility (the toggle is a no-op there).
#[test]
fn toolbar_hint_chip_appears_only_while_toolbar_hidden() {
    let style = StatusBarStyle::default();
    let mut state = make_state();

    let has_chip = |state: &InputState| {
        compute_status_hud_layout(state, StatusPosition::BottomLeft, &style, 1920, 1080)
            .expect("layout")
            .segments
            .iter()
            .any(|s| s.kind == StatusHudSegmentKind::Toolbar)
    };

    // Toolbars visible: no hint.
    assert!(!has_chip(&state));

    // All toolbar surfaces hidden: the hint appears with the F9 binding.
    state.set_toolbar_visible(false);
    assert!(has_chip(&state));
    let layout = compute_status_hud_layout(&state, StatusPosition::BottomLeft, &style, 1920, 1080)
        .expect("layout");
    assert!(
        layout.runs.iter().any(|run| matches!(
            run,
            StatusHudRun::Text { text, .. } if text == "F9 Toolbar"
        )),
        "expected the default-binding hint text"
    );

    // Presenter mode with hide_toolbars: the hint must not tease a
    // toggle that presenter mode suppresses.
    state.presenter_mode = true;
    assert!(!has_chip(&state));
    state.presenter_mode = false;

    // `[ui] show_toolbar_hint = false` opts deliberate toolbar-less
    // setups out entirely.
    state.ui_visibility.show_toolbar_hint = false;
    assert!(!has_chip(&state));
}
