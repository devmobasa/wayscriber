mod helpers;

use super::{ColorSectionInfo, SidePaletteLayout};
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::config::QuickColorPalette;
use crate::draw::Color;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSideSection};
use crate::ui_text::UiTextStyle;

use super::super::widgets::constants::{FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL};
use super::super::widgets::{draw_group_card, draw_round_rect};
use super::section_header::draw_collapsible_header;
use helpers::{
    ColorSwatch, ColorSwatchRowLayout, ColorSwatchToggle, draw_color_picker_area,
    draw_color_swatch_row, draw_hex_input, draw_preview_swatch_and_icon,
};

pub(super) fn draw_colors_section(
    layout: &mut SidePaletteLayout,
    y: &mut f64,
) -> Option<ColorSectionInfo> {
    let ctx = layout.ctx;
    let snapshot = layout.snapshot;
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

    if snapshot.side_section_hidden(ToolbarSideSection::Colors) {
        return None;
    }

    let palette_colors = palette_swatches(&snapshot.quick_colors);

    let swatch = ToolbarLayoutSpec::SIDE_COLOR_SWATCH;
    let swatch_gap = ToolbarLayoutSpec::SIDE_COLOR_SWATCH_GAP;
    let picker_h = ToolbarLayoutSpec::SIDE_COLOR_PICKER_INPUT_HEIGHT;
    let colors_card_h = layout.spec.side_colors_height(snapshot);

    draw_group_card(ctx, card_x, *y, card_w, colors_card_h);
    draw_collapsible_header(
        layout,
        *y,
        label_style,
        ToolbarSideSection::Colors,
        ToolbarSideSection::Colors.label(),
        ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_Y,
    );
    if snapshot.side_section_collapsed(ToolbarSideSection::Colors) {
        *y += colors_card_h + section_gap;
        return None;
    }

    let hits = &mut layout.hits;
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
    let first_row_len = palette_colors
        .len()
        .min(ToolbarLayoutSpec::SIDE_COLOR_SWATCHES_PER_ROW);
    let basic_toggle = if snapshot.show_more_colors
        || palette_colors.len() <= ToolbarLayoutSpec::SIDE_COLOR_SWATCHES_PER_ROW
    {
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
        &palette_colors[..first_row_len],
        basic_toggle,
    );

    row_y += swatch + swatch_gap;
    if snapshot.show_more_colors {
        let rows = palette_colors[first_row_len..]
            .chunks(ToolbarLayoutSpec::SIDE_COLOR_SWATCHES_PER_ROW)
            .collect::<Vec<_>>();
        for (row_index, row_colors) in rows.iter().enumerate() {
            let is_last_row = row_index + 1 == rows.len();
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
                row_colors,
                is_last_row.then_some(ColorSwatchToggle {
                    event: ToolbarEvent::ToggleMoreColors(false),
                    tooltip: "Hide colors",
                    icon_fn: crate::toolbar_icons::draw_icon_minus,
                }),
            );
            row_y += swatch + swatch_gap;
        }
    }

    *y += colors_card_h + section_gap;

    Some(ColorSectionInfo {
        picker_y,
        picker_w,
        picker_h,
    })
}

fn palette_swatches(palette: &QuickColorPalette) -> Vec<ColorSwatch> {
    palette
        .entries()
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            (
                entry.color,
                entry.label.clone(),
                QuickColorPalette::action_for_index(index),
            )
        })
        .collect()
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
