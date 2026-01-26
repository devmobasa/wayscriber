use std::f64::consts::PI;

use super::{ColorSectionInfo, SidePaletteLayout};
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::config::Action;
use crate::draw::{BLACK, BLUE, Color, GREEN, ORANGE, PINK, RED, WHITE, YELLOW};
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;
use crate::ui_text::UiTextStyle;

use super::super::widgets::constants::{FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL};
use super::super::widgets::*;

pub(super) fn draw_colors_section(layout: &mut SidePaletteLayout, y: &mut f64) -> ColorSectionInfo {
    let ctx = layout.ctx;
    let snapshot = layout.snapshot;
    let hits = &mut layout.hits;
    let hover = layout.hover;
    let x = layout.x;
    let card_x = layout.card_x;
    let card_w = layout.card_w;
    let content_width = layout.content_width;
    let section_gap = layout.section_gap;
    let label_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: FONT_SIZE_LABEL,
    };

    let basic_colors: &[(Color, &str, Option<Action>)] = &[
        (RED, "Red", Some(Action::SetColorRed)),
        (GREEN, "Green", Some(Action::SetColorGreen)),
        (BLUE, "Blue", Some(Action::SetColorBlue)),
        (YELLOW, "Yellow", Some(Action::SetColorYellow)),
        (WHITE, "White", Some(Action::SetColorWhite)),
        (BLACK, "Black", Some(Action::SetColorBlack)),
    ];
    let extended_colors: &[(Color, &str, Option<Action>)] = &[
        (ORANGE, "Orange", Some(Action::SetColorOrange)),
        (PINK, "Pink", Some(Action::SetColorPink)),
        (
            Color {
                r: 0.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            "Cyan",
            None,
        ),
        (
            Color {
                r: 0.6,
                g: 0.4,
                b: 0.8,
                a: 1.0,
            },
            "Purple",
            None,
        ),
        (
            Color {
                r: 0.4,
                g: 0.4,
                b: 0.4,
                a: 1.0,
            },
            "Gray",
            None,
        ),
    ];

    let swatch = ToolbarLayoutSpec::SIDE_COLOR_SWATCH;
    let swatch_gap = ToolbarLayoutSpec::SIDE_COLOR_SWATCH_GAP;
    let picker_h = ToolbarLayoutSpec::SIDE_COLOR_PICKER_INPUT_HEIGHT;
    let colors_card_h = layout.spec.side_colors_height(snapshot);

    draw_group_card(ctx, card_x, *y, card_w, colors_card_h);
    draw_section_label(
        ctx,
        label_style,
        x,
        *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_Y,
        "Colors",
    );

    let picker_y = *y + ToolbarLayoutSpec::SIDE_COLOR_PICKER_OFFSET_Y;
    let picker_w = content_width;
    // Visual height is fixed - use it for hit region to avoid overlap with hex input below
    let picker_visual_h = picker_h;
    let picker_hit_h = picker_visual_h;
    draw_color_picker(ctx, x, picker_y, picker_w, picker_visual_h);
    hits.push(HitRegion {
        rect: (x, picker_y, picker_w, picker_hit_h),
        event: ToolbarEvent::SetColor(snapshot.color),
        kind: HitKind::PickColor {
            x,
            y: picker_y,
            w: picker_w,
            h: picker_visual_h, // Use visual height for color calculation
        },
        tooltip: None,
    });

    // Draw indicator dot on gradient for current color position
    let (hue, _, value) = rgb_to_hsv(snapshot.color.r, snapshot.color.g, snapshot.color.b);
    let indicator_x = x + hue * picker_w;
    let indicator_y = picker_y + (1.0 - value) * picker_visual_h;
    draw_color_indicator(ctx, indicator_x, indicator_y, snapshot.color);

    // Draw current color preview row (between gradient and swatches)
    let preview_row_y = picker_y + picker_h + ToolbarLayoutSpec::SIDE_COLOR_PREVIEW_GAP_TOP;
    let preview_size = ToolbarLayoutSpec::SIDE_COLOR_PREVIEW_SIZE;

    // Draw preview swatch on the left (clickable to open color picker popup)
    let preview_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, preview_row_y, preview_size, preview_size))
        .unwrap_or(false);

    // Draw hover ring highlight when hovered
    if preview_hover {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.3);
        draw_round_rect(
            ctx,
            x - 2.0,
            preview_row_y - 2.0,
            preview_size + 4.0,
            preview_size + 4.0,
            6.0,
        );
        let _ = ctx.fill();
        // Brighter border ring
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.6);
        ctx.set_line_width(1.5);
        draw_round_rect(
            ctx,
            x - 2.0,
            preview_row_y - 2.0,
            preview_size + 4.0,
            preview_size + 4.0,
            6.0,
        );
        let _ = ctx.stroke();
    }

    draw_swatch(
        ctx,
        x,
        preview_row_y,
        preview_size,
        snapshot.color,
        preview_hover,
    );

    // Draw expand icon overlay in bottom-right corner of swatch
    let icon_size = ToolbarLayoutSpec::SIDE_COLOR_EXPAND_ICON_SIZE;
    let icon_x = x + preview_size - icon_size - 2.0;
    let icon_y = preview_row_y + preview_size - icon_size - 2.0;
    // Dark background circle for visibility on any color
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.4);
    ctx.arc(
        icon_x + icon_size / 2.0,
        icon_y + icon_size / 2.0,
        icon_size / 2.0 + 1.0,
        0.0,
        PI * 2.0,
    );
    let _ = ctx.fill();
    // Draw white expand arrow (↗)
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
    ctx.set_line_width(1.2);
    ctx.set_line_cap(cairo::LineCap::Round);
    let arrow_margin = icon_size * 0.2;
    let arrow_x1 = icon_x + arrow_margin;
    let arrow_y1 = icon_y + icon_size - arrow_margin;
    let arrow_x2 = icon_x + icon_size - arrow_margin;
    let arrow_y2 = icon_y + arrow_margin;
    ctx.move_to(arrow_x1, arrow_y1);
    ctx.line_to(arrow_x2, arrow_y2);
    let _ = ctx.stroke();
    // Arrow head
    let head_len = icon_size * 0.3;
    ctx.move_to(arrow_x2 - head_len, arrow_y2);
    ctx.line_to(arrow_x2, arrow_y2);
    ctx.line_to(arrow_x2, arrow_y2 + head_len);
    let _ = ctx.stroke();

    // Hit region for swatch only (no label)
    hits.push(HitRegion {
        rect: (x, preview_row_y, preview_size, preview_size),
        event: ToolbarEvent::OpenColorPickerPopup,
        kind: HitKind::Click,
        tooltip: Some("Pick color".to_string()),
    });

    // Draw hex value next to preview (clickable for copy/paste)
    let hex = format!(
        "#{:02X}{:02X}{:02X}",
        (snapshot.color.r * 255.0).round() as u8,
        (snapshot.color.g * 255.0).round() as u8,
        (snapshot.color.b * 255.0).round() as u8
    );
    let hex_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 11.0,
    };

    // Hex input background (subtle rounded rect)
    let hex_input_x = x + preview_size + 8.0;
    let hex_input_h = ToolbarLayoutSpec::SIDE_COLOR_HEX_INPUT_HEIGHT;
    let hex_input_y = preview_row_y + (preview_size - hex_input_h) / 2.0; // Center vertically with swatch
    let hex_input_w = ToolbarLayoutSpec::SIDE_COLOR_HEX_INPUT_WIDTH;
    let hex_icon_size = 10.0; // Small clipboard icon inside
    let hex_icon_pad = 4.0;

    let hex_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, hex_input_x, hex_input_y, hex_input_w, hex_input_h))
        .unwrap_or(false);

    // Draw hex input background with stronger hover state
    if hex_hover {
        // Subtle glow on hover
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.06);
        draw_round_rect(
            ctx,
            hex_input_x - 1.0,
            hex_input_y - 1.0,
            hex_input_w + 2.0,
            hex_input_h + 2.0,
            5.0,
        );
        let _ = ctx.fill();
        ctx.set_source_rgba(0.35, 0.35, 0.4, 0.9);
    } else {
        ctx.set_source_rgba(0.2, 0.2, 0.2, 0.6);
    }
    draw_round_rect(ctx, hex_input_x, hex_input_y, hex_input_w, hex_input_h, 4.0);
    let _ = ctx.fill();

    // Border on hover for clearer affordance
    if hex_hover {
        ctx.set_source_rgba(0.5, 0.5, 0.55, 0.6);
        ctx.set_line_width(1.0);
        draw_round_rect(ctx, hex_input_x, hex_input_y, hex_input_w, hex_input_h, 4.0);
        let _ = ctx.stroke();
    }

    // Draw small clipboard icon on the right side (indicates copy)
    let clip_icon_x = hex_input_x + hex_input_w - hex_icon_size - hex_icon_pad;
    let clip_icon_y = hex_input_y + (hex_input_h - hex_icon_size) / 2.0;
    if hex_hover {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.85);
    } else {
        ctx.set_source_rgba(0.6, 0.6, 0.6, 0.5);
    }
    toolbar_icons::draw_icon_paste(ctx, clip_icon_x, clip_icon_y, hex_icon_size);

    // Draw hex text (shifted left to make room for icon)
    ctx.set_source_rgba(0.85, 0.85, 0.85, 1.0);
    let hex_layout = crate::ui_text::text_layout(ctx, hex_style, &hex, None);
    let hex_extents = hex_layout.ink_extents();
    let text_area_w = hex_input_w - hex_icon_size - hex_icon_pad;
    hex_layout.show_at_baseline(
        ctx,
        hex_input_x + (text_area_w - hex_extents.width()) / 2.0,
        hex_input_y + hex_input_h / 2.0 + hex_extents.height() / 2.0,
    );

    // Hit region for hex input (click to copy, or paste from clipboard)
    hits.push(HitRegion {
        rect: (hex_input_x, hex_input_y, hex_input_w, hex_input_h),
        event: ToolbarEvent::CopyHexColor,
        kind: HitKind::Click,
        tooltip: Some("Click to copy hex (Ctrl+V to paste)".to_string()),
    });

    // Add paste button
    let paste_btn_x = hex_input_x + hex_input_w + 4.0;
    let paste_btn_size = 20.0; // Fixed size to match hex input
    let paste_btn_hover = hover
        .map(|(hx, hy)| {
            point_in_rect(
                hx,
                hy,
                paste_btn_x,
                hex_input_y,
                paste_btn_size,
                paste_btn_size,
            )
        })
        .unwrap_or(false);
    draw_button(
        ctx,
        paste_btn_x,
        hex_input_y,
        paste_btn_size,
        paste_btn_size,
        false,
        paste_btn_hover,
    );
    set_icon_color(ctx, paste_btn_hover);
    toolbar_icons::draw_icon_paste(
        ctx,
        paste_btn_x + (paste_btn_size - 12.0) / 2.0,
        hex_input_y + (paste_btn_size - 12.0) / 2.0,
        12.0,
    );
    hits.push(HitRegion {
        rect: (paste_btn_x, hex_input_y, paste_btn_size, paste_btn_size),
        event: ToolbarEvent::PasteHexColor,
        kind: HitKind::Click,
        tooltip: Some("Paste hex color from clipboard".to_string()),
    });

    let mut cx = x;
    let mut row_y = preview_row_y + preview_size + ToolbarLayoutSpec::SIDE_COLOR_PREVIEW_GAP_BOTTOM;
    for (color, name, action) in basic_colors {
        draw_swatch(ctx, cx, row_y, swatch, *color, *color == snapshot.color);
        let binding = action.and_then(|action| snapshot.binding_hints.binding_for_action(action));
        let tooltip = crate::backend::wayland::toolbar::format_binding_label(name, binding);
        hits.push(HitRegion {
            rect: (cx, row_y, swatch, swatch),
            event: ToolbarEvent::SetColor(*color),
            kind: HitKind::Click,
            tooltip: Some(tooltip),
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
        for (color, name, action) in extended_colors {
            draw_swatch(ctx, cx, row_y, swatch, *color, *color == snapshot.color);
            let binding =
                action.and_then(|action| snapshot.binding_hints.binding_for_action(action));
            let tooltip = crate::backend::wayland::toolbar::format_binding_label(name, binding);
            hits.push(HitRegion {
                rect: (cx, row_y, swatch, swatch),
                event: ToolbarEvent::SetColor(*color),
                kind: HitKind::Click,
                tooltip: Some(tooltip),
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

    *y += colors_card_h + section_gap;

    ColorSectionInfo {
        picker_y,
        picker_w,
        picker_h,
    }
}

pub(super) fn draw_preset_hover_highlight(
    layout: &SidePaletteLayout,
    info: &ColorSectionInfo,
    color: Color,
) {
    let ctx = layout.ctx;
    let x = layout.x;

    ctx.set_source_rgba(color.r, color.g, color.b, 0.85);
    ctx.set_line_width(2.0);
    draw_round_rect(
        ctx,
        x - 2.0,
        info.picker_y - 2.0,
        info.picker_w + 4.0,
        info.picker_h + 4.0,
        6.0,
    );
    let _ = ctx.stroke();
}
