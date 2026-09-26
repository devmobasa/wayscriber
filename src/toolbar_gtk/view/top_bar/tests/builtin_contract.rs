//! Built-in toolbar order and semantic contract tests.

use super::super::{
    Tool, ToolbarLayoutMode, ToolbarSnapshot, TopStripPlan, model, plan_top_strip, top_toolbar_size,
};
use super::builtin_style_nodes::assert_builtin_style_pill_node;
use super::expectations::{
    SemanticAdapterRecord, SemanticControlRecord, SemanticLane, expected_semantic_records,
    stroke_controls_scenarios, style_pill_controls, style_pill_selection_snapshot,
    style_pill_tool_snapshot,
};
use crate::input::state::test_support::make_test_input_state;
use crate::ui::toolbar::ToolbarBindingHints;

fn record_id(record: &SemanticAdapterRecord) -> &str {
    match record {
        SemanticAdapterRecord::Divider(id) => id,
        SemanticAdapterRecord::Control(control) => &control.id,
    }
}

fn record_lane(record: &SemanticAdapterRecord) -> SemanticLane {
    match record {
        SemanticAdapterRecord::Divider(_) => SemanticLane::Strip,
        SemanticAdapterRecord::Control(control) => control.lane,
    }
}

fn assert_builtin_node(
    node: &crate::backend::wayland::TopToolbarWidgetKind,
    interaction: Option<&crate::ui::toolbar::ToolbarEvent>,
    tooltip: Option<&str>,
    shortcut_badge: Option<&str>,
    expected: &SemanticAdapterRecord,
) {
    use crate::backend::wayland::TopToolbarWidgetKind as W;

    let SemanticAdapterRecord::Control(expected) = expected else {
        assert!(matches!(node, W::Divider { vertical: true }));
        return;
    };

    assert_eq!(interaction, expected.enabled.then_some(&expected.event));
    assert_eq!(
        tooltip,
        expected.enabled.then_some(expected.tooltip.as_str())
    );
    assert_eq!(shortcut_badge, expected.shortcut_badge.as_deref());

    assert_builtin_node_kind(node, expected);
}

fn assert_builtin_node_kind(
    node: &crate::backend::wayland::TopToolbarWidgetKind,
    expected: &SemanticControlRecord,
) {
    use crate::backend::wayland::TopToolbarWidgetKind as W;

    match node {
        W::IconButton {
            glyph,
            icon_size: _,
            style,
        } => {
            let icon = expected.icon.expect("semantic icon for icon button");
            assert!(std::ptr::fn_addr_eq(
                glyph.0,
                crate::toolbar_icons::top_toolbar_icon_painter(icon)
            ));
            assert_builtin_button_style(style.active, style.disabled, style.destructive, expected);
        }
        W::TextButton { label, style } => {
            assert_eq!(label.text, expected.label);
            assert_builtin_button_style(style.active, style.disabled, style.destructive, expected);
        }
        W::Swatch { selected, .. } => assert_eq!(*selected, expected.active),
        W::PresetSlot {
            glyph,
            label,
            active,
            ..
        } => {
            assert_eq!(*active, expected.active);
            match expected.icon {
                // A filled slot carries the saved tool glyph; an empty slot
                // has no glyph and shows its 1-based number label.
                Some(icon) => {
                    let glyph = glyph.as_ref().expect("filled preset slot has a glyph");
                    assert!(std::ptr::fn_addr_eq(
                        glyph.0,
                        crate::toolbar_icons::top_toolbar_icon_painter(icon)
                    ));
                }
                None => {
                    assert!(glyph.is_none(), "empty preset slot has no glyph");
                    assert_eq!(*label, expected.label);
                }
            }
        }
        W::MicroChip { glyph, .. } => {
            let icon = expected.icon.expect("semantic icon for micro chip");
            assert!(std::ptr::fn_addr_eq(
                glyph.0,
                crate::toolbar_icons::top_toolbar_icon_painter(icon)
            ));
            assert_eq!(expected.role, model::TopToolbarControlRole::Restore);
        }
        W::RestoreTab { glyph, label } => {
            let icon = expected.icon.expect("semantic icon for the restore tab");
            assert!(std::ptr::fn_addr_eq(
                glyph.0,
                crate::toolbar_icons::top_toolbar_icon_painter(icon)
            ));
            assert_eq!(label.text, expected.label);
            assert_eq!(expected.role, model::TopToolbarControlRole::Restore);
        }
        W::PinButton { pinned } => assert_eq!(*pinned, expected.active),
        W::MiniCheckbox { checked, label } => {
            assert_eq!(*checked, expected.active);
            assert_eq!(label.text, expected.label);
        }
        W::DragHandle | W::MinimizeButton => {}
        other => panic!("unexpected semantic control kind: {other:?}"),
    }
}

fn assert_builtin_button_style(
    active: bool,
    disabled: bool,
    destructive: bool,
    expected: &SemanticControlRecord,
) {
    assert_eq!(active, expected.active);
    assert_eq!(disabled, !expected.enabled);
    assert_eq!(
        destructive,
        expected.role == model::TopToolbarControlRole::Destructive
    );
}

fn builtin_semantic_records(
    snapshot: &ToolbarSnapshot,
    expected: &[SemanticAdapterRecord],
) -> Vec<SemanticAdapterRecord> {
    let (width, height) = top_toolbar_size(&crate::ui_text::UiTextEngine::default(), snapshot);
    let tree = crate::backend::wayland::build_top_toolbar_view(
        &crate::ui_text::UiTextEngine::default(),
        snapshot,
        width as f64,
        height as f64,
    );
    let mut records = Vec::new();
    for node in tree.nodes() {
        let raw_id = node.id.as_str();
        let (lane, id) = if let Some(id) = raw_id.strip_prefix("top.overflow.") {
            (SemanticLane::Overflow, id)
        } else if raw_id == "top.utility.highlight-ring" {
            (SemanticLane::Contextual, raw_id)
        } else {
            let lane = expected
                .iter()
                .find(|record| record_id(record) == raw_id)
                .map(record_lane)
                .unwrap_or(SemanticLane::Strip);
            (lane, raw_id)
        };
        let Some(record) = expected
            .iter()
            .find(|record| record_id(record) == id && record_lane(record) == lane)
        else {
            continue;
        };
        assert_builtin_node(
            &node.kind,
            node.interact.as_ref().map(|interaction| &interaction.event),
            node.interact
                .as_ref()
                .and_then(|interaction| interaction.tooltip.as_deref()),
            node.shortcut_badge
                .as_ref()
                .map(|badge| badge.label.as_str()),
            record,
        );
        records.push(record.clone());
    }
    records
}

#[test]
fn shared_spec_matches_builtin_order_and_full_semantics_without_starting_a_gui() {
    let state = make_test_input_state();
    let regular = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let mut simple = regular.clone();
    simple.layout_mode = ToolbarLayoutMode::Simple;
    let mut minimized = regular.clone();
    minimized.top_minimized = true;
    let mut micro = regular.clone();
    micro.top_display_mode = crate::config::TopDisplayMode::Micro;
    let mut narrow = regular.clone();
    narrow.top_viewport_max = Some(520.0);
    narrow.top_overflow_open = true;
    let mut text = regular.clone();
    text.use_icons = false;
    let mut shapes = regular.clone();
    shapes.shape_picker_open = true;
    shapes.active_tool = Tool::RegularPolygon;
    let mut highlighted = regular.clone();
    highlighted.highlight_tool_active = true;

    for (name, snapshot) in [
        ("regular", regular),
        ("simple", simple),
        ("minimized", minimized),
        ("micro", micro),
        ("narrow", narrow),
        ("text", text),
        ("shapes", shapes),
        ("highlighted", highlighted),
    ] {
        let plan = plan_top_strip(&crate::ui_text::UiTextEngine::default(), &snapshot);
        let spec = super::super::strip::top_toolbar_spec(&snapshot, &plan);
        let expected = expected_semantic_records(&snapshot, &spec, &plan);
        for record in &expected {
            if let SemanticAdapterRecord::Control(control) = record {
                assert!(!control.accessible_label.is_empty(), "{name}: {control:?}");
            }
        }
        assert_eq!(
            expected,
            builtin_semantic_records(&snapshot, &expected),
            "{name} adapter semantics"
        );
    }
}

/// Expected builtin node kinds per style-pill control, in tree order.
pub(super) enum StylePillNodeExpectation {
    Control(model::StylePillControl),
    /// Inline readout decoration (the opacity slider's percent label).
    Readout(model::StylePillControl),
    /// Interactive half of a segmented control.
    SegmentHalf(model::StylePillControl, usize),
    /// Interactive −/+ half of a selection stepper.
    StepHalf(model::StylePillControl, usize),
    /// The value readout between the stepper halves (decor).
    StepValue(model::StylePillControl),
    /// The caption naming a tool meter or stepper, before its bars or its
    /// − half (decor).
    Caption(model::StylePillControl),
    /// One interactive bar of a level meter.
    MeterBar(model::StylePillControl, usize),
    /// The arrow style chip's drawn glyph and its name (decor laid over the
    /// chip's button body).
    ArrowChipGlyph,
    ArrowChipLabel,
}

fn expected_style_pill_nodes(
    snapshot: &ToolbarSnapshot,
    plan: &TopStripPlan,
) -> Vec<(String, StylePillNodeExpectation)> {
    let mut nodes = Vec::new();
    for (id, control) in style_pill_controls(snapshot, plan) {
        // Meters render as a caption and one node per bar, and steppers as
        // three nodes (−, readout, +), neither with a node carrying the
        // control id itself.
        if let Some(meter) = control.meter(snapshot) {
            nodes.push((
                format!("{id}.caption"),
                StylePillNodeExpectation::Caption(control),
            ));
            for (index, segment) in meter.segments.iter().enumerate() {
                nodes.push((
                    segment.id.clone(),
                    StylePillNodeExpectation::MeterBar(control, index),
                ));
            }
            continue;
        }
        if let Some(steps) = control.steps(snapshot) {
            if control.caption().is_some() {
                nodes.push((
                    format!("{id}.caption"),
                    StylePillNodeExpectation::Caption(control),
                ));
            }
            nodes.push((
                steps[0].id.to_string(),
                StylePillNodeExpectation::StepHalf(control, 0),
            ));
            nodes.push((
                format!("{id}.value"),
                StylePillNodeExpectation::StepValue(control),
            ));
            nodes.push((
                steps[1].id.to_string(),
                StylePillNodeExpectation::StepHalf(control, 1),
            ));
            continue;
        }
        nodes.push((id.clone(), StylePillNodeExpectation::Control(control)));
        if control == model::StylePillControl::ArrowStyleChip {
            nodes.push((
                format!("{id}.glyph"),
                StylePillNodeExpectation::ArrowChipGlyph,
            ));
            nodes.push((
                format!("{id}.label"),
                StylePillNodeExpectation::ArrowChipLabel,
            ));
        }
        if control.carries_inline_readout() {
            nodes.push((
                format!("{id}.readout"),
                StylePillNodeExpectation::Readout(control),
            ));
        }
        if let Some(segments) = control.segments(snapshot) {
            for (index, segment) in segments.iter().enumerate() {
                nodes.push((
                    segment.id.to_string(),
                    StylePillNodeExpectation::SegmentHalf(control, index),
                ));
            }
        }
    }
    nodes
}

#[test]
fn style_pill_spec_matches_builtin_tree_across_morph_states() {
    let state = make_test_input_state();
    let regular = ToolbarSnapshot::from_input_with_bindings(
        &state,
        ToolbarBindingHints::from_input_state(&state),
    );
    let mut arrow = style_pill_tool_snapshot(&regular, Tool::Arrow);
    arrow.arrow_label_enabled = true;
    arrow.arrow_label_next = 7;
    let mut text_mode = style_pill_tool_snapshot(&regular, Tool::Pen);
    text_mode.text_active = true;
    let mut minimized = regular.clone();
    minimized.top_minimized = true;
    let mut micro = regular.clone();
    micro.top_display_mode = crate::config::TopDisplayMode::Micro;

    for (name, snapshot) in [
        ("regular", regular.clone()),
        ("pen", style_pill_tool_snapshot(&regular, Tool::Pen)),
        (
            "shape-pen",
            style_pill_tool_snapshot(&regular, Tool::LiveShape),
        ),
        ("marker", style_pill_tool_snapshot(&regular, Tool::Marker)),
        ("eraser", style_pill_tool_snapshot(&regular, Tool::Eraser)),
        ("shape", style_pill_tool_snapshot(&regular, Tool::Rect)),
        ("arrow", arrow),
        (
            "step-marker",
            style_pill_tool_snapshot(&regular, Tool::StepMarker),
        ),
        ("text-mode", text_mode),
        ("select", style_pill_tool_snapshot(&regular, Tool::Select)),
        ("selection", style_pill_selection_snapshot(&regular)),
        ("minimized", minimized),
        ("micro", micro),
    ]
    .into_iter()
    .chain(stroke_controls_scenarios(&regular))
    {
        assert_builtin_style_pill_scenario(name, &snapshot);
    }
}

fn assert_builtin_style_pill_scenario(name: &str, snapshot: &ToolbarSnapshot) {
    let plan = plan_top_strip(&crate::ui_text::UiTextEngine::default(), snapshot);
    let expected = expected_style_pill_nodes(snapshot, &plan);
    let (width, height) = top_toolbar_size(&crate::ui_text::UiTextEngine::default(), snapshot);
    let tree = crate::backend::wayland::build_top_toolbar_view(
        &crate::ui_text::UiTextEngine::default(),
        snapshot,
        width as f64,
        height as f64,
    );

    assert_eq!(
        tree.node_by_id(&"top.island.style".into()).is_some(),
        !expected.is_empty(),
        "{name}: the pill card exists exactly when the spec has controls"
    );

    let actual: Vec<_> = tree
        .nodes()
        .iter()
        .filter(|node| node.id.as_str().starts_with("top.style."))
        .collect();
    assert_eq!(
        actual
            .iter()
            .map(|node| node.id.as_str().to_string())
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>(),
        "{name}: builtin pill node order"
    );

    for (node, (id, expectation)) in actual.iter().zip(&expected) {
        assert_builtin_style_pill_node(
            name,
            snapshot,
            &node.kind,
            node.interact.as_ref().map(|interaction| &interaction.event),
            node.interact
                .as_ref()
                .and_then(|interaction| interaction.tooltip.as_deref()),
            node.interact.is_some(),
            id,
            expectation,
        );
    }
}
