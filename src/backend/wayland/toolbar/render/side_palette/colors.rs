use super::{ColorSectionInfo, SidePaletteLayout};
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::draw::{BLACK, BLUE, Color, GREEN, ORANGE, PINK, RED, WHITE, YELLOW};
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;

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

    let swatch = ToolbarLayoutSpec::SIDE_COLOR_SWATCH;
    let swatch_gap = ToolbarLayoutSpec::SIDE_COLOR_SWATCH_GAP;
    let picker_h = ToolbarLayoutSpec::SIDE_COLOR_PICKER_INPUT_HEIGHT;
    let colors_card_h = layout.spec.side_colors_height(snapshot);

    draw_group_card(ctx, card_x, *y, card_w, colors_card_h);
    draw_section_label(
        ctx,
        x,
        *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_Y,
        "Colors",
    );

    let picker_y = *y + ToolbarLayoutSpec::SIDE_COLOR_PICKER_OFFSET_Y;
    let picker_w = content_width;
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
