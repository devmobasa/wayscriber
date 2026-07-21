use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::toolbar_icons;
use crate::ui::toolbar::model::{
    SideHeaderModel, ToolbarBoardChipPresentation, ToolbarControl, ToolbarControlKind,
    ToolbarPresentationPayload,
};
use crate::ui::toolbar::{SidePane, ToolbarEvent};
use crate::ui_text::UiTextStyle;

use super::super::widgets::constants::{
    COLOR_HEADER_BAND, FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL, FONT_SIZE_TOOLTIP, RADIUS_CARD,
    set_color,
};
use super::super::widgets::*;
use crate::ui::theme::Rgba;

/// Board chip fill: cool slate tint (hover is one step lighter). Specific
/// to the chip — no theme token.
const COLOR_BOARD_CHIP_BG_HOVER: Rgba = (0.28, 0.30, 0.34, 0.95);
const COLOR_BOARD_CHIP_BG: Rgba = (0.22, 0.24, 0.28, 0.95);
/// Board chip outline (hover/idle), cool-tinted to match the fill.
const COLOR_BOARD_CHIP_BORDER_HOVER: Rgba = (0.65, 0.7, 0.8, 0.7);
const COLOR_BOARD_CHIP_BORDER: Rgba = (0.65, 0.7, 0.8, 0.45);
/// Placeholder outline where the board color dot would sit.
const COLOR_BOARD_CHIP_EMPTY_DOT: Rgba = (0.62, 0.68, 0.76, 0.7);
/// Chip chevron glyph (hover/idle).
const COLOR_CHEVRON_HOVER: Rgba = (0.7, 0.72, 0.78, 0.95);
const COLOR_CHEVRON: Rgba = (0.7, 0.72, 0.78, 0.7);

/// Fixed chrome: one header row (drag grip, board chip, pin, close) plus the
/// pane navigation row. Returns the content start y — nothing above it
/// scrolls or moves when panes or sections change.
pub(super) fn draw_header(layout: &mut SidePaletteLayout) -> f64 {
    let ctx = layout.ctx;
    let snapshot = layout.snapshot;
    let hits = &mut layout.hits;
    let hover = layout.hover;
    let spec = &layout.spec;
    let x = layout.x;
    let width = layout.width;
    let content_width = layout.content_width;
    let label_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: FONT_SIZE_LABEL,
    };
    let header_model = SideHeaderModel::from_snapshot(snapshot);

    let band_y = ToolbarLayoutSpec::SIDE_TOP_PADDING - 2.0;
    let band_h = ToolbarLayoutSpec::SIDE_HEADER_ROW1_HEIGHT + 4.0;
    let band_x = spec.side_card_x();
    let band_w = spec.side_card_width(width);
    draw_round_rect(ctx, band_x, band_y, band_w, band_h, RADIUS_CARD);
    set_color(ctx, COLOR_HEADER_BAND);
    let _ = ctx.fill();

    let btn_size = ToolbarLayoutSpec::SIDE_HEADER_BUTTON_SIZE;
    let btn_gap = ToolbarLayoutSpec::SIDE_HEADER_BUTTON_GAP;
    let btn_margin = ToolbarLayoutSpec::SIDE_HEADER_BUTTON_MARGIN_RIGHT;

    // ========== Header row: drag grip · board chip · pin · close ==========
    let row1_y = ToolbarLayoutSpec::SIDE_TOP_PADDING;
    let row1_h = ToolbarLayoutSpec::SIDE_HEADER_ROW1_HEIGHT;

    let drag_size = ToolbarLayoutSpec::SIDE_HEADER_DRAG_SIZE;
    let drag_y = row1_y + (row1_h - drag_size) / 2.0;
    let drag_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, drag_y, drag_size, drag_size))
        .unwrap_or(false);
    draw_drag_handle(ctx, x, drag_y, drag_size, drag_size, drag_hover);
    hits.push(HitRegion {
        focus_id: None,
        rect: (x, drag_y, drag_size, drag_size),
        event: single_control_event(&header_model.drag),
        kind: HitKind::DragMoveSide,
        tooltip: header_model.drag.presentation.tooltip.as_string(),
    });

    let btn_y = row1_y + (row1_h - btn_size) / 2.0;
    let close_x = width - btn_margin - btn_size;
    let pin_x = close_x - btn_size - btn_gap;

    // Board chip fills the middle of the header row.
    let chip_x = x + drag_size + 8.0;
    let chip_w = pin_x - 8.0 - chip_x;
    let chip_h = ToolbarLayoutSpec::SIDE_BOARD_CHIP_HEIGHT;
    let chip_y = row1_y + (row1_h - chip_h) / 2.0;
    draw_board_chip(
        ctx,
        hits,
        hover,
        &header_model,
        label_style,
        chip_x,
        chip_y,
        chip_w,
        chip_h,
    );

    let pin_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, pin_x, btn_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_pin_button(ctx, pin_x, btn_y, btn_size, snapshot.side_pinned, pin_hover);
    hits.push(HitRegion {
        focus_id: None,
        rect: (pin_x, btn_y, btn_size, btn_size),
        event: single_control_event(&header_model.pin),
        kind: HitKind::Click,
        tooltip: header_model.pin.presentation.tooltip.as_string(),
    });

    let close_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, close_x, btn_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_side_minimize_button(ctx, close_x, btn_y, btn_size, close_hover);
    hits.push(HitRegion {
        focus_id: None,
        rect: (close_x, btn_y, btn_size, btn_size),
        event: single_control_event(&header_model.close),
        kind: HitKind::Click,
        tooltip: header_model.close.presentation.tooltip.as_string(),
    });

    // ========== Pane navigation row ==========
    let nav_y = spec.side_pane_nav_y();
    let nav_h = ToolbarLayoutSpec::SIDE_PANE_NAV_HEIGHT;
    let nav_gap = 4.0;
    let tab_count = SidePane::ALL.len() as f64;
    let tab_w = (content_width - nav_gap * (tab_count - 1.0)) / tab_count;
    let nav_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: FONT_SIZE_TOOLTIP,
    };
    let mut tab_x = x;
    for pane in SidePane::ALL {
        let selected = snapshot.active_side_pane == pane;
        let tab_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, tab_x, nav_y, tab_w, nav_h))
            .unwrap_or(false);
        draw_button(ctx, tab_x, nav_y, tab_w, nav_h, selected, tab_hover);
        draw_label_center(ctx, nav_style, tab_x, nav_y, tab_w, nav_h, pane.label());
        hits.push(HitRegion {
            focus_id: None,
            rect: (tab_x, nav_y, tab_w, nav_h),
            event: ToolbarEvent::SetSidePane(pane),
            kind: HitKind::Click,
            tooltip: Some(format!("{} pane", pane.label())),
        });
        tab_x += tab_w + nav_gap;
    }

    spec.side_content_start_y()
}

#[allow(clippy::too_many_arguments)]
fn draw_board_chip(
    ctx: &cairo::Context,
    hits: &mut Vec<HitRegion>,
    hover: Option<(f64, f64)>,
    header_model: &SideHeaderModel,
    label_style: UiTextStyle<'_>,
    chip_x: f64,
    chip_y: f64,
    chip_w: f64,
    chip_h: f64,
) {
    let chip_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, chip_x, chip_y, chip_w, chip_h))
        .unwrap_or(false);
    let board_chip = board_chip_payload(header_model);
    draw_round_rect(ctx, chip_x, chip_y, chip_w, chip_h, 6.0);
    set_color(
        ctx,
        if chip_hover {
            COLOR_BOARD_CHIP_BG_HOVER
        } else {
            COLOR_BOARD_CHIP_BG
        },
    );
    let _ = ctx.fill();
    set_color(
        ctx,
        if chip_hover {
            COLOR_BOARD_CHIP_BORDER_HOVER
        } else {
            COLOR_BOARD_CHIP_BORDER
        },
    );
    ctx.set_line_width(1.0);
    draw_round_rect(ctx, chip_x, chip_y, chip_w, chip_h, 6.0);
    let _ = ctx.stroke();

    // Color dot
    let dot_size = ToolbarLayoutSpec::SIDE_BOARD_COLOR_DOT_SIZE;
    let dot_x = chip_x + 6.0;
    let dot_y = chip_y + (chip_h - dot_size) * 0.5;
    if let Some(color) = board_chip.and_then(|chip| chip.color) {
        draw_swatch(ctx, dot_x, dot_y, dot_size, color, false);
    } else {
        set_color(ctx, COLOR_BOARD_CHIP_EMPTY_DOT);
        ctx.set_line_width(1.0);
        draw_round_rect(ctx, dot_x, dot_y, dot_size, dot_size, 3.0);
        let _ = ctx.stroke();
    }

    // Board icon before label
    let icon_size = 10.0;
    let icon_x = dot_x + dot_size + 4.0;
    let icon_y = chip_y + (chip_h - icon_size) * 0.5;
    set_icon_color(ctx, chip_hover);
    toolbar_icons::draw_icon_board(ctx, icon_x, icon_y, icon_size);

    // Chevron on the right
    let chevron_size = ToolbarLayoutSpec::SIDE_BOARD_CHEVRON_SIZE;
    let chevron_x = chip_x + chip_w - chevron_size - 2.0;
    let chevron_y = chip_y + (chip_h - chevron_size) / 2.0;
    draw_chevron_right(ctx, chevron_x, chevron_y, chevron_size, chip_hover);

    // Board label (truncated)
    let label_x = icon_x + icon_size + 5.0;
    let label_w = chevron_x - label_x - 4.0;
    let label = board_chip
        .map(|chip| chip.label.as_str())
        .unwrap_or(header_model.board_chip.presentation.label.as_ref());
    let display_label = ellipsize_to_width(ctx, label_style, label, label_w);
    draw_label_left(
        ctx,
        label_style,
        label_x,
        chip_y,
        label_w,
        chip_h,
        &display_label,
    );

    hits.push(HitRegion {
        focus_id: None,
        rect: (chip_x, chip_y, chip_w, chip_h),
        event: single_control_event(&header_model.board_chip),
        kind: HitKind::Click,
        tooltip: header_model.board_chip.presentation.tooltip.as_string(),
    });
}

/// Draw a right-pointing chevron
fn draw_chevron_right(ctx: &cairo::Context, x: f64, y: f64, size: f64, hover: bool) {
    set_color(
        ctx,
        if hover {
            COLOR_CHEVRON_HOVER
        } else {
            COLOR_CHEVRON
        },
    );
    ctx.set_line_width(1.5);

    let margin = size * 0.3;
    let mid_y = y + size / 2.0;
    ctx.move_to(x + margin, y + margin);
    ctx.line_to(x + size - margin, mid_y);
    ctx.line_to(x + margin, y + size - margin);
    let _ = ctx.stroke();
}

fn single_control_event(control: &ToolbarControl) -> ToolbarEvent {
    let ToolbarControlKind::Single(single) = &control.kind else {
        return ToolbarEvent::SetSideMinimized(true);
    };
    single.activation.compatibility_event()
}

fn board_chip_payload(model: &SideHeaderModel) -> Option<&ToolbarBoardChipPresentation> {
    match &model.board_chip.presentation.payload {
        ToolbarPresentationPayload::BoardChip(chip) => Some(chip),
        ToolbarPresentationPayload::None => None,
    }
}
