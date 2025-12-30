#![allow(clippy::too_many_arguments)]

use anyhow::Result;

use crate::backend::wayland::toolbar::format_binding_label;
use crate::input::Tool;
use crate::toolbar_icons;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot};

use super::super::events::HitKind;
use super::super::hit::HitRegion;
use super::super::layout::ToolbarLayoutSpec;
use super::widgets::*;

pub fn render_top_strip(
    ctx: &cairo::Context,
    width: f64,
    height: f64,
    snapshot: &ToolbarSnapshot,
    hits: &mut Vec<HitRegion>,
    hover: Option<(f64, f64)>,
) -> Result<()> {
    draw_panel_background(ctx, width, height);

    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    const TOP_LABEL_FONT_SIZE: f64 = 14.0;
    const ICON_TOGGLE_FONT_SIZE: f64 = 12.0;
    ctx.set_font_size(TOP_LABEL_FONT_SIZE);

    let spec = ToolbarLayoutSpec::new(snapshot);
    let use_icons = snapshot.use_icons;
    let gap = ToolbarLayoutSpec::TOP_GAP;
    let mut x = ToolbarLayoutSpec::TOP_START_X;
    let tool_tooltip = |tool: Tool, label: &str| {
        let default_hint = match tool {
            Tool::Line => Some("Shift+Drag"),
            Tool::Rect => Some("Ctrl+Drag"),
            Tool::Ellipse => Some("Tab+Drag"),
            Tool::Arrow => Some("Ctrl+Shift+Drag"),
            _ => None,
        };
        let binding = match (snapshot.binding_hints.for_tool(tool), default_hint) {
            (Some(binding), Some(fallback)) => Some(format!("{}, {}", binding, fallback)),
            (Some(binding), None) => Some(binding.to_string()),
            (None, Some(fallback)) => Some(fallback.to_string()),
            (None, None) => None,
        };
        format_binding_label(label, binding.as_deref())
    };

    // Drag handle (left)
    let handle_w = ToolbarLayoutSpec::TOP_HANDLE_SIZE;
    let handle_h = ToolbarLayoutSpec::TOP_HANDLE_SIZE;
    let handle_y = ToolbarLayoutSpec::TOP_HANDLE_Y;
    let handle_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, handle_y, handle_w, handle_h))
        .unwrap_or(false);
    draw_drag_handle(ctx, x, handle_y, handle_w, handle_h, handle_hover);
    hits.push(HitRegion {
        rect: (x, handle_y, handle_w, handle_h),
        event: ToolbarEvent::MoveTopToolbar { x: 0.0, y: 0.0 },
        kind: HitKind::DragMoveTop,
        tooltip: Some("Drag toolbar".to_string()),
    });
    x += handle_w + gap;

    type IconFn = fn(&cairo::Context, f64, f64, f64);
    let is_simple = snapshot.layout_mode == crate::config::ToolbarLayoutMode::Simple;
    let current_shape_tool = match snapshot.tool_override {
        Some(Tool::Line) => Some(Tool::Line),
        Some(Tool::Rect) => Some(Tool::Rect),
        Some(Tool::Ellipse) => Some(Tool::Ellipse),
        Some(Tool::Arrow) => Some(Tool::Arrow),
        _ => match snapshot.active_tool {
            Tool::Line => Some(Tool::Line),
            Tool::Rect => Some(Tool::Rect),
            Tool::Ellipse => Some(Tool::Ellipse),
            Tool::Arrow => Some(Tool::Arrow),
            _ => None,
        },
    };
    let shape_icon_tool = current_shape_tool.unwrap_or(Tool::Rect);
    let fill_tool_active = matches!(snapshot.tool_override, Some(Tool::Rect | Tool::Ellipse))
        || matches!(snapshot.active_tool, Tool::Rect | Tool::Ellipse);

    if use_icons {
        let (btn_size, _) = spec.top_button_size();
        let y = spec.top_button_y(height);
        let icon_size = ToolbarLayoutSpec::TOP_ICON_SIZE;
        let fill_h = ToolbarLayoutSpec::TOP_ICON_FILL_HEIGHT;

        let mut fill_anchor: Option<(f64, f64)> = None;

        let tool_buttons: &[(Tool, IconFn, &str)] = if is_simple {
            &[
                (
                    Tool::Select,
                    toolbar_icons::draw_icon_select as IconFn,
                    "Select",
                ),
                (Tool::Pen, toolbar_icons::draw_icon_pen as IconFn, "Pen"),
                (
                    Tool::Marker,
                    toolbar_icons::draw_icon_marker as IconFn,
                    "Marker",
                ),
                (
                    Tool::Eraser,
                    toolbar_icons::draw_icon_eraser as IconFn,
                    "Eraser",
                ),
            ]
        } else {
            &[
                (
                    Tool::Select,
                    toolbar_icons::draw_icon_select as IconFn,
                    "Select",
                ),
                (Tool::Pen, toolbar_icons::draw_icon_pen as IconFn, "Pen"),
                (
                    Tool::Marker,
                    toolbar_icons::draw_icon_marker as IconFn,
                    "Marker",
                ),
                (
                    Tool::Eraser,
                    toolbar_icons::draw_icon_eraser as IconFn,
                    "Eraser",
                ),
                (Tool::Line, toolbar_icons::draw_icon_line as IconFn, "Line"),
                (Tool::Rect, toolbar_icons::draw_icon_rect as IconFn, "Rect"),
                (
                    Tool::Ellipse,
                    toolbar_icons::draw_icon_circle as IconFn,
                    "Circle",
                ),
                (
                    Tool::Arrow,
                    toolbar_icons::draw_icon_arrow as IconFn,
                    "Arrow",
                ),
            ]
        };

        let mut rect_x = None;
        let mut circle_end_x = None;
        for (tool, icon_fn, label) in tool_buttons {
            if *tool == Tool::Rect {
                rect_x = Some(x);
            }
            if *tool == Tool::Ellipse {
                circle_end_x = Some(x + btn_size);
            }

            let is_active = snapshot.active_tool == *tool || snapshot.tool_override == Some(*tool);
            let is_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_size, btn_size))
                .unwrap_or(false);
            draw_button(ctx, x, y, btn_size, btn_size, is_active, is_hover);

            set_icon_color(ctx, is_hover);
            let icon_x = x + (btn_size - icon_size) / 2.0;
            let icon_y = y + (btn_size - icon_size) / 2.0;
            icon_fn(ctx, icon_x, icon_y, icon_size);

            let tooltip = tool_tooltip(*tool, label);
            hits.push(HitRegion {
                rect: (x, y, btn_size, btn_size),
                event: ToolbarEvent::SelectTool(*tool),
                kind: HitKind::Click,
                tooltip: Some(tooltip),
            });
            x += btn_size + gap;
        }

        if is_simple {
            let shapes_active = snapshot.shape_picker_open || current_shape_tool.is_some();
            let shapes_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_size, btn_size))
                .unwrap_or(false);
            draw_button(ctx, x, y, btn_size, btn_size, shapes_active, shapes_hover);
            set_icon_color(ctx, shapes_hover);
            let icon_x = x + (btn_size - icon_size) / 2.0;
            let icon_y = y + (btn_size - icon_size) / 2.0;
            match shape_icon_tool {
                Tool::Line => toolbar_icons::draw_icon_line(ctx, icon_x, icon_y, icon_size),
                Tool::Rect => toolbar_icons::draw_icon_rect(ctx, icon_x, icon_y, icon_size),
                Tool::Ellipse => toolbar_icons::draw_icon_circle(ctx, icon_x, icon_y, icon_size),
                Tool::Arrow => toolbar_icons::draw_icon_arrow(ctx, icon_x, icon_y, icon_size),
                _ => toolbar_icons::draw_icon_rect(ctx, icon_x, icon_y, icon_size),
            }
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
            let fill_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, fill_x, fill_y, fill_w, fill_h))
                .unwrap_or(false);
            draw_mini_checkbox(
                ctx,
                fill_x,
                fill_y,
                fill_w,
                fill_h,
                snapshot.fill_enabled,
                fill_hover,
                "Fill",
            );
            hits.push(HitRegion {
                rect: (fill_x, fill_y, fill_w, fill_h),
                event: ToolbarEvent::ToggleFill(!snapshot.fill_enabled),
                kind: HitKind::Click,
                tooltip: Some(format_binding_label(
                    "Fill",
                    snapshot.binding_hints.fill.as_deref(),
                )),
            });
        }

        let is_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_size, btn_size))
            .unwrap_or(false);
        draw_button(
            ctx,
            x,
            y,
            btn_size,
            btn_size,
            snapshot.text_active,
            is_hover,
        );
        set_icon_color(ctx, is_hover);
        toolbar_icons::draw_icon_text(
            ctx,
            x + (btn_size - icon_size) / 2.0,
            y + (btn_size - icon_size) / 2.0,
            icon_size,
        );
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

        let note_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_size, btn_size))
            .unwrap_or(false);
        draw_button(
            ctx,
            x,
            y,
            btn_size,
            btn_size,
            snapshot.note_active,
            note_hover,
        );
        set_icon_color(ctx, note_hover);
        toolbar_icons::draw_icon_note(
            ctx,
            x + (btn_size - icon_size) / 2.0,
            y + (btn_size - icon_size) / 2.0,
            icon_size,
        );
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
            let clear_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_size, btn_size))
                .unwrap_or(false);
            draw_button(ctx, x, y, btn_size, btn_size, false, clear_hover);
            set_icon_color(ctx, clear_hover);
            toolbar_icons::draw_icon_clear(
                ctx,
                x + (btn_size - icon_size) / 2.0,
                y + (btn_size - icon_size) / 2.0,
                icon_size,
            );
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

            let highlight_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_size, btn_size))
                .unwrap_or(false);
            draw_button(
                ctx,
                x,
                y,
                btn_size,
                btn_size,
                snapshot.any_highlight_active,
                highlight_hover,
            );
            set_icon_color(ctx, highlight_hover);
            toolbar_icons::draw_icon_highlight(
                ctx,
                x + (btn_size - icon_size) / 2.0,
                y + (btn_size - icon_size) / 2.0,
                icon_size,
            );
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

        let icons_w = ToolbarLayoutSpec::TOP_TOGGLE_WIDTH;
        let icons_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, icons_w, btn_size))
            .unwrap_or(false);
        ctx.set_font_size(ICON_TOGGLE_FONT_SIZE);
        draw_checkbox(ctx, x, y, icons_w, btn_size, true, icons_hover, "Icons");
        ctx.set_font_size(TOP_LABEL_FONT_SIZE);
        hits.push(HitRegion {
            rect: (x, y, icons_w, btn_size),
            event: ToolbarEvent::ToggleIconMode(false),
            kind: HitKind::Click,
            tooltip: None,
        });

        if is_simple && snapshot.shape_picker_open {
            let shape_y = y + btn_size + ToolbarLayoutSpec::TOP_SHAPE_ROW_GAP;
            let mut shape_x = ToolbarLayoutSpec::TOP_START_X + handle_w + gap;
            let shapes: &[(Tool, IconFn, &str)] = &[
                (Tool::Line, toolbar_icons::draw_icon_line as IconFn, "Line"),
                (Tool::Rect, toolbar_icons::draw_icon_rect as IconFn, "Rect"),
                (
                    Tool::Ellipse,
                    toolbar_icons::draw_icon_circle as IconFn,
                    "Circle",
                ),
                (
                    Tool::Arrow,
                    toolbar_icons::draw_icon_arrow as IconFn,
                    "Arrow",
                ),
            ];
            for (tool, icon_fn, label) in shapes {
                let is_active =
                    snapshot.active_tool == *tool || snapshot.tool_override == Some(*tool);
                let is_hover = hover
                    .map(|(hx, hy)| point_in_rect(hx, hy, shape_x, shape_y, btn_size, btn_size))
                    .unwrap_or(false);
                draw_button(
                    ctx, shape_x, shape_y, btn_size, btn_size, is_active, is_hover,
                );
                set_icon_color(ctx, is_hover);
                let icon_x = shape_x + (btn_size - icon_size) / 2.0;
                let icon_y = shape_y + (btn_size - icon_size) / 2.0;
                icon_fn(ctx, icon_x, icon_y, icon_size);
                let tooltip = tool_tooltip(*tool, label);
                hits.push(HitRegion {
                    rect: (shape_x, shape_y, btn_size, btn_size),
                    event: ToolbarEvent::SelectTool(*tool),
                    kind: HitKind::Click,
                    tooltip: Some(tooltip),
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
            let is_active = snapshot.active_tool == *tool || snapshot.tool_override == Some(*tool);
            let is_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_w, btn_h))
                .unwrap_or(false);
            draw_button(ctx, x, y, btn_w, btn_h, is_active, is_hover);
            draw_label_center(ctx, x, y, btn_w, btn_h, label);
            let tooltip = tool_tooltip(*tool, label);
            hits.push(HitRegion {
                rect: (x, y, btn_w, btn_h),
                event: ToolbarEvent::SelectTool(*tool),
                kind: HitKind::Click,
                tooltip: Some(tooltip),
            });
            x += btn_w + gap;
        }

        if is_simple {
            let shapes_active = snapshot.shape_picker_open || current_shape_tool.is_some();
            let shapes_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_w, btn_h))
                .unwrap_or(false);
            draw_button(ctx, x, y, btn_w, btn_h, shapes_active, shapes_hover);
            draw_label_center(ctx, x, y, btn_w, btn_h, "Shapes");
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
            let fill_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, x, y, fill_w, btn_h))
                .unwrap_or(false);
            draw_checkbox(
                ctx,
                x,
                y,
                fill_w,
                btn_h,
                snapshot.fill_enabled,
                fill_hover,
                "Fill",
            );
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

        let is_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_w, btn_h))
            .unwrap_or(false);
        draw_button(ctx, x, y, btn_w, btn_h, snapshot.text_active, is_hover);
        draw_label_center(ctx, x, y, btn_w, btn_h, "Text");
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

        let note_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_w, btn_h))
            .unwrap_or(false);
        draw_button(ctx, x, y, btn_w, btn_h, snapshot.note_active, note_hover);
        draw_label_center(ctx, x, y, btn_w, btn_h, "Note");
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
            let clear_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_w, btn_h))
                .unwrap_or(false);
            draw_button(ctx, x, y, btn_w, btn_h, false, clear_hover);
            draw_label_center(ctx, x, y, btn_w, btn_h, "Clear");
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

        let icons_w = ToolbarLayoutSpec::TOP_TOGGLE_WIDTH;
        let icons_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, icons_w, btn_h))
            .unwrap_or(false);
        ctx.set_font_size(ICON_TOGGLE_FONT_SIZE);
        draw_checkbox(ctx, x, y, icons_w, btn_h, false, icons_hover, "Icons");
        ctx.set_font_size(TOP_LABEL_FONT_SIZE);
        hits.push(HitRegion {
            rect: (x, y, icons_w, btn_h),
            event: ToolbarEvent::ToggleIconMode(true),
            kind: HitKind::Click,
            tooltip: None,
        });

        if is_simple && snapshot.shape_picker_open {
            let shape_y = y + btn_h + ToolbarLayoutSpec::TOP_SHAPE_ROW_GAP;
            let mut shape_x = ToolbarLayoutSpec::TOP_START_X + handle_w + gap;
            let shapes: &[(Tool, &str)] = &[
                (Tool::Line, "Line"),
                (Tool::Rect, "Rect"),
                (Tool::Ellipse, "Circle"),
                (Tool::Arrow, "Arrow"),
            ];
            for (tool, label) in shapes {
                let is_active =
                    snapshot.active_tool == *tool || snapshot.tool_override == Some(*tool);
                let is_hover = hover
                    .map(|(hx, hy)| point_in_rect(hx, hy, shape_x, shape_y, btn_w, btn_h))
                    .unwrap_or(false);
                draw_button(ctx, shape_x, shape_y, btn_w, btn_h, is_active, is_hover);
                draw_label_center(ctx, shape_x, shape_y, btn_w, btn_h, label);
                let tooltip = tool_tooltip(*tool, label);
                hits.push(HitRegion {
                    rect: (shape_x, shape_y, btn_w, btn_h),
                    event: ToolbarEvent::SelectTool(*tool),
                    kind: HitKind::Click,
                    tooltip: Some(tooltip),
                });
                shape_x += btn_w + gap;
            }
        }
    }

    let btn_size = ToolbarLayoutSpec::TOP_PIN_BUTTON_SIZE;
    let btn_y = spec.top_pin_button_y(height);
    let pin_x = spec.top_pin_x(width);
    let pin_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, pin_x, btn_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_pin_button(ctx, pin_x, btn_y, btn_size, snapshot.top_pinned, pin_hover);
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
    let close_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, close_x, btn_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_close_button(ctx, close_x, btn_y, btn_size, close_hover);
    hits.push(HitRegion {
        rect: (close_x, btn_y, btn_size, btn_size),
        event: ToolbarEvent::CloseTopToolbar,
        kind: HitKind::Click,
        tooltip: Some("Close".to_string()),
    });

    draw_tooltip(ctx, hits, hover, width, false);
    Ok(())
}
