use super::*;

#[test]
fn regular_spec_owns_control_order_ids_and_events() {
    let snapshot = snapshot();
    let spec = TopToolbarSpec::build(&snapshot, &TopStripPlan::unconstrained());
    let ids = strip_control_ids(&spec);

    let expected_order = [
        "top.chrome.drag",
        "top.tool.select",
        "top.tool.pen",
        "top.tool.marker",
        "top.tool.step-marker",
        "top.tool.eraser",
        "top.tool.line",
        "top.tool.arrow",
        "top.utility.shape-picker",
        "top.utility.text",
        "top.utility.sticky-note",
        "top.utility.highlight",
        // Colors left the strip for the pill (M7-C1); the presets island
        // now occupies the seam between the tools and history islands.
        "top.preset.0",
        "top.preset.4",
        "top.utility.undo",
        "top.utility.redo",
        "top.chrome.overflow",
    ];
    let mut position = 0;
    for expected in expected_order {
        let next = ids[position..]
            .iter()
            .position(|id| id == expected)
            .map(|offset| position + offset)
            .unwrap_or_else(|| panic!("{expected} missing from {ids:?}"));
        position = next + 1;
    }

    // No color swatches or current-color chip remain in the strip.
    assert!(
        !ids.iter().any(|id| id.starts_with("top.quick-color.")),
        "quick colors left the strip for the pill: {ids:?}"
    );
    assert!(
        !ids.contains(&"top.group.quick-colors".to_string()),
        "the current-color chip left the strip for the pill: {ids:?}"
    );

    assert!(
        !ids.contains(&ids::TOP_UTILITY_CLEAR_CANVAS.as_str().to_string()),
        "Clear lives in the overflow menu, not the strip: {ids:?}"
    );
    assert_eq!(
        spec.overflow(),
        [
            TopToolbarControl::ClearCanvas,
            TopToolbarControl::CanvasMenu,
            TopToolbarControl::SessionMenu,
            TopToolbarControl::SettingsMenu,
        ],
        "an unconstrained plan still anchors Clear plus the Canvas/Session/Settings entries"
    );

    assert_eq!(
        chrome_ids(&spec),
        [
            "top.chrome.layout",
            "top.chrome.about",
            "top.chrome.pin",
            "top.chrome.close",
        ],
        "the chrome island reads layout cycle, About, pin, minimize"
    );
    let pen = spec
        .strip()
        .iter()
        .find_map(|node| match node {
            TopToolbarNode::Control(control @ TopToolbarControl::Tool(Tool::Pen)) => Some(*control),
            _ => None,
        })
        .expect("pen control");
    assert_eq!(pen.event(&snapshot), ToolbarEvent::SelectTool(Tool::Pen));
    assert_eq!(pen.id(), TopToolbarControlId::Item(ids::TOP_TOOL_PEN));

    let divider_ids: Vec<_> = spec
        .strip()
        .iter()
        .filter_map(|node| match node {
            TopToolbarNode::Divider(divider) => Some(divider.id()),
            TopToolbarNode::Control(_) => None,
        })
        .collect();
    assert_eq!(
        divider_ids,
        ["top.divider.tools", "top.divider.annotations"],
        "thin dividers exist only inside the tools island"
    );
}

#[test]
fn simple_and_compact_specs_preserve_semantics_without_geometry() {
    let mut simple = snapshot();
    simple.layout_mode = ToolbarLayoutMode::Simple;
    let simple_spec = TopToolbarSpec::build(&simple, &TopStripPlan::unconstrained());
    let simple_ids = strip_control_ids(&simple_spec);
    assert!(!simple_ids.contains(&ids::TOP_TOOL_LINE.as_str().to_string()));
    assert!(!simple_ids.contains(&ids::TOP_TOOL_ARROW.as_str().to_string()));
    assert!(!simple_ids.contains(&ids::TOP_UTILITY_HIGHLIGHT.as_str().to_string()));
    assert!(!simple_ids.contains(&ids::TOP_UTILITY_CLEAR_CANVAS.as_str().to_string()));
    assert!(simple_ids.contains(&ids::TOP_UTILITY_SHAPE_PICKER.as_str().to_string()));

    let regular = snapshot();
    let mut compact_plan = TopStripPlan::unconstrained();
    compact_plan.compact = true;
    let compact_ids = strip_control_ids(&TopToolbarSpec::build(&regular, &compact_plan));
    // Compact keeps every tool/utility/history control but drops the
    // non-essential presets island (M7-C2); otherwise the membership is
    // unchanged from the unconstrained plan.
    assert!(!compact_ids.iter().any(|id| id.starts_with("top.preset.")));
    let full_without_presets: Vec<_> = strip_control_ids(&TopToolbarSpec::build(
        &regular,
        &TopStripPlan::unconstrained(),
    ))
    .into_iter()
    .filter(|id| !id.starts_with("top.preset."))
    .collect();
    assert_eq!(compact_ids, full_without_presets);
}

#[test]
fn minimized_spec_contains_only_the_non_hideable_restore_control() {
    let mut snapshot = snapshot();
    snapshot.top_minimized = true;
    snapshot
        .resolved_toolbar_items
        .hidden
        .insert(ids::TOP_CHROME_CLOSE);

    let spec = TopToolbarSpec::build(&snapshot, &TopStripPlan::unconstrained());
    assert_eq!(strip_control_ids(&spec), ["top.chrome.restore"]);
    assert!(spec.chrome().is_empty());
    assert!(spec.overflow().is_empty());
    assert_eq!(
        match spec.strip() {
            [TopToolbarNode::Control(control)] => control.event(&snapshot),
            other => panic!("unexpected minimized spec: {other:?}"),
        },
        ToolbarEvent::SetTopMinimized(false)
    );
}

#[test]
fn micro_spec_contains_only_the_non_hideable_micro_chip_control() {
    let mut snapshot = snapshot();
    snapshot.top_display_mode = crate::config::TopDisplayMode::Micro;
    snapshot
        .resolved_toolbar_items
        .hidden
        .insert(ids::TOP_CHROME_CLOSE);

    let spec = TopToolbarSpec::build(&snapshot, &TopStripPlan::unconstrained());
    assert_eq!(strip_control_ids(&spec), ["top.chrome.micro"]);
    assert!(spec.chrome().is_empty());
    assert!(spec.overflow().is_empty());
    assert!(spec.contextual().is_empty());
    let chip = match spec.strip() {
        [TopToolbarNode::Control(control)] => *control,
        other => panic!("unexpected micro spec: {other:?}"),
    };
    assert_eq!(
        chip.event(&snapshot),
        ToolbarEvent::SetTopDisplayMode(crate::config::TopDisplayMode::Full)
    );
    assert_eq!(chip.role(), TopToolbarControlRole::Restore);
    assert_eq!(
        chip.icon(&snapshot),
        Some(TopToolbarIcon::Tool(semantic_icon_for_tool(
            snapshot.active_tool
        )))
    );

    // Minimized wins when both states are somehow set.
    snapshot.top_minimized = true;
    let minimized = TopToolbarSpec::build(&snapshot, &TopStripPlan::unconstrained());
    assert_eq!(strip_control_ids(&minimized), ["top.chrome.restore"]);
}

#[test]
fn micro_ring_width_maps_thickness_into_the_clamped_ring_range() {
    assert_eq!(micro_ring_width(0.0), 1.5);
    assert_eq!(micro_ring_width(1.0), 1.5);
    assert_eq!(micro_ring_width(20.0), 5.0);
    assert_eq!(micro_ring_width(50.0), 5.0);
    let mid = micro_ring_width(10.5);
    assert!(mid > 1.5 && mid < 5.0);
}

#[test]
fn narrow_spec_moves_dropped_controls_to_one_ordered_overflow() {
    let snapshot = snapshot();
    let mut plan = TopStripPlan::unconstrained();
    plan.swatch_count = 0;
    plan.dropped_tools = vec![Tool::Line, Tool::Arrow];
    plan.dropped_utilities = vec![
        TopUtilityButton::Text,
        TopUtilityButton::StickyNote,
        TopUtilityButton::Highlight,
    ];
    plan.compact = true;

    let spec = TopToolbarSpec::build(&snapshot, &plan);
    let strip_ids = strip_control_ids(&spec);
    for dropped in [
        ids::TOP_TOOL_LINE,
        ids::TOP_TOOL_ARROW,
        ids::TOP_UTILITY_TEXT,
        ids::TOP_UTILITY_STICKY_NOTE,
        ids::TOP_UTILITY_HIGHLIGHT,
    ] {
        assert!(!strip_ids.contains(&dropped.as_str().to_string()));
    }
    let overflow_ids: Vec<_> = spec
        .overflow()
        .iter()
        .map(|control| control.id().render_id().into_owned())
        .collect();
    assert_eq!(
        overflow_ids,
        [
            "top.utility.clear-canvas",
            "top.tool.line",
            "top.tool.arrow",
            "top.utility.text",
            "top.utility.sticky-note",
            "top.utility.highlight",
            "top.menu.canvas",
            "top.menu.session",
            "top.menu.settings",
        ],
        "Clear leads the overflow menu; dropped items follow in order; \
         the Canvas/Session/Settings entries close it"
    );
    assert!(
        strip_ids.contains(&ids::TOP_CHROME_OVERFLOW.as_str().to_string()),
        "the overflow toggle anchors in the history island: {strip_ids:?}"
    );
    assert_eq!(
        chrome_ids(&spec),
        [
            "top.chrome.layout",
            "top.chrome.about",
            "top.chrome.pin",
            "top.chrome.close",
        ],
        "the chrome island reads layout cycle, About, pin, minimize"
    );
    assert_eq!(
        spec.overflow()[0].event(&snapshot),
        ToolbarEvent::ClearCanvas { instant: false }
    );
    assert_eq!(
        spec.overflow()[0].role(),
        TopToolbarControlRole::Destructive
    );
    assert_eq!(
        spec.overflow()[1].event(&snapshot),
        ToolbarEvent::SelectTool(Tool::Line)
    );
}

#[test]
fn overflow_hosts_canvas_session_and_settings_entries_in_every_layout() {
    // Decision (M4-B3, extended in M8): the three popover entries are
    // unconditional — they show under both `side_layout` values and are
    // not hideable items, because under `pill` they are the only surface
    // hosting Canvas/Session/Settings.
    for layout_mode in [ToolbarLayoutMode::Regular, ToolbarLayoutMode::Simple] {
        let mut snapshot = snapshot();
        snapshot.layout_mode = layout_mode;
        let spec = TopToolbarSpec::build(&snapshot, &TopStripPlan::unconstrained());
        let tail: Vec<_> = spec
            .overflow()
            .iter()
            .rev()
            .take(3)
            .rev()
            .copied()
            .collect();
        assert_eq!(
            tail,
            [
                TopToolbarControl::CanvasMenu,
                TopToolbarControl::SessionMenu,
                TopToolbarControl::SettingsMenu,
            ],
            "{layout_mode:?}: the menu entries close the overflow, Canvas first"
        );
        assert!(
            !spec.overflow().is_empty(),
            "the overflow toggle always has content"
        );
        assert!(
            spec.strip()
                .contains(&TopToolbarNode::Control(TopToolbarControl::Overflow))
        );
    }

    let mut snapshot = snapshot();
    let canvas = TopToolbarControl::CanvasMenu;
    let session = TopToolbarControl::SessionMenu;
    let settings = TopToolbarControl::SettingsMenu;

    assert_eq!(canvas.id().render_id(), "top.menu.canvas");
    assert_eq!(session.id().render_id(), "top.menu.session");
    assert_eq!(settings.id().render_id(), "top.menu.settings");
    assert_eq!(canvas.label(&snapshot), "Canvas...");
    assert_eq!(session.label(&snapshot), "Session...");
    assert_eq!(settings.label(&snapshot), "Settings...");
    assert_eq!(canvas.icon(&snapshot), Some(TopToolbarIcon::Canvas));
    assert_eq!(session.icon(&snapshot), Some(TopToolbarIcon::Session));
    assert_eq!(settings.icon(&snapshot), Some(TopToolbarIcon::Settings));
    // The three entries carry distinct icons.
    assert_ne!(canvas.icon(&snapshot), session.icon(&snapshot));
    assert_ne!(canvas.icon(&snapshot), settings.icon(&snapshot));
    assert_ne!(session.icon(&snapshot), settings.icon(&snapshot));
    assert_eq!(canvas.role(), TopToolbarControlRole::Toggle);
    assert_eq!(session.role(), TopToolbarControlRole::Toggle);
    assert_eq!(settings.role(), TopToolbarControlRole::Toggle);
    assert_eq!(canvas.island(), TopToolbarIsland::History);
    assert_eq!(session.island(), TopToolbarIsland::History);
    assert_eq!(settings.island(), TopToolbarIsland::History);
    assert!(canvas.enabled(&snapshot) && session.enabled(&snapshot) && settings.enabled(&snapshot));

    // Entries toggle their popover open state and report it as active.
    assert_eq!(
        canvas.event(&snapshot),
        ToolbarEvent::ToggleCanvasPopover(true)
    );
    assert_eq!(
        session.event(&snapshot),
        ToolbarEvent::ToggleSessionPopover(true)
    );
    assert_eq!(
        settings.event(&snapshot),
        ToolbarEvent::ToggleSettingsPopover(true)
    );
    assert!(!canvas.active(&snapshot) && !session.active(&snapshot) && !settings.active(&snapshot));
    snapshot.canvas_popover_open = true;
    assert!(canvas.active(&snapshot));
    assert_eq!(
        canvas.event(&snapshot),
        ToolbarEvent::ToggleCanvasPopover(false)
    );
    snapshot.canvas_popover_open = false;
    snapshot.session_popover_open = true;
    assert!(session.active(&snapshot));
    assert_eq!(
        session.event(&snapshot),
        ToolbarEvent::ToggleSessionPopover(false)
    );
    snapshot.session_popover_open = false;
    snapshot.settings_popover_open = true;
    assert!(settings.active(&snapshot));
    assert_eq!(
        settings.event(&snapshot),
        ToolbarEvent::ToggleSettingsPopover(false)
    );

    // Minimized and micro strips carry no overflow (and so no entries).
    let mut minimized = self::snapshot();
    minimized.top_minimized = true;
    assert!(
        TopToolbarSpec::build(&minimized, &TopStripPlan::unconstrained())
            .overflow()
            .is_empty()
    );
    let mut micro = self::snapshot();
    micro.top_display_mode = crate::config::TopDisplayMode::Micro;
    assert!(
        TopToolbarSpec::build(&micro, &TopStripPlan::unconstrained())
            .overflow()
            .is_empty()
    );
}
