use super::{ICON_TOGGLE_FONT_SIZE, TOP_LABEL_FONT_SIZE, TopStripLayout};
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::input::Tool;
use crate::ui::toolbar::ToolbarEvent;

use super::super::widgets::*;

pub(super) fn draw_text_strip(
    layout: &mut TopStripLayout,
    mut x: f64,
    handle_w: f64,
    is_simple: bool,
    current_shape_tool: Option<Tool>,
    fill_tool_active: bool,
) {
    let ctx = layout.ctx;
    let snapshot = layout.snapshot;
    let hover = layout.hover;
    let gap = layout.gap;

    let (btn_w, btn_h) = layout.spec.top_button_size();
    let y = layout.spec.top_button_y(layout.height);

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
        let tooltip = layout.tool_tooltip(*tool, label);
        layout.hits.push(HitRegion {
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
        layout.hits.push(HitRegion {
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
        layout.hits.push(HitRegion {
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
    layout.hits.push(HitRegion {
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
    layout.hits.push(HitRegion {
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
        layout.hits.push(HitRegion {
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
    layout.hits.push(HitRegion {
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
            let is_active = snapshot.active_tool == *tool || snapshot.tool_override == Some(*tool);
            let is_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, shape_x, shape_y, btn_w, btn_h))
                .unwrap_or(false);
            draw_button(ctx, shape_x, shape_y, btn_w, btn_h, is_active, is_hover);
            draw_label_center(ctx, shape_x, shape_y, btn_w, btn_h, label);
            let tooltip = layout.tool_tooltip(*tool, label);
            layout.hits.push(HitRegion {
                rect: (shape_x, shape_y, btn_w, btn_h),
                event: ToolbarEvent::SelectTool(*tool),
                kind: HitKind::Click,
                tooltip: Some(tooltip),
            });
            shape_x += btn_w + gap;
        }
    }
}
