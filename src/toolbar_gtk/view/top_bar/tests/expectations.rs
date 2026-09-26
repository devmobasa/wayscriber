//! Shared semantic expectations and snapshot scenarios.

use super::super::{Tool, ToolbarEvent, ToolbarSnapshot, TopStripPlan, model};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SemanticLane {
    Strip,
    Contextual,
    Chrome,
    Overflow,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct SemanticControlRecord {
    pub(super) lane: SemanticLane,
    pub(super) id: String,
    pub(super) event: ToolbarEvent,
    pub(super) label: String,
    pub(super) accessible_label: String,
    pub(super) tooltip: String,
    pub(super) shortcut_badge: Option<String>,
    pub(super) enabled: bool,
    pub(super) active: bool,
    pub(super) role: model::TopToolbarControlRole,
    pub(super) icon: Option<model::TopToolbarIcon>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum SemanticAdapterRecord {
    Divider(&'static str),
    Control(SemanticControlRecord),
}

pub(super) fn control_record(
    snapshot: &ToolbarSnapshot,
    lane: SemanticLane,
    control: model::TopToolbarControl,
    show_badge: bool,
) -> SemanticControlRecord {
    SemanticControlRecord {
        lane,
        id: control.id().render_id().into_owned(),
        event: control.event(snapshot),
        label: control.label(snapshot).into_owned(),
        accessible_label: control.accessible_label(snapshot).into_owned(),
        tooltip: if lane == SemanticLane::Overflow {
            control.overflow_tooltip(snapshot)
        } else {
            control.tooltip(snapshot)
        },
        shortcut_badge: show_badge
            .then(|| control.shortcut_badge(snapshot))
            .flatten(),
        enabled: control.enabled(snapshot),
        active: control.active(snapshot),
        role: control.role(),
        icon: control.icon(snapshot),
    }
}

pub(super) fn expected_semantic_records(
    snapshot: &ToolbarSnapshot,
    spec: &model::TopToolbarSpec,
    plan: &TopStripPlan,
) -> Vec<SemanticAdapterRecord> {
    let mut records = Vec::new();
    for node in spec.strip() {
        match *node {
            model::TopToolbarNode::Divider(divider) => {
                records.push(SemanticAdapterRecord::Divider(divider.id()));
            }
            model::TopToolbarNode::Control(control) => {
                // Colors left the strip (M7-C1); badges now ride tool and
                // utility buttons only, which drop to icons under compact.
                let show_badge = !plan.compact;
                records.push(SemanticAdapterRecord::Control(control_record(
                    snapshot,
                    SemanticLane::Strip,
                    control,
                    show_badge,
                )));
                if matches!(
                    control,
                    model::TopToolbarControl::Utility(model::TopToolbarUtility::Highlight)
                ) {
                    records.extend(spec.contextual().iter().copied().map(|contextual| {
                        SemanticAdapterRecord::Control(control_record(
                            snapshot,
                            SemanticLane::Contextual,
                            contextual,
                            false,
                        ))
                    }));
                }
            }
        }
    }
    records.extend(spec.chrome().iter().copied().map(|control| {
        SemanticAdapterRecord::Control(control_record(
            snapshot,
            SemanticLane::Chrome,
            control,
            false,
        ))
    }));
    if snapshot.top_overflow_open {
        records.extend(spec.overflow().iter().copied().map(|control| {
            SemanticAdapterRecord::Control(control_record(
                snapshot,
                SemanticLane::Overflow,
                control,
                !plan.compact,
            ))
        }));
    }
    records
}

/// Variant of `base` with one tool active and the settings overrides pinned
/// off, so each scenario exercises exactly one pure style-pill morph state.
pub(super) fn style_pill_tool_snapshot(base: &ToolbarSnapshot, tool: Tool) -> ToolbarSnapshot {
    let mut snapshot = base.clone();
    snapshot.active_tool = tool;
    snapshot.tool_override = None;
    snapshot.thickness_targets_eraser = tool == Tool::Eraser;
    snapshot.thickness_targets_marker = tool == Tool::Marker;
    snapshot.show_text_controls = false;
    snapshot.show_marker_opacity_section = false;
    snapshot
}

fn selection_property_entry(
    label: &str,
    value: &str,
    kind: crate::input::SelectionPropertyKind,
    disabled: bool,
) -> crate::input::SelectionPropertyEntry {
    crate::input::SelectionPropertyEntry {
        label: label.to_string(),
        value: value.to_string(),
        kind,
        disabled,
    }
}

/// Select tool with a docked selection: a cycle entry, a stepper entry,
/// and a locked (disabled) cycle entry.
pub(super) fn style_pill_selection_snapshot(base: &ToolbarSnapshot) -> ToolbarSnapshot {
    use crate::input::SelectionPropertyKind as K;
    let mut snapshot = style_pill_tool_snapshot(base, Tool::Select);
    snapshot.selection_properties = vec![
        selection_property_entry("Color", "Red", K::Color, false),
        selection_property_entry("Thickness", "3.0px", K::Thickness, false),
        selection_property_entry("Fill", "Locked", K::Fill, true),
    ];
    snapshot.selection_has_text = true;
    snapshot.selected_text_bold = Some(false);
    snapshot
}

/// Ordered `(id, control)` list of the style pill for one snapshot, from
/// the shared morph spec both frontends render.
pub(super) fn style_pill_controls(
    snapshot: &ToolbarSnapshot,
    plan: &TopStripPlan,
) -> Vec<(String, model::StylePillControl)> {
    model::StylePillSpec::build(snapshot, plan)
        .controls()
        .iter()
        .map(|control| (control.id().into_owned(), *control))
        .collect()
}

/// Pen, Marker, and Shape Pen in each `stroke_controls` style, plus Shape Pen
/// with its Pen feel panel open.
pub(super) fn stroke_controls_scenarios(
    regular: &ToolbarSnapshot,
) -> Vec<(&'static str, ToolbarSnapshot)> {
    use crate::config::ToolbarStrokeControls as S;

    let styled = |tool, style| {
        let mut snapshot = style_pill_tool_snapshot(regular, tool);
        snapshot.stroke_controls = style;
        snapshot
    };
    let mut open = styled(Tool::LiveShape, S::Panel);
    open.pen_feel_open = true;
    vec![
        ("pen-panel", styled(Tool::Pen, S::Panel)),
        ("pen-meter", styled(Tool::Pen, S::Meter)),
        ("pen-stepper", styled(Tool::Pen, S::Stepper)),
        ("marker-stepper", styled(Tool::Marker, S::Stepper)),
        ("shape-pen-panel", styled(Tool::LiveShape, S::Panel)),
        ("shape-pen-meter", styled(Tool::LiveShape, S::Meter)),
        ("shape-pen-stepper", styled(Tool::LiveShape, S::Stepper)),
        ("shape-pen-panel-open", open),
    ]
}

pub(super) fn slider_opacity_paint(
    control: model::StylePillControl,
    snapshot: &ToolbarSnapshot,
) -> Option<model::OpacityPaint> {
    match control {
        model::StylePillControl::Slider(slider) => slider.opacity_paint(snapshot),
        _ => None,
    }
}
