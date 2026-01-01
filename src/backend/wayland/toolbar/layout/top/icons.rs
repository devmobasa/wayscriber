use super::super::super::events::HitKind;
use super::super::super::format_binding_label;
use super::super::super::hit::HitRegion;
use super::super::spec::ToolbarLayoutSpec;
use super::shape_buttons;
use super::tool_buttons;
use crate::input::Tool;
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
    for (tool, label) in tool_buttons {
        if *tool == Tool::Rect {
            rect_x = Some(x);
        }
        if *tool == Tool::Ellipse {
            circle_end_x = Some(x + btn_size);
        }
        hits.push(HitRegion {
            rect: (x, y, btn_size, btn_size),
            event: ToolbarEvent::SelectTool(*tool),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                label,
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
    } else if let (Some(rect_x), Some(circle_end_x)) = (rect_x, circle_end_x) {
        fill_anchor = Some((rect_x, circle_end_x - rect_x));
    }

    if fill_tool_active
        && !(is_simple && snapshot.shape_picker_open)
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
                "Fill",
                snapshot.binding_hints.fill.as_deref(),
            )),
        });
    }

    hits.push(HitRegion {
        rect: (x, y, btn_size, btn_size),
        event: ToolbarEvent::EnterTextMode,
        kind: HitKind::Click,
        tooltip: Some(format_binding_label(
            "Text",
            snapshot.binding_hints.text.as_deref(),
        )),
    });
    x += btn_size + gap;

    hits.push(HitRegion {
        rect: (x, y, btn_size, btn_size),
        event: ToolbarEvent::EnterStickyNoteMode,
        kind: HitKind::Click,
        tooltip: Some(format_binding_label(
            "Note",
            snapshot.binding_hints.note.as_deref(),
        )),
    });
    x += btn_size + gap;

    if !is_simple {
        hits.push(HitRegion {
            rect: (x, y, btn_size, btn_size),
            event: ToolbarEvent::ClearCanvas,
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                "Clear",
                snapshot.binding_hints.clear.as_deref(),
            )),
        });
        x += btn_size + gap;

        hits.push(HitRegion {
            rect: (x, y, btn_size, btn_size),
            event: ToolbarEvent::ToggleAllHighlight(!snapshot.any_highlight_active),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                "Click highlight",
                snapshot.binding_hints.toggle_highlight.as_deref(),
            )),
        });
        x += btn_size + gap;
    }

    hits.push(HitRegion {
        rect: (x, y, ToolbarLayoutSpec::TOP_TOGGLE_WIDTH, btn_size),
        event: ToolbarEvent::ToggleIconMode(false),
        kind: HitKind::Click,
        tooltip: None,
    });

    if is_simple && snapshot.shape_picker_open {
        let shape_y = y + btn_size + ToolbarLayoutSpec::TOP_SHAPE_ROW_GAP;
        let mut shape_x = ToolbarLayoutSpec::TOP_START_X + ToolbarLayoutSpec::TOP_HANDLE_SIZE + gap;
        for (tool, label) in shape_buttons() {
            hits.push(HitRegion {
                rect: (shape_x, shape_y, btn_size, btn_size),
                event: ToolbarEvent::SelectTool(*tool),
                kind: HitKind::Click,
                tooltip: Some(format_binding_label(
                    label,
                    snapshot.binding_hints.for_tool(*tool),
                )),
            });
            shape_x += btn_size + gap;
        }
    }
}
