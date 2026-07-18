use super::*;
use crate::backend::wayland::toolbar::layout::{ToolbarLayoutSpec, top_size};
use crate::backend::wayland::toolbar::view::{ShortcutBadgePlacement, WidgetKind, WidgetTree};
use crate::config::{Action, action_label, toolbar_item_ids as ids};
use crate::input::state::test_support::make_test_input_state;
use crate::ui::toolbar::{ToolbarBindingHints, ToolbarEvent, ToolbarSnapshot};

fn snapshot() -> ToolbarSnapshot {
    let state = make_test_input_state();
    ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default())
}

fn build(snapshot: &ToolbarSnapshot) -> WidgetTree {
    let (w, h) = top_size(snapshot);
    build_top_view(snapshot, w as f64, h as f64)
}

fn node_id_list(tree: &WidgetTree) -> Vec<&str> {
    tree.nodes().iter().map(|node| node.id.as_str()).collect()
}

#[test]
fn strip_reads_as_divider_chunked_groups() {
    let snapshot = snapshot();
    let tree = build(&snapshot);
    let ids = node_id_list(&tree);

    let expected_order = [
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
        "top.group.quick-colors",
        "top.utility.undo",
        "top.utility.redo",
        "top.utility.clear-canvas",
    ];
    let mut last = 0;
    for id in expected_order {
        let pos = ids.iter().position(|x| *x == id).unwrap_or_else(|| {
            panic!("{id} missing from strip: {ids:?}");
        });
        assert!(pos > last || last == 0, "{id} out of order");
        last = pos;
    }

    // Rect/Ellipse/Blur left the inline row for the picker grid.
    assert!(!ids.contains(&"top.tool.rect"));
    assert!(!ids.contains(&"top.tool.blur"));

    // The Ico/Txt segmented toggle left the strip for the Settings pane.
    assert!(!ids.contains(&"top.utility.icon-mode"));

    // Divider-chunked groups exist.
    assert!(ids.contains(&"top.divider.tools"));
    assert!(ids.contains(&"top.divider.annotations"));
    assert!(ids.contains(&"top.divider.colors"));
    assert!(ids.contains(&"top.divider.history"));
}

#[test]
fn quick_colors_render_in_slot_order_with_a_chip() {
    let snapshot = snapshot();
    let tree = build(&snapshot);

    let expected: Vec<_> = snapshot
        .quick_colors
        .rendered_entries()
        .iter()
        .take(TOP_MAX_QUICK_COLORS)
        .map(|entry| entry.color)
        .collect();
    assert!(!expected.is_empty());
    for (index, color) in expected.iter().enumerate() {
        let node = tree
            .node_by_id(&format!("top.quick-color.{index}").into())
            .expect("swatch node");
        match node.kind {
            WidgetKind::Swatch { color: c, .. } => {
                assert_eq!(c, (color.r, color.g, color.b, color.a), "slot {index}");
            }
            ref other => panic!("swatch kind, got {other:?}"),
        }
        assert!(matches!(
            node.interact.as_ref().unwrap().event,
            ToolbarEvent::SetQuickColor { color: c, action }
                if c == *color
                    && action == crate::config::QuickColorPalette::action_for_index(index)
        ));
    }

    let chip = tree
        .node_by_id(&"top.group.quick-colors".into())
        .expect("current color chip");
    assert!(matches!(
        chip.interact.as_ref().unwrap().event,
        ToolbarEvent::OpenColorPickerPopup
    ));
}

#[test]
fn shortcut_badges_follow_the_snapshot_bindings() {
    let state = make_test_input_state();
    let snapshot = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let tree = build(&snapshot);

    let pen = tree.node_by_id(&"top.tool.pen".into()).expect("pen");
    assert_eq!(
        pen.shortcut_badge
            .as_ref()
            .map(|badge| badge.label.as_str()),
        Some("F")
    );
    assert_eq!(
        pen.shortcut_badge.as_ref().map(|badge| badge.placement),
        Some(ShortcutBadgePlacement::Corner)
    );

    let red = tree
        .node_by_id(&"top.quick-color.0".into())
        .expect("red swatch");
    assert_eq!(
        red.shortcut_badge
            .as_ref()
            .map(|badge| badge.label.as_str()),
        Some("R")
    );
    assert_eq!(
        red.shortcut_badge.as_ref().map(|badge| badge.placement),
        Some(ShortcutBadgePlacement::Above)
    );
    assert!(
        red.interact
            .as_ref()
            .unwrap()
            .tooltip
            .as_deref()
            .is_some_and(|text| text.contains("(R)"))
    );
}

#[test]
fn history_buttons_disable_without_history() {
    let snapshot = snapshot();
    let tree = build(&snapshot);

    let undo = tree.node_by_id(&"top.utility.undo".into()).expect("undo");
    assert!(undo.interact.is_none(), "empty history is not clickable");
    match undo.kind {
        WidgetKind::IconButton { style, .. } => assert!(style.disabled),
        ref other => panic!("icon button, got {other:?}"),
    }
}

#[test]
fn clear_sits_isolated_after_history() {
    let snapshot = snapshot();
    let tree = build(&snapshot);
    let redo = tree.node_by_id(&"top.utility.redo".into()).expect("redo");
    let clear = tree
        .node_by_id(&"top.utility.clear-canvas".into())
        .expect("clear");
    let gap = ToolbarLayoutSpec::TOP_GAP;
    assert!(
        clear.rect.0 >= redo.rect.0 + redo.rect.2 + gap * 2.0 - 1e-9,
        "double gap isolates Clear"
    );
    assert!(matches!(
        clear.kind,
        WidgetKind::IconButton { style, .. } if style.destructive
    ));
}

#[test]
fn strip_fits_its_declared_width() {
    for use_icons in [true, false] {
        let mut snapshot = snapshot();
        snapshot.use_icons = use_icons;
        let tree = build(&snapshot);
        let (w, _) = tree.size();
        for node in tree.nodes() {
            if node.interact.is_some() {
                assert!(
                    node.rect.0 + node.rect.2 <= w + 0.5,
                    "{:?} exceeds width {w}",
                    node.id
                );
            }
        }
    }
}

#[test]
fn shape_picker_grid_hosts_the_relocated_shapes() {
    let mut state = make_test_input_state();
    state.toolbar_shapes_expanded = true;
    let snapshot =
        ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default());
    assert!(snapshot.shape_picker_open);

    let tree = build(&snapshot);
    let picker_ids: Vec<&str> = tree
        .nodes()
        .iter()
        .map(|node| node.id.as_str())
        .filter(|id| id.starts_with("top.picker."))
        .collect();
    assert!(picker_ids.contains(&"top.picker.top.tool.rect"));
    assert!(picker_ids.contains(&"top.picker.top.tool.blur"));
    assert!(picker_ids.contains(&"top.picker.top.tool.regular-polygon"));
}

#[test]
fn shape_picker_shows_fill_while_line_is_active() {
    let mut state = make_test_input_state();
    state.toolbar_shapes_expanded = true;
    let mut snapshot =
        ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default());
    snapshot.active_tool = crate::input::Tool::Line;
    snapshot.tool_override = None;

    let tree = build(&snapshot);

    assert!(
        tree.node_by_id(&ids::TOP_UTILITY_FILL.as_str().into())
            .is_some(),
        "Fill configures the next shape and must not depend on the active tool"
    );

    snapshot.resolved_toolbar_items = crate::config::ToolbarItemsConfig {
        hidden: vec![ids::TOP_UTILITY_FILL.as_str().to_string()],
        shown: Vec::new(),
        order: crate::config::ToolbarItemOrderConfig::default(),
    }
    .resolved();
    assert!(
        build(&snapshot)
            .node_by_id(&ids::TOP_UTILITY_FILL.as_str().into())
            .is_none(),
        "an explicitly hidden Fill item stays hidden"
    );
}

#[test]
fn input_rects_cover_bar_and_open_popovers_only() {
    let mut state = make_test_input_state();
    state.toolbar_shapes_expanded = false;
    let snapshot =
        ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default());
    let (w, h) = top_size(&snapshot);
    assert!(
        top_input_rects(&snapshot, w as f64, h as f64).is_none(),
        "no popover: whole surface takes input"
    );

    state.toolbar_shapes_expanded = true;
    let snapshot =
        ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default());
    let (w, h) = top_size(&snapshot);
    let rects = top_input_rects(&snapshot, w as f64, h as f64).expect("partial input region");
    assert_eq!(rects.len(), 2, "bar band + shapes panel: {rects:?}");
    assert_eq!(rects[0].0, 0.0);
    assert_eq!(rects[0].1, 0.0);
    assert!(rects[0].3 < h as f64, "bar band ends above the popover");
    let tree = build_top_view(&snapshot, w as f64, h as f64);
    let panel = tree
        .node_by_id(&"top.shapes.panel".into())
        .expect("panel node");
    assert!(rects[1].1 <= panel.rect.1 && rects[1].3 >= panel.rect.3);
}

#[test]
fn panel_background_stops_at_bar_band_when_popover_is_open() {
    let mut state = make_test_input_state();
    state.toolbar_shapes_expanded = true;
    let snapshot =
        ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default());
    let (w, h) = top_size(&snapshot);
    let tree = build_top_view(&snapshot, w as f64, h as f64);
    let panel = tree
        .node_by_id(&"top.panel".into())
        .expect("top panel node");
    let input_rects = top_input_rects(&snapshot, w as f64, h as f64).expect("partial input region");

    assert_eq!(panel.rect, input_rects[0]);
    assert!(
        panel.rect.3 < h as f64,
        "popover area must stay transparent"
    );
}

#[test]
fn minimized_strip_is_a_single_restore_tab() {
    let mut snapshot = snapshot();
    snapshot.top_minimized = true;

    let (w, h) = top_size(&snapshot);
    assert_eq!((w, h), (64, 24));

    let tree = build_top_view(&snapshot, w as f64, h as f64);
    let interactive: Vec<_> = tree
        .nodes()
        .iter()
        .filter(|node| node.interact.is_some())
        .collect();
    assert_eq!(interactive.len(), 1, "one restore button only");
    assert_eq!(interactive[0].id.as_str(), "top.chrome.restore");
    assert!(matches!(
        interactive[0].interact.as_ref().unwrap().event,
        ToolbarEvent::SetTopMinimized(false)
    ));
}

#[test]
fn compact_shape_picker_preserves_its_full_semantic_icon_size() {
    let snapshot = snapshot();
    let mut plan = TopStripPlan::unconstrained();
    plan.compact = true;
    let tree = super::build::build_top_view_planned(&snapshot, &plan, 800.0, 100.0);
    let picker = tree
        .node_by_id(&ids::TOP_UTILITY_SHAPE_PICKER.as_str().into())
        .expect("shape picker");

    assert!(matches!(
        picker.kind,
        WidgetKind::IconButton { icon_size, .. }
            if (icon_size - ToolbarLayoutSpec::TOP_ICON_SIZE).abs() < f64::EPSILON
    ));
}

#[test]
fn overflow_utility_tooltips_remain_bare_action_labels() {
    let state = make_test_input_state();
    let mut snapshot = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    snapshot.top_overflow_open = true;
    let mut plan = TopStripPlan::unconstrained();
    plan.swatch_count = 0;
    plan.show_overflow = true;
    plan.dropped_utilities = vec![model::TopUtilityButton::Text];

    let tree = super::build::build_top_view_planned(&snapshot, &plan, 800.0, 160.0);
    let text = tree
        .node_by_id(&"top.overflow.top.utility.text".into())
        .expect("overflow text control");
    assert_eq!(
        text.interact
            .as_ref()
            .and_then(|interaction| interaction.tooltip.as_deref()),
        Some(action_label(Action::EnterTextMode))
    );
}

#[test]
fn hidden_items_produce_no_nodes() {
    let mut snapshot = snapshot();
    snapshot.resolved_toolbar_items = crate::config::ToolbarItemsConfig {
        hidden: vec![
            ids::TOP_TOOL_PEN.as_str().to_string(),
            ids::TOP_GROUP_QUICK_COLORS.as_str().to_string(),
            ids::TOP_UTILITY_UNDO.as_str().to_string(),
        ],
        shown: Vec::new(),
        order: crate::config::ToolbarItemOrderConfig::default(),
    }
    .resolved();

    let tree = build(&snapshot);
    assert!(tree.node_by_id(&"top.tool.pen".into()).is_none());
    assert!(tree.node_by_id(&"top.group.quick-colors".into()).is_none());
    assert!(tree.node_by_id(&"top.quick-color.0".into()).is_none());
    assert!(tree.node_by_id(&"top.utility.undo".into()).is_none());
    assert!(tree.node_by_id(&"top.tool.marker".into()).is_some());
}
