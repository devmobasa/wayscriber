use super::{ICON_TOGGLE_FONT_SIZE, TOP_LABEL_FONT_SIZE, TopStripLayout};
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::input::Tool;
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;

use super::super::widgets::*;

pub(super) fn draw_icon_strip(
    layout: &mut TopStripLayout,
    mut x: f64,
    handle_w: f64,
    is_simple: bool,
    current_shape_tool: Option<Tool>,
    shape_icon_tool: Tool,
    fill_tool_active: bool,
) {
    let ctx = layout.ctx;
    let snapshot = layout.snapshot;
    let hover = layout.hover;
    let gap = layout.gap;

    let (btn_size, _) = layout.spec.top_button_size();
    let y = layout.spec.top_button_y(layout.height);
    let icon_size = ToolbarLayoutSpec::TOP_ICON_SIZE;
    let fill_h = ToolbarLayoutSpec::TOP_ICON_FILL_HEIGHT;

    type IconFn = fn(&cairo::Context, f64, f64, f64);

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

        let tooltip = layout.tool_tooltip(*tool, label);
        layout.hits.push(HitRegion {
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
        layout.hits.push(HitRegion {
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
        layout.hits.push(HitRegion {
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
    layout.hits.push(HitRegion {
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
    layout.hits.push(HitRegion {
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
        layout.hits.push(HitRegion {
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
        layout.hits.push(HitRegion {
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
    layout.hits.push(HitRegion {
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
            let is_active = snapshot.active_tool == *tool || snapshot.tool_override == Some(*tool);
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
            let tooltip = layout.tool_tooltip(*tool, label);
            layout.hits.push(HitRegion {
                rect: (shape_x, shape_y, btn_size, btn_size),
                event: ToolbarEvent::SelectTool(*tool),
                kind: HitKind::Click,
                tooltip: Some(tooltip),
            });
            shape_x += btn_size + gap;
        }
    }
}
