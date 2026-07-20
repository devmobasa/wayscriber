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
        // Colors left the strip for the pill (M7-C1); the presets island now
        // occupies the seam between the tools and history islands (M7-C2).
        "top.preset.0",
        "top.preset.4",
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

    // Colors moved to the pill: no swatches or current-color chip remain.
    assert!(!ids.iter().any(|id| id.starts_with("top.quick-color.")));
    assert!(!ids.contains(&"top.group.quick-colors"));

    // Divider-chunked groups exist inside the tools island; the colors
    // divider left with the colors, the old history divider became the
    // tools|history island gap.
    assert!(ids.contains(&"top.divider.tools"));
    assert!(ids.contains(&"top.divider.annotations"));
    assert!(!ids.contains(&"top.divider.colors"));
    assert!(!ids.contains(&"top.divider.history"));

    // The five pill islands back the strip: the band's four (tools, presets,
    // history, chrome) plus the contextual style pill underneath.
    assert!(ids.contains(&"top.island.tools"));
    assert!(ids.contains(&"top.island.presets"));
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
    let presets = island("presets");
    let history = island("history");
    let chrome = island("chrome");

    let style = island("style");

    // Pills read left to right with clear gaps between their edges: tools,
    // presets, history, then the right-aligned chrome.
    let gap = ToolbarLayoutSpec::TOP_ISLAND_GAP;
    assert!((presets.rect.0 - (tools.rect.0 + tools.rect.2) - gap).abs() < 1e-9);
    assert!((history.rect.0 - (presets.rect.0 + presets.rect.2) - gap).abs() < 1e-9);
    assert!(chrome.rect.0 >= history.rect.0 + history.rect.2 + gap - 1e-9);
    // Pills hug the surface edges and paint the panel treatment.
    assert_eq!(tools.rect.0, 0.0);
    let (w, _) = tree.size();
    assert!((chrome.rect.0 + chrome.rect.2 - w).abs() < 1e-9);
    for node in [&tools, &presets, &history, &chrome, &style] {
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
fn presets_render_as_slot_buttons_in_the_presets_island() {
    use crate::input::Tool;

    let mut snapshot = snapshot();
    snapshot.presets = vec![None; 5];
    snapshot.presets[0] = Some(crate::ui::toolbar::PresetSlotSnapshot {
        name: Some("Red pen".to_string()),
        tool: Tool::Pen,
        color: crate::draw::Color::new(1.0, 0.0, 0.0, 1.0),
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
    let tree = build(&snapshot);

    // No color swatch survives in the strip; five preset slots take the seam
    // and all sit inside the presets island.
    assert!(tree.node_by_id(&"top.quick-color.0".into()).is_none());
    assert!(tree.node_by_id(&"top.group.quick-colors".into()).is_none());
    let island = tree
        .node_by_id(&"top.island.presets".into())
        .expect("presets island");
    for index in 0..5 {
        let node = tree
            .node_by_id(&format!("top.preset.{index}").into())
            .expect("preset slot node");
        assert!(
            node.rect.0 >= island.rect.0 - 1e-9
                && node.rect.0 + node.rect.2 <= island.rect.0 + island.rect.2 + 1e-9
                && node.rect.1 >= island.rect.1 - 1e-9
                && node.rect.1 + node.rect.3 <= island.rect.1 + island.rect.3 + 1e-9,
            "slot {index} stays inside the presets island"
        );
    }

    // The filled slot draws the saved tool glyph in the neutral foreground and
    // carries the preset color as a separate corner swatch; it reads active
    // (it is the applied slot); clicking applies it.
    let filled = tree
        .node_by_id(&"top.preset.0".into())
        .expect("filled slot");
    match &filled.kind {
        WidgetKind::PresetSlot {
            glyph,
            color,
            active,
            ..
        } => {
            assert!(glyph.is_some(), "filled slot carries a glyph");
            assert_eq!(*color, (1.0, 0.0, 0.0, 1.0));
            assert!(*active);
        }
        other => panic!("preset slot kind, got {other:?}"),
    }
    assert!(matches!(
        filled.interact.as_ref().unwrap().event,
        ToolbarEvent::ApplyPreset(1)
    ));

    // The empty slot shows its 1-based number and saves the setup on click.
    let empty = tree.node_by_id(&"top.preset.1".into()).expect("empty slot");
    match &empty.kind {
        WidgetKind::PresetSlot {
            glyph,
            label,
            active,
            ..
        } => {
            assert!(glyph.is_none(), "empty slot has no glyph");
            assert_eq!(label, "2");
            assert!(!*active);
        }
        other => panic!("preset slot kind, got {other:?}"),
    }
    assert!(matches!(
        empty.interact.as_ref().unwrap().event,
        ToolbarEvent::SavePreset(2)
    ));

    // The presets island joins the surface input region as its own rect.
    let (w, h) = top_size(&snapshot);
    let rects = top_input_rects(&snapshot, w as f64, h as f64).expect("input rects");
    assert!(rects.contains(&island.rect));
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

    // Colors left the strip (M7-C1), so no swatch badges remain there.
    assert!(tree.node_by_id(&"top.quick-color.0".into()).is_none());

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
        5,
        "band pills (tools/presets/history/chrome) plus the style pill: {rects:?}"
    );
    // The gaps between the band islands click through to the canvas even in
    // the common no-popover state: consecutive island rects do not touch.
    assert!(rects[0].0 + rects[0].2 < rects[1].0);
    assert!(rects[1].0 + rects[1].2 < rects[2].0);
    assert!(rects[2].0 + rects[2].2 < rects[3].0);
    // The style pill is the fifth rect, detached below the band.
    assert!(rects[4].1 > rects[0].1 + rects[0].3);
    let tree = build_top_view(&snapshot, w as f64, h as f64);
    let islands: Vec<_> = tree
        .nodes()
        .iter()
        .filter(|node| node.id.as_str().starts_with("top.island."))
        .collect();
    assert_eq!(islands.len(), 5);
    for (island, input_rect) in islands.iter().zip(&rects) {
        assert_eq!(island.rect, *input_rect, "{}", island.id);
    }

    state.toolbar_shapes_expanded = true;
    let snapshot =
        ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default());
    let (w, h) = top_size(&snapshot);
    let rects = top_input_rects(&snapshot, w as f64, h as f64).expect("partial input region");
    assert_eq!(rects.len(), 6, "five islands + shapes panel: {rects:?}");
    assert_eq!(rects[0].0, 0.0);
    assert_eq!(rects[0].1, 0.0);
    for island in &rects[..5] {
        assert!(
            island.1 + island.3 < h as f64,
            "islands end above the popover"
        );
    }
    // The gaps between the band islands stay click-through: consecutive
    // island rects do not touch.
    assert!(rects[0].0 + rects[0].2 < rects[1].0);
    assert!(rects[1].0 + rects[1].2 < rects[2].0);
    assert!(rects[2].0 + rects[2].2 < rects[3].0);
    let tree = build_top_view(&snapshot, w as f64, h as f64);
    let panel = tree
        .node_by_id(&"top.shapes.panel".into())
        .expect("panel node");
    assert!(rects[5].1 <= panel.rect.1 && rects[5].3 >= panel.rect.3);
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
    assert_eq!(islands.len(), 5, "tools/presets/history/chrome/style pills");
    for (island, input_rect) in islands.iter().zip(&input_rects[..5]) {
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
    assert_eq!(
        rects.len(),
        4,
        "no pill rect for Select (tools/presets/history/chrome): {rects:?}"
    );

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
    assert_eq!(rects.len(), 5, "selection pill rect: {rects:?}");
    let pill_rect = rects[4];
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

        // The pill joins the surface input region as the fifth rect (after
        // the tools, presets, history, and chrome band islands).
        let rects = top_input_rects(&snapshot, w as f64, h as f64).expect("island input rects");
        assert_eq!(rects.len(), 5, "{tool:?} rects: {rects:?}");
        assert_eq!(rects[4], style.rect, "{tool:?} pill input rect");
    }
}

#[test]
fn overflow_menu_always_carries_the_canvas_session_and_settings_entries() {
    let mut snapshot = snapshot();
    snapshot.top_overflow_open = true;
    let tree = build(&snapshot);

    let canvas = tree
        .node_by_id(&"top.overflow.top.menu.canvas".into())
        .expect("Canvas... entry");
    assert!(matches!(
        canvas.interact.as_ref().unwrap().event,
        ToolbarEvent::ToggleCanvasPopover(true)
    ));
    let session = tree
        .node_by_id(&"top.overflow.top.menu.session".into())
        .expect("Session... entry");
    assert!(matches!(
        session.interact.as_ref().unwrap().event,
        ToolbarEvent::ToggleSessionPopover(true)
    ));
    let settings = tree
        .node_by_id(&"top.overflow.top.menu.settings".into())
        .expect("Settings... entry");
    assert!(matches!(
        settings.interact.as_ref().unwrap().event,
        ToolbarEvent::ToggleSettingsPopover(true)
    ));
}

fn canvas_tree_has_event(tree: &WidgetTree, event: &ToolbarEvent) -> bool {
    tree.nodes().iter().any(|node| {
        node.id.as_str().starts_with("top.menu.canvas.")
            && node
                .interact
                .as_ref()
                .is_some_and(|interact| &interact.event == event)
    })
}

#[test]
fn canvas_popover_sections_are_gated_by_their_display_toggles() {
    // Every section starts off; a mutator enables just what it needs, so the
    // content stays short of the scroll viewport and nothing is withheld.
    let build_open = |mutate: &dyn Fn(&mut ToolbarSnapshot)| {
        let mut s = snapshot();
        s.canvas_popover_open = true;
        s.show_boards_section = false;
        s.show_pages_section = false;
        s.show_zoom_actions = false;
        s.show_actions_advanced = false;
        s.show_step_section = false;
        mutate(&mut s);
        build(&s)
    };

    let boards = build_open(&|s| s.show_boards_section = true);
    assert!(
        boards
            .node_by_id(&"top.menu.canvas.panel".into())
            .is_some_and(|node| matches!(node.kind, WidgetKind::Popover { .. }))
    );
    assert!(canvas_tree_has_event(&boards, &ToolbarEvent::BoardNew));
    assert!(!canvas_tree_has_event(&boards, &ToolbarEvent::PageNew));

    let pages = build_open(&|s| s.show_pages_section = true);
    assert!(canvas_tree_has_event(&pages, &ToolbarEvent::PageNew));
    assert!(!canvas_tree_has_event(&pages, &ToolbarEvent::BoardNew));

    let zoom = build_open(&|s| {
        s.show_zoom_actions = true;
        // Reset/Lock are enabled (and thus carry interactions) only while a
        // zoom is active.
        s.zoom_active = true;
    });
    assert!(canvas_tree_has_event(&zoom, &ToolbarEvent::ZoomIn));
    assert!(canvas_tree_has_event(&zoom, &ToolbarEvent::ToggleZoomLock));

    // UndoAll/RedoAll carry interactions only while history is available.
    let advanced = build_open(&|s| {
        s.show_actions_advanced = true;
        s.delay_actions_enabled = true;
        s.undo_available = true;
        s.redo_available = true;
    });
    assert!(canvas_tree_has_event(&advanced, &ToolbarEvent::UndoAll));
    assert!(canvas_tree_has_event(&advanced, &ToolbarEvent::RedoAll));
    assert!(
        canvas_tree_has_event(&advanced, &ToolbarEvent::UndoAllDelayed),
        "timed advanced actions show when delay actions are enabled"
    );
    assert!(canvas_tree_has_event(
        &advanced,
        &ToolbarEvent::ToggleFreeze
    ));

    // Advanced without delay actions drops only the timed variants. Freeze is
    // always enabled, so it is the stable membership probe here.
    let advanced_no_delay = build_open(&|s| {
        s.show_actions_advanced = true;
        s.delay_actions_enabled = false;
    });
    assert!(canvas_tree_has_event(
        &advanced_no_delay,
        &ToolbarEvent::ToggleFreeze
    ));
    assert!(!canvas_tree_has_event(
        &advanced_no_delay,
        &ToolbarEvent::UndoAllDelayed
    ));

    // Empty popover: with every section off the popover has no panel.
    let empty = build_open(&|_| {});
    assert!(
        empty.node_by_id(&"top.menu.canvas.panel".into()).is_none(),
        "an all-off Canvas popover renders nothing"
    );
}

#[test]
fn canvas_popover_step_section_carries_toggles_steppers_and_delay_sliders() {
    let mut snapshot = snapshot();
    snapshot.canvas_popover_open = true;
    snapshot.show_boards_section = false;
    snapshot.show_pages_section = false;
    snapshot.show_zoom_actions = false;
    snapshot.show_actions_advanced = false;
    snapshot.show_step_section = true;
    snapshot.custom_section_enabled = true;
    snapshot.show_delay_sliders = true;

    let tree = build(&snapshot);
    let panel = tree
        .node_by_id(&"top.menu.canvas.panel".into())
        .expect("canvas popover panel");
    assert_eq!(
        panel.rect.2,
        super::menus::CANVAS_MENU_CONTENT_W + 20.0,
        "builtin Canvas popover uses the shared content width plus panel padding"
    );

    // Both step config toggles are present.
    assert!(
        tree.node_by_id(&"top.menu.canvas.step.toggle.buttons".into())
            .is_some()
    );
    assert!(
        tree.node_by_id(&"top.menu.canvas.step.toggle.delays".into())
            .is_some()
    );
    // Step-count steppers reuse the custom-steps events.
    assert!(canvas_tree_has_event(&tree, &ToolbarEvent::CustomUndo));
    assert!(
        tree.node_by_id(&"top.menu.canvas.step.undo.plus".into())
            .is_some()
    );

    // The per-direction and global delay sliders are draggable Slider nodes
    // wired to the shared delay drag kinds.
    use crate::backend::wayland::toolbar::events::HitKind;
    let undo_row = tree
        .node_by_id(&"top.menu.canvas.step.undo.delay".into())
        .expect("per-direction undo delay slider");
    assert!(matches!(undo_row.kind, WidgetKind::Slider { .. }));
    assert!(matches!(
        undo_row.interact.as_ref().unwrap().kind,
        HitKind::DragCustomUndoDelay
    ));
    let global_undo = tree
        .node_by_id(&"top.menu.canvas.step.global.undo.slider".into())
        .expect("global undo delay slider");
    assert!(matches!(
        global_undo.interact.as_ref().unwrap().kind,
        HitKind::DragUndoDelay
    ));

    // The step rows and global sliders vanish when their toggles are off.
    let mut collapsed = snapshot.clone();
    collapsed.custom_section_enabled = false;
    collapsed.show_delay_sliders = false;
    let collapsed_tree = build(&collapsed);
    assert!(
        collapsed_tree
            .node_by_id(&"top.menu.canvas.step.undo.delay".into())
            .is_none(),
        "custom rows hide when Step buttons is off"
    );
    assert!(
        collapsed_tree
            .node_by_id(&"top.menu.canvas.step.global.undo.slider".into())
            .is_none(),
        "global sliders hide when Delay sliders is off"
    );

    // Rendered nodes paint inside the panel.
    for node in tree.nodes() {
        if node.id.as_str().starts_with("top.menu.canvas.")
            && node.id.as_str() != "top.menu.canvas.panel"
            && node.id.as_str() != "top.menu.canvas.scrollbar"
        {
            assert!(
                node.rect.0 >= panel.rect.0 - 1e-9
                    && node.rect.1 >= panel.rect.1 - 1e-9
                    && node.rect.0 + node.rect.2 <= panel.rect.0 + panel.rect.2 + 1e-9
                    && node.rect.1 + node.rect.3 <= panel.rect.1 + panel.rect.3 + 1e-9,
                "{} escapes the popover panel",
                node.id
            );
        }
    }
}

#[test]
fn canvas_popover_scrolls_when_all_sections_exceed_the_viewport() {
    let mut snapshot = snapshot();
    snapshot.canvas_popover_open = true;
    snapshot.show_boards_section = true;
    snapshot.show_pages_section = true;
    snapshot.show_zoom_actions = true;
    snapshot.show_actions_advanced = true;
    snapshot.show_step_section = true;
    snapshot.custom_section_enabled = true;
    snapshot.show_delay_sliders = true;
    snapshot.delay_actions_enabled = true;

    let bounds = top_popover_scroll_bounds(&snapshot).expect("canvas scroll bounds");
    assert!(
        bounds.0 > bounds.1,
        "the full section set is taller than the viewport: {bounds:?}"
    );

    let tree = build(&snapshot);
    assert!(
        tree.node_by_id(&"top.menu.canvas.scrollbar".into())
            .is_some(),
        "content taller than the viewport gets a scrollbar"
    );

    // The Canvas popover joins the input region as an extra rect.
    let (w, h) = top_size(&snapshot);
    let panel = tree
        .node_by_id(&"top.menu.canvas.panel".into())
        .expect("canvas panel");
    let rects = top_input_rects(&snapshot, w as f64, h as f64).expect("input rects");
    assert!(
        rects
            .iter()
            .any(|rect| rect.1 + rect.3 >= panel.rect.1 + panel.rect.3
                && rect.0 <= panel.rect.0
                && rect.0 + rect.2 >= panel.rect.0 + panel.rect.2),
        "canvas popover rect missing from {rects:?}"
    );
}

#[test]
fn session_popover_re_hosts_the_session_pane_content() {
    use std::path::PathBuf;

    let mut snapshot = snapshot();
    snapshot.session_popover_open = true;
    snapshot.active_session_name = Some("lecture.wayscriber-session".to_string());
    snapshot.active_session_path = Some(PathBuf::from("/tmp/lecture.wayscriber-session"));
    snapshot.recent_sessions = (0..2)
        .map(|index| crate::ui::toolbar::SessionRecentSnapshot {
            display_name: format!("recent-{index}.wayscriber-session"),
            path: PathBuf::from(format!("/tmp/recent-{index}.wayscriber-session")),
        })
        .collect();

    let (w, h) = top_size(&snapshot);
    let tree = build_top_view(&snapshot, w as f64, h as f64);
    let panel = tree
        .node_by_id(&"top.menu.session.panel".into())
        .expect("session popover panel");
    assert!(matches!(panel.kind, WidgetKind::Popover { .. }));

    // Content parity with the Session pane: the same model drives both, so
    // every model control appears exactly once with the same event.
    let model = crate::ui::toolbar::model::ToolbarSessionModel::for_popover(&snapshot)
        .expect("session model");
    assert_eq!(model.buttons.len(), 5);
    for button in &model.buttons {
        let node = tree
            .nodes()
            .iter()
            .find(|node| {
                node.id.as_str().starts_with("top.menu.session.")
                    && node
                        .interact
                        .as_ref()
                        .is_some_and(|interact| interact.event == button.event)
            })
            .unwrap_or_else(|| panic!("popover control for {:?}", button.event));
        match &node.kind {
            WidgetKind::TextButton { label, style } => {
                assert_eq!(label.text, button.label);
                assert_eq!(style.disabled, !button.enabled);
                assert_eq!(
                    style.destructive,
                    matches!(button.event, ToolbarEvent::ClearSession)
                );
            }
            other => panic!("session button kind, got {other:?}"),
        }
    }
    for (index, recent) in model.recents.iter().enumerate() {
        let node = tree
            .node_by_id(&format!("top.menu.session.recent.{index}").into())
            .expect("recent row");
        assert_eq!(node.interact.as_ref().unwrap().event, recent.event());
    }
    // Meta labels are decor; the pane's collapsible header is side chrome
    // and deliberately absent.
    assert!(tree.node_by_id(&"top.menu.session.name".into()).is_some());
    assert!(tree.node_by_id(&"top.menu.session.path".into()).is_some());

    // Every content node paints inside the panel.
    for node in tree.nodes() {
        if node.id.as_str().starts_with("top.menu.session.")
            && node.id.as_str() != "top.menu.session.panel"
        {
            assert!(
                node.rect.0 >= panel.rect.0 - 1e-9
                    && node.rect.1 >= panel.rect.1 - 1e-9
                    && node.rect.0 + node.rect.2 <= panel.rect.0 + panel.rect.2 + 1e-9
                    && node.rect.1 + node.rect.3 <= panel.rect.1 + panel.rect.3 + 1e-9,
                "{} escapes the popover panel",
                node.id
            );
        }
    }

    // The popover joins the input region as an extra rect below the band.
    let rects = top_input_rects(&snapshot, w as f64, h as f64).expect("input rects");
    assert!(
        rects
            .iter()
            .any(|rect| rect.1 + rect.3 >= panel.rect.1 + panel.rect.3
                && rect.0 <= panel.rect.0
                && rect.0 + rect.2 >= panel.rect.0 + panel.rect.2),
        "popover rect missing from {rects:?}"
    );

    // A pending Save-As overwrite swaps the button grid for the
    // confirmation, exactly like the pane.
    snapshot.pending_save_as_overwrite_path =
        Some(PathBuf::from("/tmp/existing.wayscriber-session"));
    let tree = build(&snapshot);
    let replace = tree
        .node_by_id(&"top.menu.session.confirm-replace".into())
        .expect("replace button");
    assert!(matches!(
        &replace.interact.as_ref().unwrap().event,
        ToolbarEvent::SaveSessionAsConfirm(path)
            if path.as_path() == std::path::Path::new("/tmp/existing.wayscriber-session")
    ));
    let cancel = tree
        .node_by_id(&"top.menu.session.confirm-cancel".into())
        .expect("cancel button");
    assert!(matches!(
        cancel.interact.as_ref().unwrap().event,
        ToolbarEvent::SaveSessionAsCancel
    ));
    assert!(tree.node_by_id(&"top.menu.session.open".into()).is_none());
}

#[test]
fn settings_popover_re_hosts_the_settings_pane_content() {
    let mut snapshot = snapshot();
    snapshot.settings_popover_open = true;

    let tree = build(&snapshot);
    let panel = tree
        .node_by_id(&"top.menu.settings.panel".into())
        .expect("settings popover panel");
    assert!(matches!(panel.kind, WidgetKind::Popover { .. }));

    let model = crate::ui::toolbar::model::ToolbarSettingsModel::for_popover(&snapshot)
        .expect("settings model");

    // Layout-mode segments render as three exclusive tabs.
    let control = crate::ui::toolbar::model::layout_mode_control(snapshot.layout_mode);
    let crate::ui::toolbar::model::ToolbarControlKind::Segmented(segmented) = &control.kind else {
        panic!("layout mode control is segmented");
    };
    for (index, segment) in segmented.segments().iter().enumerate() {
        let node = tree
            .node_by_id(&format!("top.menu.settings.mode.{index}").into())
            .expect("mode segment");
        assert_eq!(
            node.interact.as_ref().unwrap().event,
            segment.activation.compatibility_event()
        );
        assert!(matches!(
            &node.kind,
            WidgetKind::TextButton { style, .. }
                if style.active == (segmented.active_segment() == Some(segment.id))
        ));
    }

    // Every pane toggle appears once, as a checkbox with the same event.
    let toggles: Vec<_> = model.toggle_rows().into_iter().flatten().collect();
    assert!(!toggles.is_empty());
    for (index, toggle) in toggles.iter().enumerate() {
        let node = tree
            .node_by_id(&format!("top.menu.settings.toggle.{index}").into())
            .unwrap_or_else(|| panic!("toggle {index} ({})", toggle.label));
        assert_eq!(
            node.interact.as_ref().unwrap().event,
            toggle.activation.compatibility_event(),
            "{}",
            toggle.label
        );
        assert!(matches!(
            node.kind,
            WidgetKind::Checkbox { checked, .. } if checked == toggle.checked
        ));
    }

    // Every settings button appears once with the same event.
    for (index, button) in model.buttons().iter().enumerate() {
        let node = tree
            .node_by_id(&format!("top.menu.settings.button.{index}").into())
            .unwrap_or_else(|| panic!("button {index} ({})", button.label));
        assert_eq!(node.interact.as_ref().unwrap().event, button.event);
    }
}

#[test]
fn settings_popover_customization_rows_keep_reorder_and_drag_events() {
    let mut snapshot = snapshot();
    snapshot.settings_popover_open = true;
    snapshot.customize_items_open = true;
    snapshot.customize_items_group = Some(crate::ui::toolbar::ToolbarItemCustomizeGroup::TopTools);

    let tree = build(&snapshot);
    let model = crate::ui::toolbar::model::ToolbarSettingsModel::for_popover(&snapshot)
        .expect("settings model");
    let overrides = model.item_overrides();
    assert!(!overrides.is_empty());

    // Some rows may sit behind the internal scrollbar; the visible ones
    // must carry the pane's exact events (checkbox, up/down, row drag).
    let mut seen = 0;
    for (index, item) in overrides.iter().enumerate() {
        let Some(check) = tree.node_by_id(&format!("top.menu.settings.item.{index}").into()) else {
            continue;
        };
        seen += 1;
        assert_eq!(
            check.interact.as_ref().unwrap().event,
            item.activation.compatibility_event()
        );
        let order = item.order.as_ref().expect("top tools rows are orderable");
        let drag = tree
            .node_by_id(&format!("top.menu.settings.item.{index}.drag").into())
            .expect("row drag region");
        assert!(matches!(
            drag.interact.as_ref().unwrap().kind,
            crate::backend::wayland::toolbar::events::HitKind::DragToolbarItem { id, .. }
                if id == item.id
        ));
        let up = tree
            .node_by_id(&format!("top.menu.settings.item.{index}.up").into())
            .expect("move up");
        assert_eq!(
            up.interact.as_ref().map(|interact| &interact.event),
            order
                .can_move_up
                .then(|| order.move_up.compatibility_event())
                .as_ref()
        );
        let down = tree
            .node_by_id(&format!("top.menu.settings.item.{index}.down").into())
            .expect("move down");
        assert_eq!(
            down.interact.as_ref().map(|interact| &interact.event),
            order
                .can_move_down
                .then(|| order.move_down.compatibility_event())
                .as_ref()
        );
    }
    assert!(seen > 0, "at least the first rows are visible");

    // The customize back button replaces the settings buttons.
    let back = tree
        .node_by_id(&"top.menu.settings.button.0".into())
        .expect("back button");
    assert!(matches!(
        back.interact.as_ref().unwrap().event,
        ToolbarEvent::SetToolbarItemCustomizationGroup(None)
    ));
    // Customizing hides the layout-mode segments (pane parity).
    assert!(
        tree.node_by_id(&"top.menu.settings.mode.0".into())
            .is_none()
    );
}

#[test]
fn tall_popover_content_caps_the_panel_and_scrolls_internally() {
    let mut snapshot = snapshot();
    snapshot.settings_popover_open = true;
    snapshot.customize_items_open = true;
    // The Top controls group lists enough rows to exceed the cap.
    snapshot.customize_items_group =
        Some(crate::ui::toolbar::ToolbarItemCustomizeGroup::TopControls);

    let (w, h) = top_size(&snapshot);
    let tree = build_top_view(&snapshot, w as f64, h as f64);
    let panel = tree
        .node_by_id(&"top.menu.settings.panel".into())
        .expect("settings popover panel");
    assert!(
        panel.rect.3 <= super::menus::MENU_MAX_CONTENT_H + 2.0 * 10.0 + 1e-9,
        "panel height capped: {}",
        panel.rect.3
    );

    let scrollbar = tree
        .node_by_id(&"top.menu.settings.scrollbar".into())
        .expect("internal scrollbar");
    let interaction = scrollbar.interact.as_ref().expect("scroll drag");
    let crate::backend::wayland::toolbar::events::HitKind::DragScrollTopPopover { max_scroll } =
        interaction.kind
    else {
        panic!("scrollbar kind, got {:?}", interaction.kind);
    };
    assert!(max_scroll > 0.0);
    assert!(matches!(scrollbar.kind, WidgetKind::VScrollbar { .. }));

    // Content nodes never paint outside the panel, scrolled or not.
    let assert_content_inside = |tree: &WidgetTree, panel_rect: (f64, f64, f64, f64)| {
        for node in tree.nodes() {
            if node.id.as_str().starts_with("top.menu.settings.")
                && node.id.as_str() != "top.menu.settings.panel"
            {
                assert!(
                    node.rect.1 >= panel_rect.1 - 1e-9
                        && node.rect.1 + node.rect.3 <= panel_rect.1 + panel_rect.3 + 1e-9,
                    "{} escapes the capped panel",
                    node.id
                );
            }
        }
    };
    assert_content_inside(&tree, panel.rect);

    // Scrolling reveals the tail rows: the first row leaves, the last
    // appears, and the thumb reaches the bottom.
    let unscrolled_first = tree.node_by_id(&"top.menu.settings.button.0".into());
    assert!(unscrolled_first.is_some(), "top row visible before scroll");
    snapshot.top_popover_scroll = max_scroll + 500.0; // clamps to max
    let scrolled = build_top_view(&snapshot, w as f64, h as f64);
    let scrolled_panel = scrolled
        .node_by_id(&"top.menu.settings.panel".into())
        .expect("panel");
    assert_content_inside(&scrolled, scrolled_panel.rect);
    assert!(
        scrolled
            .node_by_id(&"top.menu.settings.button.0".into())
            .is_none(),
        "top row scrolled out"
    );
    let scrollbar = scrolled
        .node_by_id(&"top.menu.settings.scrollbar".into())
        .expect("scrollbar");
    assert!(matches!(
        scrollbar.kind,
        WidgetKind::VScrollbar { t, .. } if (t - 1.0).abs() < 1e-9
    ));
}

/// The wheel path scrolls the open popover against the same bounds the
/// tree's scrollbar drags against — the popover must stay wheel-scrollable
/// like the side panes it re-hosts (and the GTK `ScrolledWindow`).
#[test]
fn top_popover_scroll_bounds_serve_the_wheel_path() {
    let mut snapshot = snapshot();
    assert!(
        top_popover_scroll_bounds(&snapshot).is_none(),
        "no bounds while no menu popover is open"
    );

    snapshot.settings_popover_open = true;
    snapshot.customize_items_open = true;
    snapshot.customize_items_group =
        Some(crate::ui::toolbar::ToolbarItemCustomizeGroup::TopControls);
    let (natural, viewport) =
        top_popover_scroll_bounds(&snapshot).expect("bounds while the settings popover is open");
    assert_eq!(
        viewport,
        super::menus::MENU_MAX_CONTENT_H,
        "tall content caps at the viewport"
    );
    assert!(
        natural > viewport,
        "the Top-controls list overflows the cap"
    );

    // The wheel bounds and the scrollbar the tree builds agree on the exact
    // max scroll, so the two scroll paths can never diverge.
    let tree = build(&snapshot);
    let scrollbar = tree
        .node_by_id(&"top.menu.settings.scrollbar".into())
        .expect("internal scrollbar");
    let crate::backend::wayland::toolbar::events::HitKind::DragScrollTopPopover { max_scroll } =
        scrollbar.interact.as_ref().expect("scroll drag").kind
    else {
        panic!("scrollbar kind");
    };
    assert!((max_scroll - (natural - viewport)).abs() < 1e-9);

    // The session popover reports bounds too; its short default content
    // fits the viewport, so the wheel path has nothing to scroll.
    snapshot.settings_popover_open = false;
    snapshot.customize_items_open = false;
    snapshot.customize_items_group = None;
    snapshot.session_popover_open = true;
    let (natural, viewport) =
        top_popover_scroll_bounds(&snapshot).expect("bounds while the session popover is open");
    assert_eq!(natural, viewport, "short content never scrolls");
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
