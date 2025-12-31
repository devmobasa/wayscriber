use super::super::events::HitKind;
use super::super::format_binding_label;
use super::super::hit::HitRegion;
use super::spec::ToolbarLayoutSpec;
use crate::config::ToolbarLayoutMode;
use crate::input::Tool;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot};

/// Populate hit regions for the top toolbar.
#[allow(dead_code)]
pub fn build_top_hits(
    width: f64,
    height: f64,
    snapshot: &ToolbarSnapshot,
    hits: &mut Vec<HitRegion>,
) {
    let spec = ToolbarLayoutSpec::new(snapshot);
    let use_icons = spec.use_icons();
    let gap = ToolbarLayoutSpec::TOP_GAP;
    let mut x = ToolbarLayoutSpec::TOP_START_X;

    let is_simple = snapshot.layout_mode == ToolbarLayoutMode::Simple;
    let fill_tool_active = matches!(snapshot.tool_override, Some(Tool::Rect | Tool::Ellipse))
        || matches!(snapshot.active_tool, Tool::Rect | Tool::Ellipse);

    if use_icons {
        let (btn_size, _) = spec.top_button_size();
        let y = spec.top_button_y(height);
        let mut fill_anchor: Option<(f64, f64)> = None;
        let tool_buttons: &[(Tool, &str)] = if is_simple {
            &[
                (Tool::Select, "Select"),
                (Tool::Pen, "Pen"),
                (Tool::Marker, "Marker"),
                (Tool::Eraser, "Eraser"),
            ]
        } else {
            &[
                (Tool::Select, "Select"),
                (Tool::Pen, "Pen"),
                (Tool::Marker, "Marker"),
                (Tool::Eraser, "Eraser"),
                (Tool::Line, "Line"),
                (Tool::Rect, "Rect"),
                (Tool::Ellipse, "Circle"),
                (Tool::Arrow, "Arrow"),
            ]
        };

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
            let mut shape_x =
                ToolbarLayoutSpec::TOP_START_X + ToolbarLayoutSpec::TOP_HANDLE_SIZE + gap;
            let shapes: &[(Tool, &str)] = &[
                (Tool::Line, "Line"),
                (Tool::Rect, "Rect"),
                (Tool::Ellipse, "Circle"),
                (Tool::Arrow, "Arrow"),
            ];
            for (tool, label) in shapes {
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
    } else {
        let (btn_w, btn_h) = spec.top_button_size();
        let y = spec.top_button_y(height);
        let tool_buttons: &[(Tool, &str)] = if is_simple {
            &[
                (Tool::Select, "Select"),
                (Tool::Pen, "Pen"),
                (Tool::Marker, "Marker"),
                (Tool::Eraser, "Eraser"),
            ]
        } else {
            &[
                (Tool::Select, "Select"),
                (Tool::Pen, "Pen"),
                (Tool::Marker, "Marker"),
                (Tool::Eraser, "Eraser"),
                (Tool::Line, "Line"),
                (Tool::Rect, "Rect"),
                (Tool::Ellipse, "Circle"),
                (Tool::Arrow, "Arrow"),
            ]
        };
        for (tool, label) in tool_buttons {
            hits.push(HitRegion {
                rect: (x, y, btn_w, btn_h),
                event: ToolbarEvent::SelectTool(*tool),
                kind: HitKind::Click,
                tooltip: Some(format_binding_label(
                    label,
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
                    "Fill",
                    snapshot.binding_hints.fill.as_deref(),
                )),
            });
            x += fill_w + gap;
        }

        hits.push(HitRegion {
            rect: (x, y, btn_w, btn_h),
            event: ToolbarEvent::EnterTextMode,
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                "Text",
                snapshot.binding_hints.text.as_deref(),
            )),
        });
        x += btn_w + gap;

        hits.push(HitRegion {
            rect: (x, y, btn_w, btn_h),
            event: ToolbarEvent::EnterStickyNoteMode,
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                "Note",
                snapshot.binding_hints.note.as_deref(),
            )),
        });
        x += btn_w + gap;

        if !is_simple {
            hits.push(HitRegion {
                rect: (x, y, btn_w, btn_h),
                event: ToolbarEvent::ClearCanvas,
                kind: HitKind::Click,
                tooltip: Some(format_binding_label(
                    "Clear",
                    snapshot.binding_hints.clear.as_deref(),
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
            let mut shape_x =
                ToolbarLayoutSpec::TOP_START_X + ToolbarLayoutSpec::TOP_HANDLE_SIZE + gap;
            let shapes: &[(Tool, &str)] = &[
                (Tool::Line, "Line"),
                (Tool::Rect, "Rect"),
                (Tool::Ellipse, "Circle"),
                (Tool::Arrow, "Arrow"),
            ];
            for (tool, label) in shapes {
                hits.push(HitRegion {
                    rect: (shape_x, shape_y, btn_w, btn_h),
                    event: ToolbarEvent::SelectTool(*tool),
                    kind: HitKind::Click,
                    tooltip: Some(format_binding_label(
                        label,
                        snapshot.binding_hints.for_tool(*tool),
                    )),
                });
                shape_x += btn_w + gap;
            }
        }
    }

    let btn_size = ToolbarLayoutSpec::TOP_PIN_BUTTON_SIZE;
    let btn_y = spec.top_pin_button_y(height);

    let pin_x = spec.top_pin_x(width);
    hits.push(HitRegion {
        rect: (pin_x, btn_y, btn_size, btn_size),
        event: ToolbarEvent::PinTopToolbar(!snapshot.top_pinned),
        kind: HitKind::Click,
        tooltip: Some(if snapshot.top_pinned {
            "Unpin".to_string()
        } else {
            "Pin".to_string()
        }),
    });

    let close_x = spec.top_close_x(width);
    hits.push(HitRegion {
        rect: (close_x, btn_y, btn_size, btn_size),
        event: ToolbarEvent::CloseTopToolbar,
        kind: HitKind::Click,
        tooltip: Some("Close".to_string()),
    });
}
