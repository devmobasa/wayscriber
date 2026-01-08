use super::super::super::events::HitKind;
use super::super::super::format_binding_label;
use super::super::super::hit::HitRegion;
use super::super::spec::ToolbarLayoutSpec;
use super::shape_buttons;
use super::tool_buttons;
use crate::config::{Action, action_label};
use crate::ui::toolbar::bindings::tool_tooltip_label;
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
    let tool_buttons = tool_buttons(is_simple);

    for tool in tool_buttons {
        let tooltip_label = tool_tooltip_label(*tool);
        hits.push(HitRegion {
            rect: (x, y, btn_w, btn_h),
            event: ToolbarEvent::SelectTool(*tool),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                tooltip_label,
                snapshot.binding_hints.for_tool(*tool),
            )),
        });
        x += btn_w + gap;
    }

    if is_simple {
        hits.push(HitRegion {
            rect: (x, y, btn_w, btn_h),
            event: ToolbarEvent::ToggleShapePicker(!snapshot.shape_picker_open),
            kind: HitKind::Click,
            tooltip: Some("Shapes".to_string()),
        });
        x += btn_w + gap;
    }

    if fill_tool_active {
        let fill_w = ToolbarLayoutSpec::TOP_TEXT_FILL_W;
        hits.push(HitRegion {
            rect: (x, y, fill_w, btn_h),
            event: ToolbarEvent::ToggleFill(!snapshot.fill_enabled),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                action_label(Action::ToggleFill),
                snapshot.binding_hints.binding_for_action(Action::ToggleFill),
            )),
        });
        x += fill_w + gap;
    }

    hits.push(HitRegion {
        rect: (x, y, btn_w, btn_h),
        event: ToolbarEvent::EnterTextMode,
        kind: HitKind::Click,
        tooltip: Some(format_binding_label(
            action_label(Action::EnterTextMode),
            snapshot.binding_hints.binding_for_action(Action::EnterTextMode),
        )),
    });
    x += btn_w + gap;

    hits.push(HitRegion {
        rect: (x, y, btn_w, btn_h),
        event: ToolbarEvent::EnterStickyNoteMode,
        kind: HitKind::Click,
        tooltip: Some(format_binding_label(
            action_label(Action::EnterStickyNoteMode),
            snapshot
                .binding_hints
                .binding_for_action(Action::EnterStickyNoteMode),
        )),
    });
    x += btn_w + gap;

    if !is_simple {
        hits.push(HitRegion {
            rect: (x, y, btn_w, btn_h),
            event: ToolbarEvent::ClearCanvas,
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                action_label(Action::ClearCanvas),
                snapshot.binding_hints.binding_for_action(Action::ClearCanvas),
            )),
        });
        x += btn_w + gap;
    }

    hits.push(HitRegion {
        rect: (x, y, ToolbarLayoutSpec::TOP_TOGGLE_WIDTH, btn_h),
        event: ToolbarEvent::ToggleIconMode(true),
        kind: HitKind::Click,
        tooltip: None,
    });

    if is_simple && snapshot.shape_picker_open {
        let shape_y = y + btn_h + ToolbarLayoutSpec::TOP_SHAPE_ROW_GAP;
        let mut shape_x = ToolbarLayoutSpec::TOP_START_X + ToolbarLayoutSpec::TOP_HANDLE_SIZE + gap;
        for tool in shape_buttons() {
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
}
