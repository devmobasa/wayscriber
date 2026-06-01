use super::super::super::events::HitKind;
use super::super::super::format_binding_label;
use super::super::super::hit::HitRegion;
use super::super::spec::ToolbarLayoutSpec;
use super::shape_buttons;
use super::tool_buttons;
use crate::config::{Action, action_label};
use crate::ui::toolbar::bindings::tool_tooltip_label;
use crate::ui::toolbar::model;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot};

pub(super) fn build_hits(
    height: f64,
    snapshot: &ToolbarSnapshot,
    spec: &ToolbarLayoutSpec,
    is_simple: bool,
    fill_tool_active: bool,
    hits: &mut Vec<HitRegion>,
) {
    let gap = ToolbarLayoutSpec::TOP_GAP;
    let mut x = ToolbarLayoutSpec::TOP_START_X;

    let (btn_size, _) = spec.top_button_size();
    let y = spec.top_button_y(height);
    let mut fill_anchor: Option<(f64, f64)> = None;
    let tool_buttons = tool_buttons(is_simple);

    let mut rect_x = None;
    let mut circle_end_x = None;
    for tool in tool_buttons {
        if model::is_fill_tool(*tool) && rect_x.is_none() {
            rect_x = Some(x);
        }
        if model::is_fill_tool(*tool) {
            circle_end_x = Some(x + btn_size);
        }
        hits.push(HitRegion {
            rect: (x, y, btn_size, btn_size),
            event: ToolbarEvent::SelectTool(*tool),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                tool_tooltip_label(*tool),
                snapshot.binding_hints.for_tool(*tool),
            )),
        });
        x += btn_size + gap;
    }

    if is_simple {
        hits.push(HitRegion {
            rect: (x, y, btn_size, btn_size),
            event: ToolbarEvent::ToggleShapePicker(!snapshot.shape_picker_open),
            kind: HitKind::Click,
            tooltip: Some("Shapes".to_string()),
        });
        if fill_tool_active && !snapshot.shape_picker_open {
            fill_anchor = Some((x, btn_size));
        }
        x += btn_size + gap;
    } else {
        let current_shape_tool =
            model::current_shape_tool(snapshot.active_tool, snapshot.tool_override);
        let current_polygon_tool = current_shape_tool.filter(|tool| model::is_polygon_tool(*tool));
        hits.push(HitRegion {
            rect: (x, y, btn_size, btn_size),
            event: ToolbarEvent::ToggleShapePicker(!snapshot.shape_picker_open),
            kind: HitKind::Click,
            tooltip: Some("Polygons".to_string()),
        });
        if current_polygon_tool.is_some() {
            fill_anchor = Some((x, btn_size));
        } else if let (Some(rect_x), Some(circle_end_x)) = (rect_x, circle_end_x) {
            fill_anchor = Some((rect_x, circle_end_x - rect_x));
        }
        x += btn_size + gap;
    }

    if fill_tool_active
        && !snapshot.shape_picker_open
        && let Some((fill_x, fill_w)) = fill_anchor
    {
        let fill_y = y + btn_size + ToolbarLayoutSpec::TOP_ICON_FILL_OFFSET;
        hits.push(HitRegion {
            rect: (
                fill_x,
                fill_y,
                fill_w,
                ToolbarLayoutSpec::TOP_ICON_FILL_HEIGHT,
            ),
            event: ToolbarEvent::ToggleFill(!snapshot.fill_enabled),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                action_label(Action::ToggleFill),
                snapshot
                    .binding_hints
                    .binding_for_action(Action::ToggleFill),
            )),
        });
    }

    hits.push(HitRegion {
        rect: (x, y, btn_size, btn_size),
        event: ToolbarEvent::EnterTextMode,
        kind: HitKind::Click,
        tooltip: Some(format_binding_label(
            action_label(Action::EnterTextMode),
            snapshot
                .binding_hints
                .binding_for_action(Action::EnterTextMode),
        )),
    });
    x += btn_size + gap;

    hits.push(HitRegion {
        rect: (x, y, btn_size, btn_size),
        event: ToolbarEvent::EnterStickyNoteMode,
        kind: HitKind::Click,
        tooltip: Some(format_binding_label(
            action_label(Action::EnterStickyNoteMode),
            snapshot
                .binding_hints
                .binding_for_action(Action::EnterStickyNoteMode),
        )),
    });
    x += btn_size + gap;

    if !is_simple {
        hits.push(HitRegion {
            rect: (x, y, btn_size, btn_size),
            event: ToolbarEvent::ClearCanvas,
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                action_label(Action::ClearCanvas),
                snapshot
                    .binding_hints
                    .binding_for_action(Action::ClearCanvas),
            )),
        });
        x += btn_size + gap;

        let highlight_x = x;
        hits.push(HitRegion {
            rect: (x, y, btn_size, btn_size),
            event: ToolbarEvent::ToggleAllHighlight(!snapshot.any_highlight_active),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                action_label(Action::ToggleHighlightTool),
                snapshot
                    .binding_hints
                    .binding_for_action(Action::ToggleHighlightTool),
            )),
        });
        if snapshot.highlight_tool_active {
            let ring_y = y + btn_size + ToolbarLayoutSpec::TOP_ICON_FILL_OFFSET;
            hits.push(HitRegion {
                rect: (
                    highlight_x,
                    ring_y,
                    btn_size,
                    ToolbarLayoutSpec::TOP_ICON_FILL_HEIGHT,
                ),
                event: ToolbarEvent::ToggleHighlightToolRing(!snapshot.highlight_tool_ring_enabled),
                kind: HitKind::Click,
                tooltip: Some("Highlight ring".to_string()),
            });
        }
        x += btn_size + gap;
    }

    hits.push(HitRegion {
        rect: (x, y, ToolbarLayoutSpec::TOP_TOGGLE_WIDTH, btn_size),
        event: ToolbarEvent::ToggleIconMode(false),
        kind: HitKind::Click,
        tooltip: None,
    });

    if snapshot.shape_picker_open {
        let mut shape_y = y + btn_size + ToolbarLayoutSpec::TOP_SHAPE_ROW_GAP;
        push_picker_hits(
            shape_y,
            btn_size,
            gap,
            if is_simple {
                model::common_shape_tools()
            } else {
                shape_buttons()
            },
            snapshot,
            hits,
        );
        if is_simple {
            shape_y += btn_size + ToolbarLayoutSpec::TOP_SHAPE_ROW_GAP;
            push_picker_hits(shape_y, btn_size, gap, shape_buttons(), snapshot, hits);
        }
    }
}

fn push_picker_hits(
    shape_y: f64,
    btn_size: f64,
    gap: f64,
    tools: &[crate::input::Tool],
    snapshot: &ToolbarSnapshot,
    hits: &mut Vec<HitRegion>,
) {
    let mut shape_x = ToolbarLayoutSpec::TOP_START_X + ToolbarLayoutSpec::TOP_HANDLE_SIZE + gap;
    for tool in tools {
        hits.push(HitRegion {
            rect: (shape_x, shape_y, btn_size, btn_size),
            event: ToolbarEvent::SelectTool(*tool),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                tool_tooltip_label(*tool),
                snapshot.binding_hints.for_tool(*tool),
            )),
        });
        shape_x += btn_size + gap;
    }
}
