use super::super::super::events::HitKind;
use super::super::super::format_binding_label;
use super::super::super::hit::HitRegion;
use super::super::spec::ToolbarLayoutSpec;
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

    let (btn_w, btn_h) = spec.top_button_size();
    let y = spec.top_button_y(height);
    for tool in model::visible_top_tool_buttons(is_simple, snapshot) {
        let tooltip_label = tool_tooltip_label(tool);
        hits.push(HitRegion {
            rect: (x, y, btn_w, btn_h),
            event: ToolbarEvent::SelectTool(tool),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                tooltip_label,
                snapshot.binding_hints.for_tool(tool),
            )),
        });
        x += btn_w + gap;
    }

    if model::top_shape_picker_visible(snapshot) && is_simple {
        hits.push(HitRegion {
            rect: (x, y, btn_w, btn_h),
            event: ToolbarEvent::ToggleShapePicker(!snapshot.shape_picker_open),
            kind: HitKind::Click,
            tooltip: Some("Shapes".to_string()),
        });
        x += btn_w + gap;
    } else if model::top_shape_picker_visible(snapshot) {
        hits.push(HitRegion {
            rect: (x, y, btn_w, btn_h),
            event: ToolbarEvent::ToggleShapePicker(!snapshot.shape_picker_open),
            kind: HitKind::Click,
            tooltip: Some("Polygons".to_string()),
        });
        x += btn_w + gap;
    }

    if fill_tool_active && !snapshot.shape_picker_open && model::top_fill_visible(snapshot) {
        let fill_w = ToolbarLayoutSpec::TOP_TEXT_FILL_W;
        hits.push(HitRegion {
            rect: (x, y, fill_w, btn_h),
            event: ToolbarEvent::ToggleFill(!snapshot.fill_enabled),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                action_label(Action::ToggleFill),
                snapshot
                    .binding_hints
                    .binding_for_action(Action::ToggleFill),
            )),
        });
        x += fill_w + gap;
    }

    for button in model::visible_top_utility_buttons(snapshot, is_simple, false) {
        let (event, label) = match button {
            model::TopUtilityButton::Text => (ToolbarEvent::EnterTextMode, Action::EnterTextMode),
            model::TopUtilityButton::StickyNote => (
                ToolbarEvent::EnterStickyNoteMode,
                Action::EnterStickyNoteMode,
            ),
            model::TopUtilityButton::Screenshot => {
                (ToolbarEvent::CaptureScreenshot, Action::CaptureSelection)
            }
            model::TopUtilityButton::ClearCanvas => {
                (ToolbarEvent::ClearCanvas, Action::ClearCanvas)
            }
            model::TopUtilityButton::Highlight | model::TopUtilityButton::IconMode => continue,
        };
        hits.push(HitRegion {
            rect: (x, y, btn_w, btn_h),
            event,
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                action_label(label),
                snapshot.binding_hints.binding_for_action(label),
            )),
        });
        x += btn_w + gap;
    }

    if model::top_icon_mode_toggle_visible(snapshot) {
        hits.push(HitRegion {
            rect: (x, y, ToolbarLayoutSpec::TOP_TOGGLE_WIDTH, btn_h),
            event: ToolbarEvent::ToggleIconMode(true),
            kind: HitKind::Click,
            tooltip: None,
        });
    }

    if snapshot.shape_picker_open && model::top_shape_picker_visible(snapshot) {
        let mut shape_y = y + btn_h + ToolbarLayoutSpec::TOP_SHAPE_ROW_GAP;
        for row in model::visible_shape_picker_rows(snapshot, is_simple) {
            push_picker_hits(shape_y, btn_w, btn_h, gap, &row, snapshot, hits);
            shape_y += btn_h + ToolbarLayoutSpec::TOP_SHAPE_ROW_GAP;
        }
    }
}

fn push_picker_hits(
    shape_y: f64,
    btn_w: f64,
    btn_h: f64,
    gap: f64,
    tools: &[crate::input::Tool],
    snapshot: &ToolbarSnapshot,
    hits: &mut Vec<HitRegion>,
) {
    let mut shape_x = ToolbarLayoutSpec::TOP_START_X + ToolbarLayoutSpec::TOP_HANDLE_SIZE + gap;
    for tool in tools {
        if !model::tool_visible(snapshot, *tool) {
            continue;
        }
        let tooltip_label = tool_tooltip_label(*tool);
        hits.push(HitRegion {
            rect: (shape_x, shape_y, btn_w, btn_h),
            event: ToolbarEvent::SelectTool(*tool),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                tooltip_label,
                snapshot.binding_hints.for_tool(*tool),
            )),
        });
        shape_x += btn_w + gap;
    }
}
