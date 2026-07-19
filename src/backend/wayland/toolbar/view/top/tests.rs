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
        "top.chrome.overflow",
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

    // Clear moved into the overflow menu; the strip carries no Clear tile.
    assert!(!ids.contains(&"top.utility.clear-canvas"));

    // Divider-chunked groups exist inside the tools island; the old
    // history divider became the tools|history island gap.
    assert!(ids.contains(&"top.divider.tools"));
    assert!(ids.contains(&"top.divider.annotations"));
    assert!(ids.contains(&"top.divider.colors"));
    assert!(!ids.contains(&"top.divider.history"));

    // The four pill islands back the strip: the band's three plus the
    // contextual style pill underneath.
    assert!(ids.contains(&"top.island.tools"));
    assert!(ids.contains(&"top.island.history"));
    assert!(ids.contains(&"top.island.chrome"));
    assert!(ids.contains(&"top.island.style"));
}

#[test]
fn islands_are_detached_pills_in_reading_order() {
    let snapshot = snapshot();
    let tree = build(&snapshot);
    let island = |key: &str| {
        tree.node_by_id(&format!("top.island.{key}").into())
            .unwrap_or_else(|| panic!("{key} island"))
    };
    let tools = island("tools");
    let history = island("history");
    let chrome = island("chrome");

    let style = island("style");

    // Pills read left to right with clear gaps between their edges.
    let gap = ToolbarLayoutSpec::TOP_ISLAND_GAP;
    assert!((history.rect.0 - (tools.rect.0 + tools.rect.2) - gap).abs() < 1e-9);
    assert!(chrome.rect.0 >= history.rect.0 + history.rect.2 + gap - 1e-9);
    // Pills hug the surface edges and paint the panel treatment.
    assert_eq!(tools.rect.0, 0.0);
    let (w, _) = tree.size();
    assert!((chrome.rect.0 + chrome.rect.2 - w).abs() < 1e-9);
    for node in [&tools, &history, &chrome, &style] {
        assert!(matches!(node.kind, WidgetKind::Panel));
        assert!(node.interact.is_none());
    }

    // The style pill is a detached fourth pill left-aligned with island A
    // under the band.
    assert_eq!(style.rect.0, 0.0);
    assert!(
        (style.rect.1 - (tools.rect.1 + tools.rect.3 + ToolbarLayoutSpec::TOP_STYLE_PILL_GAP))
            .abs()
            < 1e-9,
        "style pill sits one gap under the band"
    );
    assert_eq!(style.rect.3, ToolbarLayoutSpec::TOP_STYLE_PILL_H);
    // The pill contents stay inside their pill.
    for node in tree.nodes() {
        if node.id.as_str().starts_with("top.style.") {
            assert!(node.rect.1 >= style.rect.1 && node.rect.0 >= style.rect.0);
            assert!(node.rect.0 + node.rect.2 <= style.rect.0 + style.rect.2 + 1e-9);
            assert!(node.rect.1 + node.rect.3 <= style.rect.1 + style.rect.3 + 1e-9);
        }
    }

    // The island contents stay inside their pill.
    let undo = tree.node_by_id(&"top.utility.undo".into()).expect("undo");
    assert!(
        undo.rect.0 >= history.rect.0
            && undo.rect.0 + undo.rect.2 <= history.rect.0 + history.rect.2
    );
    let overflow = tree
        .node_by_id(&"top.chrome.overflow".into())
        .expect("overflow toggle");
    assert!(overflow.rect.0 + overflow.rect.2 <= history.rect.0 + history.rect.2 + 1e-9);
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
    // Icon buttons caption their shortcut under the icon.
    assert_eq!(
        pen.shortcut_badge.as_ref().map(|badge| badge.placement),
        Some(ShortcutBadgePlacement::Below)
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

    // Text-label buttons keep the boxed corner micro-badge.
    let mut text_snapshot = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    text_snapshot.use_icons = false;
    let text_tree = build(&text_snapshot);
    let text_pen = text_tree.node_by_id(&"top.tool.pen".into()).expect("pen");
    assert_eq!(
        text_pen
            .shortcut_badge
            .as_ref()
            .map(|badge| badge.placement),
        Some(ShortcutBadgePlacement::Corner)
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
fn clear_lives_in_overflow_first_with_destructive_styling() {
    let mut snapshot = snapshot();
    snapshot.top_overflow_open = true;
    let tree = build(&snapshot);

    assert!(
        tree.node_by_id(&"top.utility.clear-canvas".into())
            .is_none(),
        "Clear left the strip for the overflow menu"
    );
    let first_overflow_item = tree
        .nodes()
        .iter()
        .find(|node| node.id.as_str().starts_with("top.overflow.") && node.interact.is_some())
        .expect("open overflow menu content");
    assert_eq!(
        first_overflow_item.id.as_str(),
        "top.overflow.top.utility.clear-canvas",
        "Clear leads the overflow menu"
    );
    assert!(matches!(
        first_overflow_item.kind,
        WidgetKind::IconButton { style, .. } if style.destructive
    ));
    assert!(matches!(
        first_overflow_item.interact.as_ref().unwrap().event,
        ToolbarEvent::ClearCanvas { instant: false }
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
fn input_rects_cover_islands_and_open_popovers_only() {
    let mut state = make_test_input_state();
    state.toolbar_shapes_expanded = false;
    let snapshot =
        ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default());
    let (w, h) = top_size(&snapshot);
    let rects = top_input_rects(&snapshot, w as f64, h as f64)
        .expect("no popover: the islands still restrict input");
    assert_eq!(
        rects.len(),
        4,
        "band pills plus the style pill only: {rects:?}"
    );
    // The gaps between the band islands click through to the canvas even in
    // the common no-popover state: consecutive island rects do not touch.
    assert!(rects[0].0 + rects[0].2 < rects[1].0);
    assert!(rects[1].0 + rects[1].2 < rects[2].0);
    // The style pill is the fourth rect, detached below the band.
    assert!(rects[3].1 > rects[0].1 + rects[0].3);
    let tree = build_top_view(&snapshot, w as f64, h as f64);
    let islands: Vec<_> = tree
        .nodes()
        .iter()
        .filter(|node| node.id.as_str().starts_with("top.island."))
        .collect();
    assert_eq!(islands.len(), 4);
    for (island, input_rect) in islands.iter().zip(&rects) {
        assert_eq!(island.rect, *input_rect, "{}", island.id);
    }

    state.toolbar_shapes_expanded = true;
    let snapshot =
        ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default());
    let (w, h) = top_size(&snapshot);
    let rects = top_input_rects(&snapshot, w as f64, h as f64).expect("partial input region");
    assert_eq!(rects.len(), 5, "four islands + shapes panel: {rects:?}");
    assert_eq!(rects[0].0, 0.0);
    assert_eq!(rects[0].1, 0.0);
    for island in &rects[..4] {
        assert!(
            island.1 + island.3 < h as f64,
            "islands end above the popover"
        );
    }
    // The gaps between the band islands stay click-through: consecutive
    // island rects do not touch.
    assert!(rects[0].0 + rects[0].2 < rects[1].0);
    assert!(rects[1].0 + rects[1].2 < rects[2].0);
    let tree = build_top_view(&snapshot, w as f64, h as f64);
    let panel = tree
        .node_by_id(&"top.shapes.panel".into())
        .expect("panel node");
    assert!(rects[4].1 <= panel.rect.1 && rects[4].3 >= panel.rect.3);
    // The popover opens below the style pill, never over it.
    let style = tree
        .node_by_id(&"top.island.style".into())
        .expect("style pill");
    assert!(panel.rect.1 >= style.rect.1 + style.rect.3);
}

#[test]
fn island_backgrounds_stop_at_bar_band_when_popover_is_open() {
    let mut state = make_test_input_state();
    state.toolbar_shapes_expanded = true;
    let snapshot =
        ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default());
    let (w, h) = top_size(&snapshot);
    let tree = build_top_view(&snapshot, w as f64, h as f64);
    let input_rects = top_input_rects(&snapshot, w as f64, h as f64).expect("partial input region");

    let islands: Vec<_> = tree
        .nodes()
        .iter()
        .filter(|node| node.id.as_str().starts_with("top.island."))
        .collect();
    assert_eq!(islands.len(), 4, "tools/history/chrome/style pills");
    for (island, input_rect) in islands.iter().zip(&input_rects[..4]) {
        assert_eq!(island.rect, *input_rect, "{}", island.id);
        assert!(
            island.rect.1 + island.rect.3 < h as f64,
            "popover area must stay transparent"
        );
    }
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
fn micro_strip_is_a_single_round_chip() {
    let mut snapshot = snapshot();
    snapshot.top_display_mode = crate::config::TopDisplayMode::Micro;

    let (w, h) = top_size(&snapshot);
    assert_eq!(
        (w, h),
        ToolbarLayoutSpec::TOP_MICRO_SIZE,
        "micro mode is one 44px chip"
    );

    let tree = build_top_view(&snapshot, w as f64, h as f64);
    let interactive: Vec<_> = tree
        .nodes()
        .iter()
        .filter(|node| node.interact.is_some())
        .collect();
    assert_eq!(interactive.len(), 1, "one chip control only");
    assert_eq!(interactive[0].id.as_str(), "top.chrome.micro");
    assert!(matches!(
        interactive[0].interact.as_ref().unwrap().event,
        ToolbarEvent::SetTopDisplayMode(crate::config::TopDisplayMode::Full)
    ));
    match &interactive[0].kind {
        WidgetKind::MicroChip {
            glyph,
            ring_color,
            ring_width,
        } => {
            let expected_icon = crate::ui::toolbar::model::TopToolbarIcon::Tool(
                crate::ui::toolbar::model::semantic_icon_for_tool(snapshot.active_tool),
            );
            assert!(std::ptr::fn_addr_eq(
                glyph.0,
                crate::toolbar_icons::top_toolbar_icon_painter(expected_icon)
            ));
            assert_eq!(
                *ring_color,
                (
                    snapshot.color.r,
                    snapshot.color.g,
                    snapshot.color.b,
                    snapshot.color.a
                ),
                "ring strokes in the current color"
            );
            assert_eq!(
                *ring_width,
                crate::ui::toolbar::model::micro_ring_width(snapshot.thickness)
            );
        }
        other => panic!("micro chip kind, got {other:?}"),
    }

    // No popovers, ring rows, or partial input regions in micro mode: the
    // whole 44px surface takes input.
    assert!(top_input_rects(&snapshot, w as f64, h as f64).is_none());
    assert_eq!(top_extra_height(&snapshot), 0.0);

    // Minimized wins if both states are somehow set.
    snapshot.top_minimized = true;
    assert_eq!(top_size(&snapshot), ToolbarLayoutSpec::TOP_MINIMIZED_SIZE);
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

/// Snapshot with one tool active and the settings overrides pinned off, so
/// each case exercises exactly one pure style-pill morph state (the
/// overrides themselves are covered by the spec's unit tests).
fn snapshot_for_tool(tool: crate::input::Tool) -> ToolbarSnapshot {
    let mut snapshot = snapshot();
    snapshot.active_tool = tool;
    snapshot.tool_override = None;
    snapshot.thickness_targets_eraser = tool == crate::input::Tool::Eraser;
    snapshot.thickness_targets_marker = tool == crate::input::Tool::Marker;
    snapshot.show_text_controls = false;
    snapshot.show_marker_opacity_section = false;
    snapshot
}

fn style_ids(tree: &WidgetTree) -> Vec<String> {
    tree.nodes()
        .iter()
        .filter(|node| node.id.as_str().starts_with("top.style."))
        .map(|node| node.id.as_str().to_string())
        .collect()
}

#[test]
fn style_pill_sliders_reuse_the_shared_drag_hit_kinds() {
    use crate::backend::wayland::toolbar::events::HitKind;

    let snapshot = snapshot_for_tool(crate::input::Tool::Pen);
    let tree = build(&snapshot);

    let chip = tree
        .node_by_id(&"top.style.color-chip".into())
        .expect("pill color chip");
    assert!(matches!(
        chip.interact.as_ref().unwrap().event,
        ToolbarEvent::OpenColorPickerPopup
    ));
    match chip.kind {
        WidgetKind::Swatch { color, selected } => {
            assert_eq!(
                color,
                (
                    snapshot.color.r,
                    snapshot.color.g,
                    snapshot.color.b,
                    snapshot.color.a
                )
            );
            assert!(selected, "the chip always shows the live color as active");
        }
        ref other => panic!("chip swatch kind, got {other:?}"),
    }

    let swatch = tree
        .node_by_id(&"top.style.swatch.0".into())
        .expect("pill quick swatch");
    let entry = &snapshot.quick_colors.rendered_entries()[0];
    assert!(matches!(
        swatch.interact.as_ref().unwrap().event,
        ToolbarEvent::SetQuickColor { color, action }
            if color == entry.color
                && action == crate::config::QuickColorPalette::action_for_index(0)
    ));

    let slider = tree
        .node_by_id(&"top.style.thickness".into())
        .expect("pill thickness slider");
    let spec = model::ToolbarSliderSpec::THICKNESS;
    match slider.kind {
        WidgetKind::Slider { t } => {
            assert!((t - spec.t_from_value(snapshot.thickness)).abs() < 1e-9);
        }
        ref other => panic!("slider kind, got {other:?}"),
    }
    let interaction = slider.interact.as_ref().unwrap();
    assert_eq!(
        interaction.kind,
        HitKind::DragSetThickness {
            min: spec.min,
            max: spec.max,
        },
        "the pill reuses the existing thickness drag kind"
    );
    assert!(matches!(
        interaction.event,
        ToolbarEvent::SetThickness(value) if value == snapshot.thickness
    ));

    // The live numeral is a distinct pango-labelled button that opens the
    // overlay precise-entry popup.
    let numeral = tree
        .node_by_id(&"top.style.thickness-value".into())
        .expect("pill thickness numeral");
    match &numeral.kind {
        WidgetKind::TextButton { label, .. } => {
            assert_eq!(label.text, format!("{:.0}px", snapshot.thickness));
        }
        other => panic!("numeral kind, got {other:?}"),
    }
    // The numeral opens the overlay precise-entry popup.
    assert_eq!(
        numeral.interact.as_ref().map(|interact| &interact.event),
        Some(&ToolbarEvent::OpenPrecisionEntry(
            crate::ui::toolbar::PrecisionEntryTarget::Thickness
        ))
    );
}

#[test]
fn style_pill_morphs_per_tool() {
    use crate::backend::wayland::toolbar::events::HitKind;
    use crate::input::{EraserMode, Tool};

    // Marker: thickness (targeting the marker size) plus the opacity slider
    // with its inline readout decoration.
    let marker = snapshot_for_tool(Tool::Marker);
    let tree = build(&marker);
    let opacity = tree
        .node_by_id(&"top.style.opacity".into())
        .expect("marker opacity slider");
    let spec = model::ToolbarSliderSpec::MARKER_OPACITY;
    assert_eq!(
        opacity.interact.as_ref().unwrap().kind,
        HitKind::DragSetMarkerOpacity {
            min: spec.min,
            max: spec.max,
        }
    );
    let readout = tree
        .node_by_id(&"top.style.opacity.readout".into())
        .expect("opacity readout");
    match &readout.kind {
        WidgetKind::Label(label) => {
            assert_eq!(label.text, format!("{:.0}%", marker.marker_opacity * 100.0));
        }
        other => panic!("readout kind, got {other:?}"),
    }
    assert!(readout.interact.is_none());

    // Eraser: colorless; the old checkbox became a Brush/Stroke segment
    // emitting SetEraserMode, painted by the activated SegmentedControl.
    let eraser = snapshot_for_tool(Tool::Eraser);
    let tree = build(&eraser);
    let ids = style_ids(&tree);
    assert!(!ids.contains(&"top.style.color-chip".to_string()));
    assert!(!ids.contains(&"top.style.swatch.0".to_string()));
    let segment = tree
        .node_by_id(&"top.style.eraser-mode".into())
        .expect("eraser mode segment");
    match &segment.kind {
        WidgetKind::SegmentedControl {
            left,
            right,
            active_right,
        } => {
            assert_eq!(left.text, "Brush");
            assert_eq!(right.text, "Stroke");
            assert_eq!(*active_right, eraser.eraser_mode == EraserMode::Stroke);
        }
        other => panic!("segment kind, got {other:?}"),
    }
    assert!(segment.interact.is_none(), "halves carry the interactions");
    for (id, mode) in [
        ("top.style.eraser-mode.brush", EraserMode::Brush),
        ("top.style.eraser-mode.stroke", EraserMode::Stroke),
    ] {
        let half = tree.node_by_id(&id.into()).expect("segment half");
        assert!(matches!(half.kind, WidgetKind::HitArea));
        assert!(matches!(
            half.interact.as_ref().unwrap().event,
            ToolbarEvent::SetEraserMode(value) if value == mode
        ));
    }
    // The eraser-size numeral shares the thickness precise-entry target
    // (the snapshot routes the eraser size through `thickness`).
    match tree
        .node_by_id(&"top.style.thickness-value".into())
        .expect("eraser size numeral")
        .interact
        .as_ref()
        .map(|interact| &interact.event)
    {
        Some(ToolbarEvent::OpenPrecisionEntry(
            crate::ui::toolbar::PrecisionEntryTarget::Thickness,
        )) => {}
        other => panic!("numeral opens the precise entry, got {other:?}"),
    }

    // Shapes: the Fill mini-toggle joins the stroke controls.
    let rect = snapshot_for_tool(Tool::Rect);
    let tree = build(&rect);
    let fill = tree
        .node_by_id(&"top.style.fill".into())
        .expect("fill toggle");
    assert!(matches!(
        fill.kind,
        WidgetKind::MiniCheckbox { checked, .. } if checked == rect.fill_enabled
    ));
    assert!(matches!(
        fill.interact.as_ref().unwrap().event,
        ToolbarEvent::ToggleFill(value) if value == !rect.fill_enabled
    ));

    // Arrow: auto-number toggle, and the reset button (with the next-N
    // tooltip) only while numbering is enabled.
    let mut arrow = snapshot_for_tool(Tool::Arrow);
    arrow.arrow_label_enabled = false;
    let tree = build(&arrow);
    assert!(tree.node_by_id(&"top.style.auto-number".into()).is_some());
    assert!(
        tree.node_by_id(&"top.style.counter-reset.arrow".into())
            .is_none()
    );
    arrow.arrow_label_enabled = true;
    arrow.arrow_label_next = 7;
    let tree = build(&arrow);
    let reset = tree
        .node_by_id(&"top.style.counter-reset.arrow".into())
        .expect("arrow counter reset");
    let interaction = reset.interact.as_ref().unwrap();
    assert!(matches!(
        interaction.event,
        ToolbarEvent::ResetArrowLabelCounter
    ));
    assert_eq!(
        interaction.tooltip.as_deref(),
        Some("Reset numbering to 1 (next: 7)")
    );

    // Step marker: the reset targets the step counter.
    let mut step = snapshot_for_tool(Tool::StepMarker);
    step.step_marker_next = 4;
    let tree = build(&step);
    let reset = tree
        .node_by_id(&"top.style.counter-reset.step".into())
        .expect("step counter reset");
    let interaction = reset.interact.as_ref().unwrap();
    assert!(matches!(
        interaction.event,
        ToolbarEvent::ResetStepMarkerCounter
    ));
    assert_eq!(
        interaction.tooltip.as_deref(),
        Some("Reset numbering to 1 (next: 4)")
    );

    // Text: pt-labelled size slider plus the Sans/Mono segment.
    let mut text = snapshot();
    text.text_active = true;
    let tree = build(&text);
    let size = tree
        .node_by_id(&"top.style.font-size".into())
        .expect("font size slider");
    assert_eq!(
        size.interact.as_ref().unwrap().kind,
        HitKind::DragSetFontSize
    );
    match &tree
        .node_by_id(&"top.style.font-size-value".into())
        .expect("font size numeral")
        .kind
    {
        WidgetKind::TextButton { label, .. } => {
            assert_eq!(label.text, format!("{:.0}pt", text.font_size));
        }
        other => panic!("numeral kind, got {other:?}"),
    }
    assert!(matches!(
        &tree
            .node_by_id(&"top.style.font-family".into())
            .expect("font family segment")
            .kind,
        WidgetKind::SegmentedControl { left, right, .. }
            if left.text == "Sans" && right.text == "Mono"
    ));
    for (id, family) in [
        ("top.style.font-family.sans", "Sans"),
        ("top.style.font-family.mono", "Monospace"),
    ] {
        let half = tree.node_by_id(&id.into()).expect("family half");
        assert!(matches!(
            &half.interact.as_ref().unwrap().event,
            ToolbarEvent::SetFont(font) if font.family == family
        ));
    }
    assert!(!style_ids(&tree).contains(&"top.style.thickness".to_string()));
}

#[test]
fn style_pill_geometry_holds_per_tool_and_select_hides_the_pill() {
    use crate::input::Tool;

    // Select without a selection: the pill yields entirely — no pill node,
    // no fourth input rect, no extra height for it.
    let select = snapshot_for_tool(Tool::Select);
    let (w, h) = top_size(&select);
    let tree = build_top_view(&select, w as f64, h as f64);
    assert!(tree.node_by_id(&"top.island.style".into()).is_none());
    assert!(style_ids(&tree).is_empty());
    let rects = top_input_rects(&select, w as f64, h as f64).expect("island input rects");
    assert_eq!(rects.len(), 3, "no pill rect for Select: {rects:?}");

    // Select with a selection: the docked properties bring the pill (and
    // its fourth input rect) back, with the same geometry contract.
    let mut selection = snapshot_for_tool(Tool::Select);
    selection.selection_properties = vec![
        crate::input::SelectionPropertyEntry {
            label: "Color".to_string(),
            value: "Red".to_string(),
            kind: crate::input::SelectionPropertyKind::Color,
            disabled: false,
        },
        crate::input::SelectionPropertyEntry {
            label: "Thickness".to_string(),
            value: "3.0px".to_string(),
            kind: crate::input::SelectionPropertyKind::Thickness,
            disabled: false,
        },
    ];
    let (w, h) = top_size(&selection);
    let tree = build_top_view(&selection, w as f64, h as f64);
    let style = tree
        .node_by_id(&"top.island.style".into())
        .expect("selection pill card");
    assert_eq!(
        style.rect.3,
        ToolbarLayoutSpec::TOP_STYLE_PILL_H,
        "selection pill height"
    );
    assert_eq!(
        style_ids(&tree),
        [
            "top.style.sel.color",
            "top.style.sel.thickness.minus",
            "top.style.sel.thickness.value",
            "top.style.sel.thickness.plus",
        ]
    );
    let rects = top_input_rects(&selection, w as f64, h as f64).expect("island input rects");
    assert_eq!(rects.len(), 4, "selection pill rect: {rects:?}");
    let pill_rect = rects[3];
    assert_eq!(
        (pill_rect.0, pill_rect.1, pill_rect.2, pill_rect.3),
        style.rect,
        "pill input rect matches the card"
    );

    for tool in [
        Tool::Pen,
        Tool::Marker,
        Tool::Eraser,
        Tool::Rect,
        Tool::Arrow,
        Tool::StepMarker,
    ] {
        let snapshot = snapshot_for_tool(tool);
        let (w, h) = top_size(&snapshot);
        let tree = build_top_view(&snapshot, w as f64, h as f64);
        let tools = tree
            .node_by_id(&"top.island.tools".into())
            .expect("tools island");
        let style = tree
            .node_by_id(&"top.island.style".into())
            .unwrap_or_else(|| panic!("{tool:?} style pill"));

        // Detached fourth pill: left-aligned with island A, one gap under
        // the band, pill-token height.
        assert_eq!(style.rect.0, 0.0, "{tool:?}");
        assert!(
            (style.rect.1 - (tools.rect.1 + tools.rect.3 + ToolbarLayoutSpec::TOP_STYLE_PILL_GAP))
                .abs()
                < 1e-9,
            "{tool:?} pill y"
        );
        assert_eq!(
            style.rect.3,
            ToolbarLayoutSpec::TOP_STYLE_PILL_H,
            "{tool:?}"
        );

        // Every pill control paints and hits inside the pill card.
        let ids = style_ids(&tree);
        assert!(!ids.is_empty(), "{tool:?} pill content");
        for node in tree.nodes() {
            if node.id.as_str().starts_with("top.style.") {
                assert!(
                    node.rect.0 >= style.rect.0 - 1e-9
                        && node.rect.1 >= style.rect.1 - 1e-9
                        && node.rect.0 + node.rect.2 <= style.rect.0 + style.rect.2 + 1e-9
                        && node.rect.1 + node.rect.3 <= style.rect.1 + style.rect.3 + 1e-9,
                    "{tool:?} {} escapes the pill",
                    node.id
                );
            }
        }

        // The pill joins the surface input region as the fourth rect.
        let rects = top_input_rects(&snapshot, w as f64, h as f64).expect("island input rects");
        assert_eq!(rects.len(), 4, "{tool:?} rects: {rects:?}");
        assert_eq!(rects[3], style.rect, "{tool:?} pill input rect");
    }
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
