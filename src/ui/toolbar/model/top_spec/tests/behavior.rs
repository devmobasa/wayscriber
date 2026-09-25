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
        "About leads the window-chrome trio, after the layout menu"
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

/// The layout button opens the preset menu instead of cycling presets: a
/// cycling button jumped under the pointer because each preset changes the
/// strip width. Its glyph still names the mode currently on screen, and it
/// reads as active while the menu is open.
#[test]
fn layout_control_toggles_the_preset_menu_and_shows_the_current_icon() {
    let control = TopToolbarControl::LayoutMode;
    for (mode, icon) in [
        (ToolbarLayoutMode::Simple, TopToolbarIcon::LayoutSimple),
        (ToolbarLayoutMode::Regular, TopToolbarIcon::LayoutRegular),
        (ToolbarLayoutMode::Advanced, TopToolbarIcon::LayoutAdvanced),
    ] {
        let mut snapshot = snapshot();
        snapshot.layout_mode = mode;
        assert_eq!(
            control.event(&snapshot),
            ToolbarEvent::ToggleLayoutMenu(true),
            "{mode:?} opens the menu"
        );
        assert_eq!(
            control.icon(&snapshot),
            Some(icon),
            "{mode:?} shows the current mode's glyph"
        );
        assert!(!control.active(&snapshot));

        snapshot.layout_menu_open = true;
        assert_eq!(
            control.event(&snapshot),
            ToolbarEvent::ToggleLayoutMenu(false),
            "a second click closes the menu"
        );
        assert!(control.active(&snapshot), "the open menu marks its button");
    }
    let snapshot = snapshot();
    assert_eq!(control.role(), TopToolbarControlRole::Chrome);
    assert_eq!(control.island(), TopToolbarIsland::Chrome);
    assert_eq!(
        control.id(),
        TopToolbarControlId::Item(ids::TOP_CHROME_LAYOUT)
    );
    assert_eq!(control.accessible_label(&snapshot), "Toolbar layout");
}

#[test]
fn required_chrome_and_tool_controls_have_a_glyph() {
    let snapshot = snapshot();
    for control in [
        TopToolbarControl::Restore,
        TopToolbarControl::DragHandle,
        TopToolbarControl::Pin,
        TopToolbarControl::Overflow,
        TopToolbarControl::About,
        TopToolbarControl::Minimize,
        TopToolbarControl::LayoutMode,
        TopToolbarControl::Undo,
        TopToolbarControl::Redo,
        TopToolbarControl::ShapePicker,
    ] {
        assert_eq!(
            Some(control.glyph(&snapshot)),
            control.icon(&snapshot),
            "{control:?} glyph matches the optional icon"
        );
    }
    let mut unpinned = snapshot.clone();
    unpinned.top_pinned = false;
    assert_eq!(
        TopToolbarControl::Pin.glyph(&unpinned),
        TopToolbarIcon::Unpin
    );
    let mut pinned = snapshot.clone();
    pinned.top_pinned = true;
    assert_eq!(TopToolbarControl::Pin.glyph(&pinned), TopToolbarIcon::Pin);
    assert_eq!(TopToolbarControl::HighlightRing.icon(&snapshot), None);
}

/// The tooltip names the current mode for all three presets.
#[test]
fn layout_tooltip_names_the_current_mode() {
    let control = TopToolbarControl::LayoutMode;
    for (mode, tooltip) in [
        (
            ToolbarLayoutMode::Simple,
            "Layout: Simple (click to choose)",
        ),
        (
            ToolbarLayoutMode::Regular,
            "Layout: Regular (click to choose)",
        ),
        (
            ToolbarLayoutMode::Advanced,
            "Layout: Advanced (click to choose)",
        ),
    ] {
        let mut snapshot = snapshot();
        snapshot.layout_mode = mode;
        assert_eq!(control.tooltip(&snapshot), tooltip);
    }
}

/// Regular and Advanced used to render the same strip. Advanced now brings
/// the everyday shapes and the presenter effects out of the Shapes picker,
/// which keeps only the polygons.
#[test]
fn advanced_layout_shows_shapes_inline_and_keeps_polygons_in_the_picker() {
    use crate::ui::toolbar::model::{visible_shape_picker_rows, visible_top_tool_buttons};

    let mut regular = snapshot();
    regular.layout_mode = ToolbarLayoutMode::Regular;
    let mut advanced = regular.clone();
    advanced.layout_mode = ToolbarLayoutMode::Advanced;

    let regular_tools: Vec<_> = visible_top_tool_buttons(regular.layout_mode, &regular).collect();
    let advanced_tools: Vec<_> =
        visible_top_tool_buttons(advanced.layout_mode, &advanced).collect();
    for tool in [Tool::Rect, Tool::Ellipse, Tool::Blur, Tool::Spotlight] {
        assert!(
            !regular_tools.contains(&tool),
            "{tool:?} stays in Regular's picker"
        );
        assert!(
            advanced_tools.contains(&tool),
            "{tool:?} is inline in Advanced"
        );
    }
    assert!(
        regular_tools
            .iter()
            .all(|tool| advanced_tools.contains(tool)),
        "Advanced keeps every Regular tool"
    );

    let picker: Vec<_> = visible_shape_picker_rows(&advanced, advanced.layout_mode)
        .into_iter()
        .flatten()
        .collect();
    assert_eq!(
        picker,
        [
            Tool::Triangle,
            Tool::Parallelogram,
            Tool::Rhombus,
            Tool::RegularPolygon,
            Tool::FreeformPolygon,
        ]
    );

    let regular_spec = TopToolbarSpec::build(&regular, &TopStripPlan::unconstrained());
    let advanced_spec = TopToolbarSpec::build(&advanced, &TopStripPlan::unconstrained());
    assert_ne!(
        regular_spec.strip(),
        advanced_spec.strip(),
        "the two presets render different strips"
    );
}

/// Like the other chrome entries, the layout menu is hideable; hiding
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
        "hiding the layout menu leaves About, pin, minimize in order"
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
    // slot); an empty slot saves the current setup.
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

fn preset(
    tool: Tool,
    color: crate::draw::Color,
    size: f64,
) -> crate::ui::toolbar::PresetSlotSnapshot {
    crate::ui::toolbar::PresetSlotSnapshot {
        name: None,
        tool,
        color,
        size,
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
    }
}

#[test]
fn preset_slots_say_what_they_hold_and_how_to_fill_them() {
    let state = make_test_input_state();
    let mut snapshot = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let red = snapshot.quick_colors.rendered_entries()[0].clone();
    snapshot.presets = vec![None; 5];
    snapshot.presets[0] = Some(preset(Tool::Pen, red.color, 4.0));
    snapshot.presets[1] = Some(crate::ui::toolbar::PresetSlotSnapshot {
        name: Some("Lecture".to_string()),
        ..preset(
            Tool::Marker,
            crate::draw::Color::new(0.2, 0.4, 0.6, 1.0),
            12.0,
        )
    });
    snapshot.presets[2] = Some(preset(Tool::Eraser, red.color, 18.0));

    // A filled slot names the tool, the color in the palette's words, and
    // the size, followed by its apply key.
    let pen = format!("Preset 1: Pen, {}, 4px", red.label);
    assert_eq!(
        TopToolbarControl::Preset(0).accessible_label(&snapshot),
        pen
    );
    assert_eq!(
        TopToolbarControl::Preset(0).tooltip(&snapshot),
        format_binding_label(&pen, snapshot.binding_hints.apply_preset(1))
    );

    // A color off the palette reads as hex; a name the user gave leads.
    assert!(
        TopToolbarControl::Preset(1)
            .tooltip(&snapshot)
            .starts_with("Preset 2: Lecture \u{2014} Marker, #336699, 12px"),
        "{}",
        TopToolbarControl::Preset(1).tooltip(&snapshot)
    );

    // Tools without a color skip it.
    assert!(
        TopToolbarControl::Preset(2)
            .tooltip(&snapshot)
            .starts_with("Preset 3: Eraser, 18px")
    );

    // An empty slot says so and names the configured save key.
    let save = snapshot
        .binding_hints
        .save_preset(4)
        .expect("preset 4 has a default save binding")
        .to_string();
    let empty = TopToolbarControl::Preset(3).tooltip(&snapshot);
    assert!(empty.starts_with("Preset 4 (empty)"), "{empty}");
    assert!(empty.contains(&save), "{empty} names {save}");
    assert_eq!(
        TopToolbarControl::Preset(3).accessible_label(&snapshot),
        "Preset 4 (empty)"
    );

    // Without a binding the click is the only way in, and the tooltip says so.
    let unbound = TopToolbarControl::Preset(3).tooltip(&self::snapshot());
    assert_eq!(
        unbound,
        "Preset 4 (empty) \u{2014} click to save the current tool"
    );
}

/// Strip controls that currently read as the active tool: tool buttons plus
/// the Shapes picker standing in for the tools it hosts.
fn active_tool_controls(snapshot: &ToolbarSnapshot) -> Vec<TopToolbarControl> {
    TopToolbarSpec::build(snapshot, &TopStripPlan::unconstrained())
        .strip()
        .iter()
        .filter_map(|node| match node {
            TopToolbarNode::Control(control) => Some(*control),
            TopToolbarNode::Divider(_) => None,
        })
        .filter(|control| {
            matches!(
                control,
                TopToolbarControl::Tool(_) | TopToolbarControl::ShapePicker
            )
        })
        .filter(|control| control.active(snapshot))
        .collect()
}

#[test]
fn a_grouped_tool_lights_one_button() {
    let mut full = snapshot();
    full.layout_mode = ToolbarLayoutMode::Regular;
    full.shape_picker_open = false;

    // Shape Pen, Line, and Arrow have buttons of their own in full layouts,
    // so the Shapes picker stays quiet for them.
    for tool in [Tool::LiveShape, Tool::Line, Tool::Arrow] {
        full.active_tool = tool;
        full.tool_override = Some(tool);
        assert_eq!(
            active_tool_controls(&full),
            [TopToolbarControl::Tool(tool)],
            "{tool:?}"
        );
    }

    // Tools that live inside the picker light the picker instead.
    for tool in [Tool::Rect, Tool::Ellipse, Tool::RegularPolygon, Tool::Blur] {
        full.active_tool = tool;
        full.tool_override = Some(tool);
        assert_eq!(
            active_tool_controls(&full),
            [TopToolbarControl::ShapePicker],
            "{tool:?}"
        );
    }

    // Simple layouts move Shape Pen, Line, and Arrow into the picker.
    let mut simple = full.clone();
    simple.layout_mode = ToolbarLayoutMode::Simple;
    for tool in [Tool::LiveShape, Tool::Line, Tool::Arrow] {
        simple.active_tool = tool;
        simple.tool_override = Some(tool);
        assert_eq!(
            active_tool_controls(&simple),
            [TopToolbarControl::ShapePicker],
            "simple {tool:?}"
        );
    }
}

#[test]
fn an_open_shapes_picker_reads_active_whatever_the_tool() {
    let mut snapshot = snapshot();
    snapshot.active_tool = Tool::Pen;
    snapshot.tool_override = Some(Tool::Pen);
    snapshot.shape_picker_open = true;

    assert!(TopToolbarControl::ShapePicker.active(&snapshot));
}
