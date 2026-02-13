mod helpers;

use super::{ColorSectionInfo, SidePaletteLayout};
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::config::Action;
use crate::draw::{BLACK, BLUE, Color, GREEN, ORANGE, PINK, RED, WHITE, YELLOW};
use crate::ui::toolbar::ToolbarEvent;
use crate::ui_text::UiTextStyle;

use super::super::widgets::constants::{FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL};
use super::super::widgets::{draw_group_card, draw_round_rect, draw_section_label};
use helpers::{
    ColorSwatch, ColorSwatchRowLayout, ColorSwatchToggle, draw_color_picker_area,
    draw_color_swatch_row, draw_hex_input, draw_preview_swatch_and_icon,
};

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

    let basic_colors: &[ColorSwatch] = &[
        (RED, "Red", Some(Action::SetColorRed)),
        (GREEN, "Green", Some(Action::SetColorGreen)),
        (BLUE, "Blue", Some(Action::SetColorBlue)),
        (YELLOW, "Yellow", Some(Action::SetColorYellow)),
        (WHITE, "White", Some(Action::SetColorWhite)),
        (BLACK, "Black", Some(Action::SetColorBlack)),
    ];
    let extended_colors: &[ColorSwatch] = &[
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
    draw_color_picker_area(ctx, hits, snapshot, x, picker_y, picker_w, picker_h);
    let (preview_row_y, preview_size) =
        draw_preview_swatch_and_icon(ctx, hits, hover, snapshot, x, picker_y, picker_h);

    let hex = format!(
        "#{:02X}{:02X}{:02X}",
        (snapshot.color.r * 255.0).round() as u8,
        (snapshot.color.g * 255.0).round() as u8,
        (snapshot.color.b * 255.0).round() as u8
    );
    draw_hex_input(ctx, hits, hover, x, preview_row_y, preview_size, &hex);

    let mut row_y = preview_row_y + preview_size + ToolbarLayoutSpec::SIDE_COLOR_PREVIEW_GAP_BOTTOM;
    let basic_toggle = if snapshot.show_more_colors {
        None
    } else {
        Some(ColorSwatchToggle {
            event: ToolbarEvent::ToggleMoreColors(true),
            tooltip: "More colors",
            icon_fn: crate::toolbar_icons::draw_icon_plus,
        })
    };
    draw_color_swatch_row(
        ctx,
        hits,
        hover,
        snapshot,
        ColorSwatchRowLayout {
            start_x: x,
            row_y,
            swatch,
            swatch_gap,
        },
        basic_colors,
        basic_toggle,
    );

    row_y += swatch + swatch_gap;
    if snapshot.show_more_colors {
        draw_color_swatch_row(
            ctx,
            hits,
            hover,
            snapshot,
            ColorSwatchRowLayout {
                start_x: x,
                row_y,
                swatch,
                swatch_gap,
            },
            extended_colors,
            Some(ColorSwatchToggle {
                event: ToolbarEvent::ToggleMoreColors(false),
                tooltip: "Hide colors",
                icon_fn: crate::toolbar_icons::draw_icon_minus,
            }),
        );
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
