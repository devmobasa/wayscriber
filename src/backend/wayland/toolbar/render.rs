#![allow(clippy::too_many_arguments)]

use anyhow::Result;

use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar_icons;
use crate::draw::{BLACK, BLUE, Color, FontDescriptor, GREEN, ORANGE, PINK, RED, WHITE, YELLOW};
use crate::input::Tool;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot};

use super::events::{HitKind, delay_secs_from_t, delay_t_from_ms};
use super::hit::HitRegion;

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
    ctx.set_font_size(14.0);

    let use_icons = snapshot.use_icons;
    let gap = 8.0;
    let mut x = 16.0;

    // Drag handle (left)
    let handle_w = 18.0;
    let handle_h = 18.0;
    let handle_y = 10.0;
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
    let buttons: &[(Tool, IconFn, &str)] = &[
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
    ];

    if use_icons {
        let btn_size = 42.0;
        let y = 6.0;
        let icon_size = 26.0;

        let mut rect_x = 0.0;
        let mut circle_end_x = 0.0;

        for (tool, icon_fn, label) in buttons {
            if *tool == Tool::Rect {
                rect_x = x;
            }
            if *tool == Tool::Ellipse {
                circle_end_x = x + btn_size;
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

            let tooltip = format_binding_label(label, snapshot.binding_hints.for_tool(*tool));
            hits.push(HitRegion {
                rect: (x, y, btn_size, btn_size),
                event: ToolbarEvent::SelectTool(*tool),
                kind: HitKind::Click,
                tooltip: Some(tooltip),
            });
            x += btn_size + gap;
        }

        let fill_y = y + btn_size + 2.0;
        let fill_w = circle_end_x - rect_x;
        let fill_h = 18.0;
        let fill_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, rect_x, fill_y, fill_w, fill_h))
            .unwrap_or(false);
        draw_mini_checkbox(
            ctx,
            rect_x,
            fill_y,
            fill_w,
            fill_h,
            snapshot.fill_enabled,
            fill_hover,
            "Fill",
        );
        hits.push(HitRegion {
            rect: (rect_x, fill_y, fill_w, fill_h),
            event: ToolbarEvent::ToggleFill(!snapshot.fill_enabled),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                "Fill",
                snapshot.binding_hints.fill.as_deref(),
            )),
        });

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

        let icons_w = 70.0;
        let icons_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, icons_w, btn_size))
            .unwrap_or(false);
        draw_checkbox(ctx, x, y, icons_w, btn_size, true, icons_hover, "Icons");
        hits.push(HitRegion {
            rect: (x, y, icons_w, btn_size),
            event: ToolbarEvent::ToggleIconMode(false),
            kind: HitKind::Click,
            tooltip: None,
        });
    } else {
        let btn_w = 60.0;
        let btn_h = 36.0;
        let y = (height - btn_h) / 2.0;

        for (tool, _icon_fn, label) in buttons {
            let is_active = snapshot.active_tool == *tool || snapshot.tool_override == Some(*tool);
            let is_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_w, btn_h))
                .unwrap_or(false);
            draw_button(ctx, x, y, btn_w, btn_h, is_active, is_hover);
            draw_label_center(ctx, x, y, btn_w, btn_h, label);
            let tooltip = format_binding_label(label, snapshot.binding_hints.for_tool(*tool));
            hits.push(HitRegion {
                rect: (x, y, btn_w, btn_h),
                event: ToolbarEvent::SelectTool(*tool),
                kind: HitKind::Click,
                tooltip: Some(tooltip),
            });
            x += btn_w + gap;
        }

        let fill_w = 64.0;
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

        let is_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_w, btn_h))
            .unwrap_or(false);
        draw_button(ctx, x, y, btn_w, btn_h, snapshot.text_active, is_hover);
        draw_label_center(ctx, x, y, btn_w, btn_h, "Text");
        hits.push(HitRegion {
            rect: (x, y, btn_w, btn_h),
            event: ToolbarEvent::EnterTextMode,
            kind: HitKind::Click,
            tooltip: None,
        });
        x += btn_w + gap;

        let icons_w = 70.0;
        let icons_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, icons_w, btn_h))
            .unwrap_or(false);
        draw_checkbox(ctx, x, y, icons_w, btn_h, false, icons_hover, "Icons");
        hits.push(HitRegion {
            rect: (x, y, icons_w, btn_h),
            event: ToolbarEvent::ToggleIconMode(true),
            kind: HitKind::Click,
            tooltip: None,
        });
    }

    let btn_size = 24.0;
    let btn_gap = 6.0;
    let btn_y = if use_icons {
        15.0
    } else {
        (height - btn_size) / 2.0
    };

    let pin_x = width - btn_size * 2.0 - btn_gap - 12.0;
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

    let close_x = width - btn_size - 12.0;
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

pub fn render_side_palette(
    ctx: &cairo::Context,
    width: f64,
    _height: f64,
    snapshot: &ToolbarSnapshot,
    hits: &mut Vec<HitRegion>,
    hover: Option<(f64, f64)>,
) -> Result<()> {
    draw_panel_background(ctx, width, _height);

    let mut y = 12.0;
    let x = 16.0;
    let use_icons = snapshot.use_icons;
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(13.0);

    let btn_size = 22.0;
    let mut header_y = y;
    let handle_w = 18.0;
    let handle_h = 18.0;

    // Place handle above the header row to avoid widening the palette.
    let handle_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, header_y, handle_w, handle_h))
        .unwrap_or(false);
    draw_drag_handle(ctx, x, header_y, handle_w, handle_h, handle_hover);
    hits.push(HitRegion {
        rect: (x, header_y, handle_w, handle_h),
        event: ToolbarEvent::MoveSideToolbar { x: 0.0, y: 0.0 },
        kind: HitKind::DragMoveSide,
        tooltip: Some("Drag toolbar".to_string()),
    });
    header_y += handle_h + 6.0;

    let icons_w = 58.0;
    let icons_h = btn_size;
    let icons_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, header_y, icons_w, icons_h))
        .unwrap_or(false);
    draw_checkbox(
        ctx,
        x,
        header_y,
        icons_w,
        icons_h,
        use_icons,
        icons_hover,
        "Icons",
    );
    hits.push(HitRegion {
        rect: (x, header_y, icons_w, icons_h),
        event: ToolbarEvent::ToggleIconMode(!use_icons),
        kind: HitKind::Click,
        tooltip: None,
    });

    let pin_x = width - btn_size * 2.0 - 20.0;
    let pin_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, pin_x, header_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_pin_button(
        ctx,
        pin_x,
        header_y,
        btn_size,
        snapshot.side_pinned,
        pin_hover,
    );
    hits.push(HitRegion {
        rect: (pin_x, header_y, btn_size, btn_size),
        event: ToolbarEvent::PinSideToolbar(!snapshot.side_pinned),
        kind: HitKind::Click,
        tooltip: Some(if snapshot.side_pinned {
            "Unpin".to_string()
        } else {
            "Pin".to_string()
        }),
    });

    let close_x = width - btn_size - 12.0;
    let close_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, close_x, header_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_close_button(ctx, close_x, header_y, btn_size, close_hover);
    hits.push(HitRegion {
        rect: (close_x, header_y, btn_size, btn_size),
        event: ToolbarEvent::CloseSideToolbar,
        kind: HitKind::Click,
        tooltip: Some("Close".to_string()),
    });

    y = header_y + btn_size + 12.0;

    let card_x = x - 6.0;
    let card_w = width - 2.0 * x + 12.0;
    let section_gap = 12.0;

    let basic_colors: &[(Color, &str)] = &[
        (RED, "Red"),
        (GREEN, "Green"),
        (BLUE, "Blue"),
        (YELLOW, "Yellow"),
        (WHITE, "White"),
        (BLACK, "Black"),
    ];
    let extended_colors: &[(Color, &str)] = &[
        (ORANGE, "Orange"),
        (PINK, "Pink"),
        (
            Color {
                r: 0.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            "Cyan",
        ),
        (
            Color {
                r: 0.6,
                g: 0.4,
                b: 0.8,
                a: 1.0,
            },
            "Purple",
        ),
        (
            Color {
                r: 0.4,
                g: 0.4,
                b: 0.4,
                a: 1.0,
            },
            "Gray",
        ),
    ];

    let swatch = 24.0;
    let swatch_gap = 6.0;
    let basic_rows = 1;
    let extended_rows = if snapshot.show_more_colors { 1 } else { 0 };
    let picker_h = 24.0;
    let colors_card_h =
        28.0 + picker_h + 8.0 + (swatch + swatch_gap) * (basic_rows + extended_rows) as f64;

    draw_group_card(ctx, card_x, y, card_w, colors_card_h);
    draw_section_label(ctx, x, y + 12.0, "Colors");

    let picker_y = y + 24.0;
    let picker_w = card_w - 12.0;
    draw_color_picker(ctx, x, picker_y, picker_w, picker_h);
    hits.push(HitRegion {
        rect: (x, picker_y, picker_w, picker_h),
        event: ToolbarEvent::SetColor(snapshot.color),
        kind: HitKind::PickColor {
            x,
            y: picker_y,
            w: picker_w,
            h: picker_h + if snapshot.show_more_colors { 30.0 } else { 0.0 },
        },
        tooltip: None,
    });

    let mut cx = x;
    let mut row_y = picker_y + picker_h + 8.0;
    for (color, _name) in basic_colors {
        draw_swatch(ctx, cx, row_y, swatch, *color, *color == snapshot.color);
        hits.push(HitRegion {
            rect: (cx, row_y, swatch, swatch),
            event: ToolbarEvent::SetColor(*color),
            kind: HitKind::Click,
            tooltip: None,
        });
        cx += swatch + swatch_gap;
    }

    if !snapshot.show_more_colors {
        let plus_btn_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, cx, row_y, swatch, swatch))
            .unwrap_or(false);
        draw_button(ctx, cx, row_y, swatch, swatch, false, plus_btn_hover);
        set_icon_color(ctx, plus_btn_hover);
        toolbar_icons::draw_icon_plus(
            ctx,
            cx + (swatch - 14.0) / 2.0,
            row_y + (swatch - 14.0) / 2.0,
            14.0,
        );
        hits.push(HitRegion {
            rect: (cx, row_y, swatch, swatch),
            event: ToolbarEvent::ToggleMoreColors(true),
            kind: HitKind::Click,
            tooltip: Some("More colors".to_string()),
        });
    }
    row_y += swatch + swatch_gap;

    if snapshot.show_more_colors {
        cx = x;
        for (color, _name) in extended_colors {
            draw_swatch(ctx, cx, row_y, swatch, *color, *color == snapshot.color);
            hits.push(HitRegion {
                rect: (cx, row_y, swatch, swatch),
                event: ToolbarEvent::SetColor(*color),
                kind: HitKind::Click,
                tooltip: None,
            });
            cx += swatch + swatch_gap;
        }

        let minus_btn_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, cx, row_y, swatch, swatch))
            .unwrap_or(false);
        draw_button(ctx, cx, row_y, swatch, swatch, false, minus_btn_hover);
        set_icon_color(ctx, minus_btn_hover);
        toolbar_icons::draw_icon_minus(
            ctx,
            cx + (swatch - 14.0) / 2.0,
            row_y + (swatch - 14.0) / 2.0,
            14.0,
        );
        hits.push(HitRegion {
            rect: (cx, row_y, swatch, swatch),
            event: ToolbarEvent::ToggleMoreColors(false),
            kind: HitKind::Click,
            tooltip: Some("Hide colors".to_string()),
        });
    }

    y += colors_card_h + section_gap;

    let slider_card_h = 52.0;
    draw_group_card(ctx, card_x, y, card_w, slider_card_h);
    let thickness_label = if snapshot.thickness_targets_eraser {
        "Eraser size"
    } else {
        "Thickness"
    };
    draw_section_label(ctx, x, y + 12.0, thickness_label);

    let btn_size = 24.0;
    let nudge_icon_size = 14.0;
    let value_w = 40.0;
    let thickness_slider_row_y = y + 26.0;
    let track_h = 8.0;
    let knob_r = 7.0;
    let (min_thick, max_thick, nudge_step) = (1.0, 50.0, 1.0);

    let minus_x = x;
    draw_button(
        ctx,
        minus_x,
        thickness_slider_row_y,
        btn_size,
        btn_size,
        false,
        false,
    );
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    toolbar_icons::draw_icon_minus(
        ctx,
        minus_x + (btn_size - nudge_icon_size) / 2.0,
        thickness_slider_row_y + (btn_size - nudge_icon_size) / 2.0,
        nudge_icon_size,
    );
    hits.push(HitRegion {
        rect: (minus_x, thickness_slider_row_y, btn_size, btn_size),
        event: ToolbarEvent::NudgeThickness(-nudge_step),
        kind: HitKind::Click,
        tooltip: None,
    });

    let plus_x = width - x - btn_size - value_w - 4.0;
    draw_button(
        ctx,
        plus_x,
        thickness_slider_row_y,
        btn_size,
        btn_size,
        false,
        false,
    );
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    toolbar_icons::draw_icon_plus(
        ctx,
        plus_x + (btn_size - nudge_icon_size) / 2.0,
        thickness_slider_row_y + (btn_size - nudge_icon_size) / 2.0,
        nudge_icon_size,
    );
    hits.push(HitRegion {
        rect: (plus_x, thickness_slider_row_y, btn_size, btn_size),
        event: ToolbarEvent::NudgeThickness(nudge_step),
        kind: HitKind::Click,
        tooltip: None,
    });

    let track_x = minus_x + btn_size + 6.0;
    let track_w = plus_x - track_x - 6.0;
    let thickness_track_y = thickness_slider_row_y + (btn_size - track_h) / 2.0;
    let t = ((snapshot.thickness - min_thick) / (max_thick - min_thick)).clamp(0.0, 1.0);
    let knob_x = track_x + t * (track_w - knob_r * 2.0) + knob_r;

    ctx.set_source_rgba(0.5, 0.5, 0.6, 0.6);
    draw_round_rect(ctx, track_x, thickness_track_y, track_w, track_h, 4.0);
    let _ = ctx.fill();
    ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
    ctx.arc(
        knob_x,
        thickness_track_y + track_h / 2.0,
        knob_r,
        0.0,
        std::f64::consts::PI * 2.0,
    );
    let _ = ctx.fill();

    hits.push(HitRegion {
        rect: (track_x, thickness_track_y - 6.0, track_w, track_h + 12.0),
        event: ToolbarEvent::SetThickness(snapshot.thickness),
        kind: HitKind::DragSetThickness {
            min: min_thick,
            max: max_thick,
        },
        tooltip: None,
    });

    let thickness_text = format!("{:.0}px", snapshot.thickness);
    let value_x = width - x - value_w;
    draw_label_center(
        ctx,
        value_x,
        thickness_slider_row_y,
        value_w,
        btn_size,
        &thickness_text,
    );
    y += slider_card_h + section_gap;

    let show_marker_opacity =
        snapshot.show_marker_opacity_section || snapshot.thickness_targets_marker;
    if show_marker_opacity {
        let marker_slider_row_y = y + 26.0;
        draw_group_card(ctx, card_x, y, card_w, slider_card_h);
        draw_section_label(ctx, x, y + 12.0, "Marker opacity");

        let minus_x = x;
        draw_button(
            ctx,
            minus_x,
            marker_slider_row_y,
            btn_size,
            btn_size,
            false,
            false,
        );
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        toolbar_icons::draw_icon_minus(
            ctx,
            minus_x + (btn_size - nudge_icon_size) / 2.0,
            marker_slider_row_y + (btn_size - nudge_icon_size) / 2.0,
            nudge_icon_size,
        );
        hits.push(HitRegion {
            rect: (minus_x, marker_slider_row_y, btn_size, btn_size),
            event: ToolbarEvent::NudgeMarkerOpacity(-0.05),
            kind: HitKind::Click,
            tooltip: None,
        });

        let plus_x = width - x - btn_size - value_w - 4.0;
        draw_button(
            ctx,
            plus_x,
            marker_slider_row_y,
            btn_size,
            btn_size,
            false,
            false,
        );
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        toolbar_icons::draw_icon_plus(
            ctx,
            plus_x + (btn_size - nudge_icon_size) / 2.0,
            marker_slider_row_y + (btn_size - nudge_icon_size) / 2.0,
            nudge_icon_size,
        );
        hits.push(HitRegion {
            rect: (plus_x, marker_slider_row_y, btn_size, btn_size),
            event: ToolbarEvent::NudgeMarkerOpacity(0.05),
            kind: HitKind::Click,
            tooltip: None,
        });

        let track_x = minus_x + btn_size + 6.0;
        let track_w = plus_x - track_x - 6.0;
        let marker_track_y = marker_slider_row_y + (btn_size - track_h) / 2.0;
        let min_opacity = 0.05;
        let max_opacity = 0.9;
        let t =
            ((snapshot.marker_opacity - min_opacity) / (max_opacity - min_opacity)).clamp(0.0, 1.0);
        let knob_x = track_x + t * (track_w - knob_r * 2.0) + knob_r;

        ctx.set_source_rgba(0.5, 0.5, 0.6, 0.6);
        draw_round_rect(ctx, track_x, marker_track_y, track_w, track_h, 4.0);
        let _ = ctx.fill();
        ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
        ctx.arc(
            knob_x,
            marker_track_y + track_h / 2.0,
            knob_r,
            0.0,
            std::f64::consts::PI * 2.0,
        );
        let _ = ctx.fill();

        hits.push(HitRegion {
            rect: (track_x, marker_track_y - 6.0, track_w, track_h + 12.0),
            event: ToolbarEvent::SetMarkerOpacity(snapshot.marker_opacity),
            kind: HitKind::DragSetMarkerOpacity {
                min: min_opacity,
                max: max_opacity,
            },
            tooltip: None,
        });

        let opacity_text = format!("{:.0}%", snapshot.marker_opacity * 100.0);
        draw_label_center(
            ctx,
            value_x,
            marker_slider_row_y,
            value_w,
            btn_size,
            &opacity_text,
        );

        y += slider_card_h + section_gap;
    }

    draw_group_card(ctx, card_x, y, card_w, slider_card_h);
    draw_section_label(ctx, x, y + 12.0, "Text size");

    let fs_min = 8.0;
    let fs_max = 72.0;
    let fs_slider_row_y = y + 26.0;

    let fs_minus_x = x;
    draw_button(
        ctx,
        fs_minus_x,
        fs_slider_row_y,
        btn_size,
        btn_size,
        false,
        false,
    );
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    toolbar_icons::draw_icon_minus(
        ctx,
        fs_minus_x + (btn_size - nudge_icon_size) / 2.0,
        fs_slider_row_y + (btn_size - nudge_icon_size) / 2.0,
        nudge_icon_size,
    );
    hits.push(HitRegion {
        rect: (fs_minus_x, fs_slider_row_y, btn_size, btn_size),
        event: ToolbarEvent::SetFontSize((snapshot.font_size - 2.0).max(fs_min)),
        kind: HitKind::Click,
        tooltip: None,
    });

    let fs_plus_x = width - x - btn_size - value_w - 4.0;
    draw_button(
        ctx,
        fs_plus_x,
        fs_slider_row_y,
        btn_size,
        btn_size,
        false,
        false,
    );
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    toolbar_icons::draw_icon_plus(
        ctx,
        fs_plus_x + (btn_size - nudge_icon_size) / 2.0,
        fs_slider_row_y + (btn_size - nudge_icon_size) / 2.0,
        nudge_icon_size,
    );
    hits.push(HitRegion {
        rect: (fs_plus_x, fs_slider_row_y, btn_size, btn_size),
        event: ToolbarEvent::SetFontSize((snapshot.font_size + 2.0).min(fs_max)),
        kind: HitKind::Click,
        tooltip: None,
    });

    let fs_track_x = fs_minus_x + btn_size + 6.0;
    let fs_track_w = fs_plus_x - fs_track_x - 6.0;
    let fs_track_y = fs_slider_row_y + (btn_size - track_h) / 2.0;
    let fs_t = ((snapshot.font_size - fs_min) / (fs_max - fs_min)).clamp(0.0, 1.0);
    let fs_knob_x = fs_track_x + fs_t * (fs_track_w - knob_r * 2.0) + knob_r;

    ctx.set_source_rgba(0.5, 0.5, 0.6, 0.6);
    draw_round_rect(ctx, fs_track_x, fs_track_y, fs_track_w, track_h, 4.0);
    let _ = ctx.fill();
    ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
    ctx.arc(
        fs_knob_x,
        fs_track_y + track_h / 2.0,
        knob_r,
        0.0,
        std::f64::consts::PI * 2.0,
    );
    let _ = ctx.fill();

    hits.push(HitRegion {
        rect: (fs_track_x, fs_track_y - 6.0, fs_track_w, track_h + 12.0),
        event: ToolbarEvent::SetFontSize(snapshot.font_size),
        kind: HitKind::DragSetFontSize,
        tooltip: None,
    });

    let fs_text = format!("{:.0}pt", snapshot.font_size);
    draw_label_center(
        ctx,
        width - x - value_w,
        fs_slider_row_y,
        value_w,
        btn_size,
        &fs_text,
    );

    y += slider_card_h + section_gap;

    let font_card_h = 50.0;
    draw_group_card(ctx, card_x, y, card_w, font_card_h);
    draw_section_label(ctx, x, y + 14.0, "Font");

    let font_btn_h = 24.0;
    let font_gap = 8.0;
    let font_btn_w = (width - 2.0 * x - font_gap) / 2.0;
    let fonts = [
        FontDescriptor::new("Sans".to_string(), "bold".to_string(), "normal".to_string()),
        FontDescriptor::new(
            "Monospace".to_string(),
            "normal".to_string(),
            "normal".to_string(),
        ),
    ];
    let mut fx = x;
    let fy = y + 22.0;
    for font in fonts {
        let is_active = font.family == snapshot.font.family;
        let font_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, fx, fy, font_btn_w, font_btn_h))
            .unwrap_or(false);
        draw_button(ctx, fx, fy, font_btn_w, font_btn_h, is_active, font_hover);
        draw_label_left(ctx, fx + 8.0, fy, font_btn_w, font_btn_h, &font.family);
        hits.push(HitRegion {
            rect: (fx, fy, font_btn_w, font_btn_h),
            event: ToolbarEvent::SetFont(font.clone()),
            kind: HitKind::Click,
            tooltip: None,
        });
        fx += font_btn_w + font_gap;
    }

    y += font_card_h + section_gap;

    let actions_checkbox_h = 24.0;
    let actions_content_h = if snapshot.show_actions_section {
        if use_icons {
            let icon_btn_size = 42.0;
            let icon_gap = 6.0;
            let icon_rows = 3;
            (icon_btn_size + icon_gap) * icon_rows as f64
        } else {
            let action_h = 24.0;
            let action_gap = 5.0;
            let action_rows = 7;
            (action_h + action_gap) * action_rows as f64
        }
    } else {
        0.0
    };
    let actions_card_h = 20.0 + actions_checkbox_h + actions_content_h;

    draw_group_card(ctx, card_x, y, card_w, actions_card_h);
    draw_section_label(ctx, x, y + 14.0, "Actions");

    let actions_toggle_y = y + 22.0;
    let actions_toggle_w = card_w - 12.0;
    let actions_toggle_hover = hover
        .map(|(hx, hy)| {
            point_in_rect(
                hx,
                hy,
                x,
                actions_toggle_y,
                actions_toggle_w,
                actions_checkbox_h,
            )
        })
        .unwrap_or(false);
    draw_checkbox(
        ctx,
        x,
        actions_toggle_y,
        actions_toggle_w,
        actions_checkbox_h,
        snapshot.show_actions_section,
        actions_toggle_hover,
        "Show actions",
    );
    hits.push(HitRegion {
        rect: (x, actions_toggle_y, actions_toggle_w, actions_checkbox_h),
        event: ToolbarEvent::ToggleActionsSection(!snapshot.show_actions_section),
        kind: HitKind::Click,
        tooltip: None,
    });

    if snapshot.show_actions_section {
        let actions_start_y = actions_toggle_y + actions_checkbox_h + 6.0;

        type IconFn = fn(&cairo::Context, f64, f64, f64);
        let lock_label = if snapshot.zoom_locked {
            "Unlock Zoom"
        } else {
            "Lock Zoom"
        };
        let all_actions: &[(ToolbarEvent, IconFn, &str, bool)] = &[
            (
                ToolbarEvent::Undo,
                toolbar_icons::draw_icon_undo as IconFn,
                "Undo",
                snapshot.undo_available,
            ),
            (
                ToolbarEvent::Redo,
                toolbar_icons::draw_icon_redo as IconFn,
                "Redo",
                snapshot.redo_available,
            ),
            (
                ToolbarEvent::UndoAll,
                toolbar_icons::draw_icon_undo_all as IconFn,
                "Undo All",
                snapshot.undo_available,
            ),
            (
                ToolbarEvent::RedoAll,
                toolbar_icons::draw_icon_redo_all as IconFn,
                "Redo All",
                snapshot.redo_available,
            ),
            (
                ToolbarEvent::UndoAllDelayed,
                toolbar_icons::draw_icon_undo_all_delay as IconFn,
                "Undo All Delay",
                snapshot.undo_available,
            ),
            (
                ToolbarEvent::RedoAllDelayed,
                toolbar_icons::draw_icon_redo_all_delay as IconFn,
                "Redo All Delay",
                snapshot.redo_available,
            ),
            (
                ToolbarEvent::ClearCanvas,
                toolbar_icons::draw_icon_clear as IconFn,
                "Clear",
                true,
            ),
            (
                ToolbarEvent::ToggleFreeze,
                if snapshot.frozen_active {
                    toolbar_icons::draw_icon_unfreeze as IconFn
                } else {
                    toolbar_icons::draw_icon_freeze as IconFn
                },
                if snapshot.frozen_active {
                    "Unfreeze"
                } else {
                    "Freeze"
                },
                true,
            ),
            (
                ToolbarEvent::ZoomIn,
                toolbar_icons::draw_icon_zoom_in as IconFn,
                "Zoom In",
                true,
            ),
            (
                ToolbarEvent::ZoomOut,
                toolbar_icons::draw_icon_zoom_out as IconFn,
                "Zoom Out",
                true,
            ),
            (
                ToolbarEvent::ResetZoom,
                toolbar_icons::draw_icon_zoom_reset as IconFn,
                "Reset Zoom",
                snapshot.zoom_active,
            ),
            (
                ToolbarEvent::ToggleZoomLock,
                if snapshot.zoom_locked {
                    toolbar_icons::draw_icon_lock as IconFn
                } else {
                    toolbar_icons::draw_icon_unlock as IconFn
                },
                lock_label,
                snapshot.zoom_active,
            ),
            (
                ToolbarEvent::OpenConfigurator,
                toolbar_icons::draw_icon_settings as IconFn,
                "Config UI",
                true,
            ),
            (
                ToolbarEvent::OpenConfigFile,
                toolbar_icons::draw_icon_file as IconFn,
                "Config file",
                true,
            ),
        ];

        if use_icons {
            let icon_btn_size = 42.0;
            let icon_gap = 6.0;
            let icons_per_row = 5;
            let icon_size = 22.0;
            let total_icons_w =
                icons_per_row as f64 * icon_btn_size + (icons_per_row - 1) as f64 * icon_gap;
            let icons_start_x = x + (card_w - 12.0 - total_icons_w) / 2.0;

            for (idx, (evt, icon_fn, label, enabled)) in all_actions.iter().enumerate() {
                let row = idx / icons_per_row;
                let col = idx % icons_per_row;
                let bx = icons_start_x + (icon_btn_size + icon_gap) * col as f64;
                let by = actions_start_y + (icon_btn_size + icon_gap) * row as f64;
                let is_hover = hover
                    .map(|(hx, hy)| point_in_rect(hx, hy, bx, by, icon_btn_size, icon_btn_size))
                    .unwrap_or(false);

                if *enabled {
                    draw_button(ctx, bx, by, icon_btn_size, icon_btn_size, false, is_hover);
                    set_icon_color(ctx, is_hover);
                } else {
                    draw_button(ctx, bx, by, icon_btn_size, icon_btn_size, false, false);
                    ctx.set_source_rgba(0.5, 0.5, 0.55, 0.5);
                }

                let icon_x = bx + (icon_btn_size - icon_size) / 2.0;
                let icon_y = by + (icon_btn_size - icon_size) / 2.0;
                icon_fn(ctx, icon_x, icon_y, icon_size);

                hits.push(HitRegion {
                    rect: (bx, by, icon_btn_size, icon_btn_size),
                    event: evt.clone(),
                    kind: HitKind::Click,
                    tooltip: Some((*label).to_string()),
                });
            }
        } else {
            let action_h = 24.0;
            let action_gap = 5.0;
            let action_col_gap = 6.0;
            let action_w = ((width - 2.0 * x) - action_col_gap) / 2.0;

            for (idx, (evt, _icon, label, enabled)) in all_actions.iter().enumerate() {
                let row = idx / 2;
                let col = idx % 2;
                let bx = x + (action_w + action_col_gap) * col as f64;
                let by = actions_start_y + (action_h + action_gap) * row as f64;
                let is_hover = hover
                    .map(|(hx, hy)| point_in_rect(hx, hy, bx, by, action_w, action_h))
                    .unwrap_or(false);
                draw_button(ctx, bx, by, action_w, action_h, *enabled, is_hover);
                draw_label_center(ctx, bx, by, action_w, action_h, label);
                if *enabled {
                    hits.push(HitRegion {
                        rect: (bx, by, action_w, action_h),
                        event: evt.clone(),
                        kind: HitKind::Click,
                        tooltip: None,
                    });
                }
            }
        }
    }

    y += actions_card_h + section_gap;

    let custom_toggle_h = 24.0;
    let toggle_gap = 6.0;
    let toggles_h = custom_toggle_h * 2.0 + toggle_gap;
    let custom_content_h = if snapshot.custom_section_enabled {
        120.0
    } else {
        0.0
    };
    let delay_sliders_h = if snapshot.show_delay_sliders {
        55.0
    } else {
        0.0
    };
    let custom_card_h = 20.0 + toggles_h + custom_content_h + delay_sliders_h;
    draw_group_card(ctx, card_x, y, card_w, custom_card_h);
    draw_section_label(ctx, x, y + 14.0, "Step Undo/Redo");

    let custom_toggle_y = y + 22.0;
    let toggle_w = card_w - 12.0;

    let step_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, custom_toggle_y, toggle_w, custom_toggle_h))
        .unwrap_or(false);
    draw_checkbox(
        ctx,
        x,
        custom_toggle_y,
        toggle_w,
        custom_toggle_h,
        snapshot.custom_section_enabled,
        step_hover,
        "Step controls",
    );
    hits.push(HitRegion {
        rect: (x, custom_toggle_y, toggle_w, custom_toggle_h),
        event: ToolbarEvent::ToggleCustomSection(!snapshot.custom_section_enabled),
        kind: HitKind::Click,
        tooltip: None,
    });

    let delay_toggle_y = custom_toggle_y + custom_toggle_h + toggle_gap;
    let delay_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, delay_toggle_y, toggle_w, custom_toggle_h))
        .unwrap_or(false);
    draw_checkbox(
        ctx,
        x,
        delay_toggle_y,
        toggle_w,
        custom_toggle_h,
        snapshot.show_delay_sliders,
        delay_hover,
        "Delay sliders",
    );
    hits.push(HitRegion {
        rect: (x, delay_toggle_y, toggle_w, custom_toggle_h),
        event: ToolbarEvent::ToggleDelaySliders(!snapshot.show_delay_sliders),
        kind: HitKind::Click,
        tooltip: None,
    });

    let mut custom_y = delay_toggle_y + custom_toggle_h + 6.0;

    if snapshot.custom_section_enabled {
        let render_custom_row = |ctx: &cairo::Context,
                                 hits: &mut Vec<HitRegion>,
                                 x: f64,
                                 y: f64,
                                 w: f64,
                                 snapshot: &ToolbarSnapshot,
                                 is_undo: bool,
                                 hover: Option<(f64, f64)>| {
            let row_h = 26.0;
            let btn_w = if snapshot.use_icons { 42.0 } else { 90.0 };
            let steps_btn_w = 26.0;
            let gap = 6.0;
            let label = if is_undo { "Step Undo" } else { "Step Redo" };
            let steps = if is_undo {
                snapshot.custom_undo_steps
            } else {
                snapshot.custom_redo_steps
            };
            let delay_ms = if is_undo {
                snapshot.custom_undo_delay_ms
            } else {
                snapshot.custom_redo_delay_ms
            };

            let btn_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_w, row_h))
                .unwrap_or(false);

            if snapshot.use_icons {
                let icon_size = 20.0;
                draw_button(ctx, x, y, btn_w, row_h, false, btn_hover);
                set_icon_color(ctx, btn_hover);
                if is_undo {
                    toolbar_icons::draw_icon_step_undo(
                        ctx,
                        x + (btn_w - icon_size) / 2.0,
                        y + (row_h - icon_size) / 2.0,
                        icon_size,
                    );
                } else {
                    toolbar_icons::draw_icon_step_redo(
                        ctx,
                        x + (btn_w - icon_size) / 2.0,
                        y + (row_h - icon_size) / 2.0,
                        icon_size,
                    );
                }
            } else {
                draw_button(ctx, x, y, btn_w, row_h, false, btn_hover);
                draw_label_left(ctx, x + 10.0, y, btn_w - 20.0, row_h, label);
            }
            hits.push(HitRegion {
                rect: (x, y, btn_w, row_h),
                event: if is_undo {
                    ToolbarEvent::CustomUndo
                } else {
                    ToolbarEvent::CustomRedo
                },
                kind: HitKind::Click,
                tooltip: if snapshot.use_icons {
                    Some(if is_undo {
                        "Step Undo".to_string()
                    } else {
                        "Step Redo".to_string()
                    })
                } else {
                    None
                },
            });

            let steps_x = x + btn_w + gap;
            let minus_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, steps_x, y, steps_btn_w, row_h))
                .unwrap_or(false);
            draw_button(ctx, steps_x, y, steps_btn_w, row_h, false, minus_hover);
            set_icon_color(ctx, minus_hover);
            toolbar_icons::draw_icon_minus(
                ctx,
                steps_x + (steps_btn_w - 14.0) / 2.0,
                y + (row_h - 14.0) / 2.0,
                14.0,
            );
            hits.push(HitRegion {
                rect: (steps_x, y, steps_btn_w, row_h),
                event: if is_undo {
                    ToolbarEvent::SetCustomUndoSteps(steps.saturating_sub(1).max(1))
                } else {
                    ToolbarEvent::SetCustomRedoSteps(steps.saturating_sub(1).max(1))
                },
                kind: HitKind::Click,
                tooltip: None,
            });

            let steps_val_x = steps_x + steps_btn_w + 4.0;
            draw_label_center(
                ctx,
                steps_val_x,
                y,
                54.0,
                row_h,
                &format!("{} steps", steps),
            );

            let steps_plus_x = steps_val_x + 58.0;
            let plus_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, steps_plus_x, y, steps_btn_w, row_h))
                .unwrap_or(false);
            draw_button(ctx, steps_plus_x, y, steps_btn_w, row_h, false, plus_hover);
            set_icon_color(ctx, plus_hover);
            toolbar_icons::draw_icon_plus(
                ctx,
                steps_plus_x + (steps_btn_w - 14.0) / 2.0,
                y + (row_h - 14.0) / 2.0,
                14.0,
            );
            hits.push(HitRegion {
                rect: (steps_plus_x, y, steps_btn_w, row_h),
                event: if is_undo {
                    ToolbarEvent::SetCustomUndoSteps(steps.saturating_add(1))
                } else {
                    ToolbarEvent::SetCustomRedoSteps(steps.saturating_add(1))
                },
                kind: HitKind::Click,
                tooltip: None,
            });

            let slider_y = y + row_h + 8.0;
            let slider_h = 6.0;
            let slider_r = 6.0;
            let slider_w = w - 12.0;
            ctx.set_source_rgba(0.4, 0.4, 0.45, 0.7);
            draw_round_rect(ctx, x, slider_y, slider_w, slider_h, 3.0);
            let _ = ctx.fill();
            let t = delay_t_from_ms(delay_ms);
            let knob_x = x + t * (slider_w - slider_r * 2.0) + slider_r;
            ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
            ctx.arc(
                knob_x,
                slider_y + slider_h / 2.0,
                slider_r,
                0.0,
                std::f64::consts::PI * 2.0,
            );
            let _ = ctx.fill();
            hits.push(HitRegion {
                rect: (x, slider_y - 4.0, slider_w, slider_h + 8.0),
                event: if is_undo {
                    ToolbarEvent::SetCustomUndoDelay(delay_secs_from_t(t))
                } else {
                    ToolbarEvent::SetCustomRedoDelay(delay_secs_from_t(t))
                },
                kind: if is_undo {
                    HitKind::DragCustomUndoDelay
                } else {
                    HitKind::DragCustomRedoDelay
                },
                tooltip: None,
            });

            slider_y + slider_h + 10.0 - y
        };

        let undo_row_h = render_custom_row(ctx, hits, x, custom_y, card_w, snapshot, true, hover);
        custom_y += undo_row_h + 8.0;
        let _redo_row_h = render_custom_row(ctx, hits, x, custom_y, card_w, snapshot, false, hover);
    }

    if snapshot.show_delay_sliders {
        let sliders_w = card_w - 12.0;
        let slider_h = 6.0;
        let slider_knob_r = 6.0;
        let slider_start_y = y + 20.0 + toggles_h + custom_content_h + 4.0;

        let undo_label = format!(
            "Undo delay: {:.1}s",
            snapshot.undo_all_delay_ms as f64 / 1000.0
        );
        ctx.set_source_rgba(0.7, 0.7, 0.75, 0.9);
        ctx.set_font_size(11.0);
        ctx.move_to(x, slider_start_y + 10.0);
        let _ = ctx.show_text(&undo_label);

        let undo_slider_y = slider_start_y + 16.0;
        ctx.set_source_rgba(0.4, 0.4, 0.45, 0.7);
        draw_round_rect(ctx, x, undo_slider_y, sliders_w, slider_h, 3.0);
        let _ = ctx.fill();
        let undo_t = delay_t_from_ms(snapshot.undo_all_delay_ms);
        let undo_knob_x = x + undo_t * (sliders_w - slider_knob_r * 2.0) + slider_knob_r;
        ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
        ctx.arc(
            undo_knob_x,
            undo_slider_y + slider_h / 2.0,
            slider_knob_r,
            0.0,
            std::f64::consts::PI * 2.0,
        );
        let _ = ctx.fill();
        hits.push(HitRegion {
            rect: (x, undo_slider_y - 4.0, sliders_w, slider_h + 8.0),
            event: ToolbarEvent::SetUndoDelay(delay_secs_from_t(undo_t)),
            kind: HitKind::DragUndoDelay,
            tooltip: None,
        });

        let redo_label = format!(
            "Redo delay: {:.1}s",
            snapshot.redo_all_delay_ms as f64 / 1000.0
        );
        ctx.set_source_rgba(0.7, 0.7, 0.75, 0.9);
        ctx.move_to(x + sliders_w / 2.0 + 10.0, slider_start_y + 10.0);
        let _ = ctx.show_text(&redo_label);

        let redo_slider_y = slider_start_y + 32.0;
        ctx.set_source_rgba(0.4, 0.4, 0.45, 0.7);
        draw_round_rect(ctx, x, redo_slider_y, sliders_w, slider_h, 3.0);
        let _ = ctx.fill();
        let redo_t = delay_t_from_ms(snapshot.redo_all_delay_ms);
        let redo_knob_x = x + redo_t * (sliders_w - slider_knob_r * 2.0) + slider_knob_r;
        ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
        ctx.arc(
            redo_knob_x,
            redo_slider_y + slider_h / 2.0,
            slider_knob_r,
            0.0,
            std::f64::consts::PI * 2.0,
        );
        let _ = ctx.fill();
        hits.push(HitRegion {
            rect: (x, redo_slider_y - 4.0, sliders_w, slider_h + 8.0),
            event: ToolbarEvent::SetRedoDelay(delay_secs_from_t(redo_t)),
            kind: HitKind::DragRedoDelay,
            tooltip: None,
        });
    }

    draw_tooltip(ctx, hits, hover, width, false);
    Ok(())
}

fn draw_panel_background(ctx: &cairo::Context, width: f64, height: f64) {
    ctx.set_source_rgba(0.05, 0.05, 0.08, 0.92);
    draw_round_rect(ctx, 0.0, 0.0, width, height, 14.0);
    let _ = ctx.fill();
}

fn draw_drag_handle(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64, hover: bool) {
    draw_round_rect(ctx, x, y, w, h, 4.0);
    let alpha = if hover { 0.9 } else { 0.6 };
    ctx.set_source_rgba(1.0, 1.0, 1.0, alpha * 0.5);
    let _ = ctx.fill();

    ctx.set_line_width(1.1);
    ctx.set_source_rgba(1.0, 1.0, 1.0, alpha);
    let bar_w = w * 0.55;
    let bar_h = 2.0;
    let bar_x = x + (w - bar_w) / 2.0;
    let mut bar_y = y + (h - 3.0 * bar_h) / 2.0;
    for _ in 0..3 {
        draw_round_rect(ctx, bar_x, bar_y, bar_w, bar_h, 1.0);
        let _ = ctx.fill();
        bar_y += bar_h + 2.0;
    }
}

fn draw_group_card(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64) {
    ctx.set_source_rgba(0.12, 0.12, 0.18, 0.35);
    draw_round_rect(ctx, x, y, w, h, 8.0);
    let _ = ctx.fill();
}

fn point_in_rect(px: f64, py: f64, x: f64, y: f64, w: f64, h: f64) -> bool {
    px >= x && px <= x + w && py >= y && py <= y + h
}

fn set_icon_color(ctx: &cairo::Context, hover: bool) {
    if hover {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    } else {
        ctx.set_source_rgba(0.95, 0.95, 0.95, 0.9);
    }
}

fn draw_tooltip(
    ctx: &cairo::Context,
    hits: &[HitRegion],
    hover: Option<(f64, f64)>,
    panel_width: f64,
    above: bool,
) {
    let Some((hx, hy)) = hover else { return };

    for hit in hits {
        if hit.contains(hx, hy) {
            if let Some(text) = &hit.tooltip {
                ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
                ctx.set_font_size(12.0);

                if let Ok(ext) = ctx.text_extents(text) {
                    let pad = 6.0;
                    let tooltip_w = ext.width() + pad * 2.0;
                    let tooltip_h = ext.height() + pad * 2.0;

                    let btn_center_x = hit.rect.0 + hit.rect.2 / 2.0;
                    let mut tooltip_x = btn_center_x - tooltip_w / 2.0;
                    let gap = 6.0;
                    let tooltip_y = if above {
                        hit.rect.1 - tooltip_h - gap
                    } else {
                        hit.rect.1 + hit.rect.3 + gap
                    };

                    if tooltip_x < 4.0 {
                        tooltip_x = 4.0;
                    }
                    if tooltip_x + tooltip_w > panel_width - 4.0 {
                        tooltip_x = panel_width - tooltip_w - 4.0;
                    }

                    let shadow_offset = 2.0;
                    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.3);
                    draw_round_rect(
                        ctx,
                        tooltip_x + shadow_offset,
                        tooltip_y + shadow_offset,
                        tooltip_w,
                        tooltip_h,
                        4.0,
                    );
                    let _ = ctx.fill();

                    ctx.set_source_rgba(0.1, 0.1, 0.15, 0.95);
                    draw_round_rect(ctx, tooltip_x, tooltip_y, tooltip_w, tooltip_h, 4.0);
                    let _ = ctx.fill();

                    ctx.set_source_rgba(0.4, 0.4, 0.5, 0.8);
                    ctx.set_line_width(1.0);
                    draw_round_rect(ctx, tooltip_x, tooltip_y, tooltip_w, tooltip_h, 4.0);
                    let _ = ctx.stroke();

                    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
                    ctx.move_to(
                        tooltip_x + pad - ext.x_bearing(),
                        tooltip_y + pad - ext.y_bearing(),
                    );
                    let _ = ctx.show_text(text);
                }
                break;
            }
        }
    }
}

fn draw_close_button(ctx: &cairo::Context, x: f64, y: f64, size: f64, hover: bool) {
    let r = size / 2.0;
    let cx = x + r;
    let cy = y + r;

    if hover {
        ctx.set_source_rgba(0.8, 0.3, 0.3, 0.9);
    } else {
        ctx.set_source_rgba(0.5, 0.5, 0.55, 0.7);
    }
    ctx.arc(cx, cy, r, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();

    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    ctx.set_line_width(2.0);
    let inset = size * 0.3;
    ctx.move_to(x + inset, y + inset);
    ctx.line_to(x + size - inset, y + size - inset);
    let _ = ctx.stroke();
    ctx.move_to(x + size - inset, y + inset);
    ctx.line_to(x + inset, y + size - inset);
    let _ = ctx.stroke();
}

fn draw_pin_button(ctx: &cairo::Context, x: f64, y: f64, size: f64, pinned: bool, hover: bool) {
    let (r, g, b, a) = if pinned {
        (0.25, 0.6, 0.35, 0.95)
    } else if hover {
        (0.35, 0.35, 0.45, 0.85)
    } else {
        (0.3, 0.3, 0.35, 0.7)
    };
    ctx.set_source_rgba(r, g, b, a);
    draw_round_rect(ctx, x, y, size, size, 4.0);
    let _ = ctx.fill();

    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    let cx = x + size / 2.0;
    let cy = y + size / 2.0;
    let pin_r = size * 0.2;

    ctx.arc(cx, cy - pin_r * 0.5, pin_r, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();

    ctx.set_line_width(2.0);
    ctx.move_to(cx, cy + pin_r * 0.5);
    ctx.line_to(cx, cy + pin_r * 2.0);
    let _ = ctx.stroke();
}

fn draw_button(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64, active: bool, hover: bool) {
    let (r, g, b, a) = if active {
        (0.25, 0.5, 0.95, 0.95)
    } else if hover {
        (0.35, 0.35, 0.45, 0.85)
    } else {
        (0.2, 0.22, 0.26, 0.75)
    };
    ctx.set_source_rgba(r, g, b, a);
    draw_round_rect(ctx, x, y, w, h, 6.0);
    let _ = ctx.fill();
}

fn draw_label_center(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64, text: &str) {
    if let Ok(ext) = ctx.text_extents(text) {
        let tx = x + (w - ext.width()) / 2.0 - ext.x_bearing();
        let ty = y + (h - ext.height()) / 2.0 - ext.y_bearing();
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        ctx.move_to(tx, ty);
        let _ = ctx.show_text(text);
    }
}

fn draw_label_left(ctx: &cairo::Context, x: f64, y: f64, _w: f64, h: f64, text: &str) {
    if let Ok(ext) = ctx.text_extents(text) {
        let ty = y + (h - ext.height()) / 2.0 - ext.y_bearing();
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        ctx.move_to(x, ty);
        let _ = ctx.show_text(text);
    }
}

fn draw_section_label(ctx: &cairo::Context, x: f64, y: f64, text: &str) {
    ctx.set_source_rgba(0.8, 0.8, 0.85, 0.9);
    ctx.move_to(x, y);
    let _ = ctx.show_text(text);
}

fn draw_swatch(ctx: &cairo::Context, x: f64, y: f64, size: f64, color: Color, active: bool) {
    ctx.set_source_rgba(color.r, color.g, color.b, 1.0);
    draw_round_rect(ctx, x, y, size, size, 4.0);
    let _ = ctx.fill();

    let luminance = 0.299 * color.r + 0.587 * color.g + 0.114 * color.b;
    if luminance < 0.3 {
        ctx.set_source_rgba(0.5, 0.5, 0.5, 0.8);
        ctx.set_line_width(1.5);
        draw_round_rect(ctx, x, y, size, size, 4.0);
        let _ = ctx.stroke();
    }

    if active {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
        ctx.set_line_width(2.0);
        draw_round_rect(ctx, x - 2.0, y - 2.0, size + 4.0, size + 4.0, 5.0);
        let _ = ctx.stroke();
    }
}

fn draw_checkbox(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    checked: bool,
    hover: bool,
    label: &str,
) {
    let (r, g, b, a) = if hover {
        (0.32, 0.34, 0.4, 0.9)
    } else {
        (0.22, 0.24, 0.28, 0.75)
    };
    ctx.set_source_rgba(r, g, b, a);
    draw_round_rect(ctx, x, y, w, h, 4.0);
    let _ = ctx.fill();

    let box_size = h * 0.55;
    let box_x = x + 8.0;
    let box_y = y + (h - box_size) / 2.0;
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
    ctx.rectangle(box_x, box_y, box_size, box_size);
    ctx.set_line_width(1.5);
    let _ = ctx.stroke();
    if checked {
        ctx.move_to(box_x + 3.0, box_y + box_size / 2.0);
        ctx.line_to(box_x + box_size / 2.0, box_y + box_size - 3.0);
        ctx.line_to(box_x + box_size - 3.0, box_y + 3.0);
        let _ = ctx.stroke();
    }

    let label_x = box_x + box_size + 8.0;
    draw_label_left(ctx, label_x, y, w - (label_x - x), h, label);
}

fn draw_mini_checkbox(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    checked: bool,
    hover: bool,
    label: &str,
) {
    let (r, g, b, a) = if checked {
        (0.25, 0.5, 0.35, 0.9)
    } else if hover {
        (0.32, 0.34, 0.4, 0.85)
    } else {
        (0.2, 0.22, 0.26, 0.7)
    };
    ctx.set_source_rgba(r, g, b, a);
    draw_round_rect(ctx, x, y, w, h, 3.0);
    let _ = ctx.fill();

    let box_size = h * 0.6;
    let box_x = x + 4.0;
    let box_y = y + (h - box_size) / 2.0;
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.85);
    ctx.rectangle(box_x, box_y, box_size, box_size);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    if checked {
        ctx.move_to(box_x + 2.0, box_y + box_size / 2.0);
        ctx.line_to(box_x + box_size / 2.0, box_y + box_size - 2.0);
        ctx.line_to(box_x + box_size - 2.0, box_y + 2.0);
        let _ = ctx.stroke();
    }

    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(10.0);
    if let Ok(ext) = ctx.text_extents(label) {
        let label_x = x + box_size + 8.0 + (w - box_size - 12.0 - ext.width()) / 2.0;
        let label_y = y + (h + ext.height()) / 2.0;
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
        ctx.move_to(label_x, label_y);
        let _ = ctx.show_text(label);
    }
}

fn draw_color_picker(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64) {
    let hue_grad = cairo::LinearGradient::new(x, y, x + w, y);
    hue_grad.add_color_stop_rgba(0.0, 1.0, 0.0, 0.0, 1.0);
    hue_grad.add_color_stop_rgba(0.17, 1.0, 1.0, 0.0, 1.0);
    hue_grad.add_color_stop_rgba(0.33, 0.0, 1.0, 0.0, 1.0);
    hue_grad.add_color_stop_rgba(0.5, 0.0, 1.0, 1.0, 1.0);
    hue_grad.add_color_stop_rgba(0.66, 0.0, 0.0, 1.0, 1.0);
    hue_grad.add_color_stop_rgba(0.83, 1.0, 0.0, 1.0, 1.0);
    hue_grad.add_color_stop_rgba(1.0, 1.0, 0.0, 0.0, 1.0);

    ctx.rectangle(x, y, w, h);
    let _ = ctx.set_source(&hue_grad);
    let _ = ctx.fill();

    let val_grad = cairo::LinearGradient::new(x, y, x, y + h);
    val_grad.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.0);
    val_grad.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.65);
    ctx.rectangle(x, y, w, h);
    let _ = ctx.set_source(&val_grad);
    let _ = ctx.fill();

    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.4);
    ctx.rectangle(x + 0.5, y + 0.5, w - 1.0, h - 1.0);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();
}

fn draw_round_rect(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64, radius: f64) {
    let r = radius.min(w / 2.0).min(h / 2.0);
    ctx.new_sub_path();
    ctx.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    ctx.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    ctx.arc(
        x + r,
        y + h - r,
        r,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    ctx.arc(
        x + r,
        y + r,
        r,
        std::f64::consts::PI,
        std::f64::consts::PI * 1.5,
    );
    ctx.close_path();
}
