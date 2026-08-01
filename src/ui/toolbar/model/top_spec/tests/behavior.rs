use super::*;

#[test]
fn island_assignment_is_total_and_ordered() {
    let regular = snapshot();
    let mut narrow_plan = TopStripPlan::unconstrained();
    narrow_plan.swatch_count = 0;
    narrow_plan.dropped_tools = vec![Tool::Line];
    narrow_plan.dropped_utilities = vec![TopUtilityButton::Text];

    for plan in [TopStripPlan::unconstrained(), narrow_plan] {
        let spec = TopToolbarSpec::build(&regular, &plan);
        let islands: Vec<_> = spec.strip().iter().map(TopToolbarNode::island).collect();

        // Total: every strip node maps to exactly one non-chrome island.
        assert!(!islands.is_empty());
        assert!(
            islands
                .iter()
                .all(|island| *island != TopToolbarIsland::Chrome),
            "chrome controls never appear in the strip: {islands:?}"
        );
        // Ordered: the tools island fully precedes the history island.
        assert!(
            islands.windows(2).all(|pair| pair[0] <= pair[1]),
            "islands must be contiguous and ordered: {islands:?}"
        );
        assert!(islands.contains(&TopToolbarIsland::Tools));
        assert!(
            islands.contains(&TopToolbarIsland::Presets),
            "the presets island sits between tools and history: {islands:?}"
        );
        assert!(islands.contains(&TopToolbarIsland::History));

        // Chrome lane is exactly the chrome island.
        for control in spec.chrome() {
            assert_eq!(control.island(), TopToolbarIsland::Chrome);
        }
    }
}

#[test]
fn contextual_ring_is_owned_by_the_highlight_control_spec() {
    let mut snapshot = snapshot();
    snapshot.highlight_tool_active = true;
    snapshot.highlight_tool_ring_enabled = false;
    let plan = TopStripPlan::unconstrained();

    let spec = TopToolbarSpec::build(&snapshot, &plan);
    assert_eq!(spec.contextual(), [TopToolbarControl::HighlightRing]);
    assert_eq!(
        spec.contextual()[0].event(&snapshot),
        ToolbarEvent::ToggleHighlightToolRing(true)
    );

    let mut dropped = plan.clone();
    dropped.dropped_utilities = vec![TopUtilityButton::Highlight];
    assert!(
        TopToolbarSpec::build(&snapshot, &dropped)
            .contextual()
            .is_empty()
    );

    snapshot
        .resolved_toolbar_items
        .hidden
        .insert(ids::TOP_UTILITY_HIGHLIGHT_RING);
    assert!(
        TopToolbarSpec::build(&snapshot, &plan)
            .contextual()
            .is_empty()
    );
}

#[test]
fn allocation_free_queries_match_the_materialized_spec() {
    let regular = snapshot();
    let mut highlighted = regular.clone();
    highlighted.highlight_tool_active = true;
    let mut minimized = highlighted.clone();
    minimized.top_minimized = true;
    let mut narrow_plan = TopStripPlan::unconstrained();
    narrow_plan.dropped_tools = vec![Tool::Line, Tool::Arrow];
    narrow_plan.dropped_utilities = vec![TopUtilityButton::Text];

    for (snapshot, plan) in [
        (&regular, TopStripPlan::unconstrained()),
        (&highlighted, TopStripPlan::unconstrained()),
        (&highlighted, narrow_plan),
        (&minimized, TopStripPlan::unconstrained()),
    ] {
        let spec = TopToolbarSpec::build(snapshot, &plan);
        assert_eq!(
            TopToolbarSpec::shape_picker_visible(snapshot),
            spec.strip()
                .contains(&TopToolbarNode::Control(TopToolbarControl::ShapePicker))
        );
        assert_eq!(
            TopToolbarSpec::contextual_highlight_ring_visible(snapshot, &plan),
            !spec.contextual().is_empty()
        );
        assert_eq!(
            TopToolbarSpec::chrome_control_count(snapshot, &plan),
            spec.chrome().len()
        );
        assert_eq!(
            TopToolbarSpec::overflow_control_count(snapshot, &plan),
            spec.overflow().len()
        );
    }
}

/// About sits in the chrome island, opens the dialog rather than changing
/// the toolbar, and can be hidden like any other chrome entry.
#[test]
fn about_is_a_hideable_chrome_entry_that_opens_the_dialog() {
    let snapshot = snapshot();
    let spec = TopToolbarSpec::build(&snapshot, &TopStripPlan::unconstrained());

    assert_eq!(
        spec.chrome().get(1).copied(),
        Some(TopToolbarControl::About),
        "About leads the window-chrome trio, after the layout cycle"
    );
    assert_eq!(TopToolbarControl::About.island(), TopToolbarIsland::Chrome);
    assert_eq!(
        TopToolbarControl::About.event(&snapshot),
        ToolbarEvent::OpenAbout
    );
    assert_eq!(
        TopToolbarControl::About.action(&snapshot),
        Some(Action::OpenAbout)
    );
    // It opens a window, so it never reads as an active toggle.
    assert!(!TopToolbarControl::About.active(&snapshot));
    assert_eq!(
        TopToolbarControl::About.role(),
        TopToolbarControlRole::Chrome
    );
    // Chrome controls never appear in the strip or the overflow menu.
    assert!(!strip_control_ids(&spec).contains(&ids::TOP_CHROME_ABOUT.as_str().to_string()));
    assert!(!spec.overflow().contains(&TopToolbarControl::About));

    let mut hidden = snapshot.clone();
    let mut items = ToolbarItemsConfig::default();
    items.set_hidden(ids::TOP_CHROME_ABOUT, true);
    hidden.resolved_toolbar_items = items.resolved();
    let spec = TopToolbarSpec::build(&hidden, &TopStripPlan::unconstrained());

    assert_eq!(
        chrome_ids(&spec),
        ["top.chrome.layout", "top.chrome.pin", "top.chrome.close"],
        "hiding the item leaves the rest of the chrome island intact"
    );
}

/// The layout cycle advances Simple → Regular → Advanced → Simple while
/// its glyph names the mode currently on screen, so the icon is the
/// state and the click is the transition.
#[test]
fn layout_cycle_control_maps_each_mode_to_its_next_event_and_current_icon() {
    let control = TopToolbarControl::LayoutMode;
    for (mode, next, icon) in [
        (
            ToolbarLayoutMode::Simple,
            ToolbarLayoutMode::Regular,
            TopToolbarIcon::LayoutSimple,
        ),
        (
            ToolbarLayoutMode::Regular,
            ToolbarLayoutMode::Advanced,
            TopToolbarIcon::LayoutRegular,
        ),
        (
            ToolbarLayoutMode::Advanced,
            ToolbarLayoutMode::Simple,
            TopToolbarIcon::LayoutAdvanced,
        ),
    ] {
        let mut snapshot = snapshot();
        snapshot.layout_mode = mode;
        assert_eq!(
            control.event(&snapshot),
            ToolbarEvent::SetToolbarLayoutMode(next),
            "{mode:?} advances to {next:?}"
        );
        assert_eq!(
            control.icon(&snapshot),
            Some(icon),
            "{mode:?} shows the current mode's glyph"
        );
        // A cycle, not a toggle: it never reads as active.
        assert!(!control.active(&snapshot));
    }
    let snapshot = snapshot();
    assert_eq!(control.role(), TopToolbarControlRole::Chrome);
    assert_eq!(control.island(), TopToolbarIsland::Chrome);
    assert_eq!(
        control.id(),
        TopToolbarControlId::Item(ids::TOP_CHROME_LAYOUT)
    );
    assert_eq!(control.accessible_label(&snapshot), "Cycle toolbar layout");
}

/// The tooltip names the current mode and where the click lands, for all
/// three presets.
#[test]
fn layout_cycle_tooltip_names_current_and_next_mode() {
    let control = TopToolbarControl::LayoutMode;
    for (mode, tooltip) in [
        (
            ToolbarLayoutMode::Simple,
            "Layout: Simple (click for Regular)",
        ),
        (
            ToolbarLayoutMode::Regular,
            "Layout: Regular (click for Advanced)",
        ),
        (
            ToolbarLayoutMode::Advanced,
            "Layout: Advanced (click for Simple)",
        ),
    ] {
        let mut snapshot = snapshot();
        snapshot.layout_mode = mode;
        assert_eq!(control.tooltip(&snapshot), tooltip);
    }
}

/// Like the other chrome entries, the layout cycle is hideable; hiding
/// it leaves the window-chrome trio in reading order.
#[test]
fn hiding_the_layout_cycle_leaves_the_chrome_trio_in_order() {
    let mut hidden = snapshot();
    let mut items = ToolbarItemsConfig::default();
    items.set_hidden(ids::TOP_CHROME_LAYOUT, true);
    hidden.resolved_toolbar_items = items.resolved();

    let spec = TopToolbarSpec::build(&hidden, &TopStripPlan::unconstrained());
    assert_eq!(
        chrome_ids(&spec),
        ["top.chrome.about", "top.chrome.pin", "top.chrome.close"],
        "hiding the layout cycle leaves About, pin, minimize in order"
    );
}

#[test]
fn customized_visibility_and_order_flow_through_the_spec() {
    let mut snapshot = snapshot();
    let mut items = ToolbarItemsConfig::default();
    items.set_hidden(ids::TOP_UTILITY_STICKY_NOTE, true);
    assert!(items.move_item_to_index(ToolbarItemOrderGroup::TopTools, ids::TOP_TOOL_ERASER, 0,));
    snapshot.resolved_toolbar_items = items.resolved();

    let spec = TopToolbarSpec::build(&snapshot, &TopStripPlan::unconstrained());
    let ids = strip_control_ids(&spec);
    let first_tool = ids
        .iter()
        .find(|id| id.starts_with("top.tool."))
        .map(String::as_str);
    assert_eq!(first_tool, Some("top.tool.eraser"));
    assert!(!ids.contains(&"top.utility.sticky-note".to_string()));
}

fn has_preset(spec: &TopToolbarSpec) -> bool {
    spec.strip()
        .iter()
        .any(|node| matches!(node, TopToolbarNode::Control(TopToolbarControl::Preset(_))))
}

#[test]
fn presets_island_hosts_the_saved_slots() {
    use crate::draw::Color;

    let mut snapshot = snapshot();
    snapshot.presets = vec![None; 5];
    snapshot.presets[0] = Some(crate::ui::toolbar::PresetSlotSnapshot {
        name: Some("Red pen".to_string()),
        tool: Tool::Pen,
        color: Color::new(1.0, 0.0, 0.0, 1.0),
        size: 4.0,
        eraser_kind: None,
        eraser_mode: None,
        marker_opacity: None,
        fill_enabled: None,
        font_size: None,
        text_background_enabled: None,
        arrow_length: None,
        arrow_angle: None,
        arrow_head_at_end: None,
        show_status_bar: None,
    });
    snapshot.active_preset_slot = Some(1);

    let spec = TopToolbarSpec::build(&snapshot, &TopStripPlan::unconstrained());
    let preset_ids: Vec<_> = spec
        .strip()
        .iter()
        .filter_map(|node| match node {
            TopToolbarNode::Control(control @ TopToolbarControl::Preset(_)) => {
                Some(control.id().render_id().into_owned())
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        preset_ids,
        [
            "top.preset.0",
            "top.preset.1",
            "top.preset.2",
            "top.preset.3",
            "top.preset.4",
        ]
    );

    // Every preset control belongs to the presets island, which sits
    // ahead of the history island in the strip ordering.
    for node in spec.strip() {
        if let TopToolbarNode::Control(control @ TopToolbarControl::Preset(_)) = node {
            assert_eq!(control.island(), TopToolbarIsland::Presets);
        }
    }
    assert!(TopToolbarIsland::Tools < TopToolbarIsland::Presets);
    assert!(TopToolbarIsland::Presets < TopToolbarIsland::History);

    let filled = TopToolbarControl::Preset(0);
    let empty = TopToolbarControl::Preset(1);
    // The filled slot applies preset 1 and reads active (the applied
    // slot); the empty slot reuses the side-palette save convention.
    assert_eq!(filled.event(&snapshot), ToolbarEvent::ApplyPreset(1));
    assert!(filled.active(&snapshot));
    assert_eq!(
        filled.icon(&snapshot),
        Some(TopToolbarIcon::Tool(semantic_icon_for_tool(Tool::Pen)))
    );
    assert!(filled.tooltip(&snapshot).contains("Red pen"));
    assert_eq!(empty.event(&snapshot), ToolbarEvent::SavePreset(2));
    assert!(!empty.active(&snapshot));
    assert_eq!(empty.icon(&snapshot), None);
    assert_eq!(empty.label(&snapshot), "2");
    assert_eq!(empty.shortcut_badge(&snapshot), None);
    assert_eq!(empty.role(), TopToolbarControlRole::Button);

    // Gating: the display toggle, the compact plan, and the width-drop
    // flag each remove the whole island.
    let mut hidden = snapshot.clone();
    hidden.show_presets = false;
    assert!(!has_preset(&TopToolbarSpec::build(
        &hidden,
        &TopStripPlan::unconstrained()
    )));

    let mut compact = TopStripPlan::unconstrained();
    compact.compact = true;
    assert!(!has_preset(&TopToolbarSpec::build(&snapshot, &compact)));

    let mut dropped = TopStripPlan::unconstrained();
    dropped.drop_presets = true;
    assert!(!has_preset(&TopToolbarSpec::build(&snapshot, &dropped)));
}
