use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::config::ToolbarLayoutMode;
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;
use crate::ui_text::{UiTextStyle, text_layout};

use super::super::widgets::constants::{
    COLOR_HEADER_BAND, FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL, RADIUS_CARD, set_color,
};
use super::super::widgets::*;

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

    let band_y = ToolbarLayoutSpec::SIDE_TOP_PADDING - 2.0;
    let band_h = ToolbarLayoutSpec::SIDE_HEADER_ROW1_HEIGHT
        + ToolbarLayoutSpec::SIDE_HEADER_ROW2_HEIGHT
        + 4.0;
    let band_x = spec.side_card_x();
    let band_w = spec.side_card_width(width);
    draw_round_rect(ctx, band_x, band_y, band_w, band_h, RADIUS_CARD);
    set_color(ctx, COLOR_HEADER_BAND);
    let _ = ctx.fill();

    let btn_size = ToolbarLayoutSpec::SIDE_HEADER_BUTTON_SIZE;
    let btn_gap = ToolbarLayoutSpec::SIDE_HEADER_BUTTON_GAP;
    let btn_margin = ToolbarLayoutSpec::SIDE_HEADER_BUTTON_MARGIN_RIGHT;

    // ========== ROW 1: Drag + Ico/Txt (center) + Pin/Close ==========
    let row1_y = ToolbarLayoutSpec::SIDE_TOP_PADDING;
    let row1_h = ToolbarLayoutSpec::SIDE_HEADER_ROW1_HEIGHT;

    // Drag handle on left (compact 18x18)
    let drag_size = ToolbarLayoutSpec::SIDE_HEADER_DRAG_SIZE;
    let drag_y = row1_y + (row1_h - drag_size) / 2.0;
    let drag_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, drag_y, drag_size, drag_size))
        .unwrap_or(false);
    draw_drag_handle(ctx, x, drag_y, drag_size, drag_size, drag_hover);
    hits.push(HitRegion {
        rect: (x, drag_y, drag_size, drag_size),
        event: ToolbarEvent::MoveSideToolbar { x: 0.0, y: 0.0 },
        kind: HitKind::DragMoveSide,
        tooltip: Some("Drag toolbar".to_string()),
    });

    // Utility buttons on right: [Pin] [Close]
    let btn_y = row1_y + (row1_h - btn_size) / 2.0;
    let close_x = width - btn_margin - btn_size;
    let pin_x = close_x - btn_size - btn_gap;

    // Ico/Txt toggle in center of row 1
    let icons_w = ToolbarLayoutSpec::SIDE_MODE_ICONS_WIDTH;
    let segment_h = ToolbarLayoutSpec::SIDE_SEGMENT_HEIGHT;
    let center_start = x + drag_size + 8.0;
    let center_end = pin_x - 8.0;
    let icons_x = center_start + (center_end - center_start - icons_w) / 2.0;
    let icons_y = row1_y + (row1_h - segment_h) / 2.0;
    let icons_active = if snapshot.use_icons { 0 } else { 1 };
    let icons_hover = hover.and_then(|(hx, hy)| {
        if point_in_rect(hx, hy, icons_x, icons_y, icons_w, segment_h) {
            Some(if hx < icons_x + icons_w / 2.0 { 0 } else { 1 })
        } else {
            None
        }
    });
    draw_segmented_control(
        ctx,
        icons_x,
        icons_y,
        icons_w,
        segment_h,
        ("Ico", "Txt"),
        icons_active,
        icons_hover,
        label_style,
    );

    // Pin button
    let pin_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, pin_x, btn_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_pin_button(ctx, pin_x, btn_y, btn_size, snapshot.side_pinned, pin_hover);
    hits.push(HitRegion {
        rect: (pin_x, btn_y, btn_size, btn_size),
        event: ToolbarEvent::PinSideToolbar(!snapshot.side_pinned),
        kind: HitKind::Click,
        tooltip: Some(if snapshot.side_pinned {
            "Pinned (click to unpin)".to_string()
        } else {
            "Pin toolbar".to_string()
        }),
    });

    // Close button
    let close_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, close_x, btn_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_close_button(ctx, close_x, btn_y, btn_size, close_hover);
    hits.push(HitRegion {
        rect: (close_x, btn_y, btn_size, btn_size),
        event: ToolbarEvent::CloseSideToolbar,
        kind: HitKind::Click,
        tooltip: Some("Close".to_string()),
    });

    let icons_half_w = icons_w / 2.0;
    hits.push(HitRegion {
        rect: (icons_x, icons_y, icons_half_w, segment_h),
        event: ToolbarEvent::ToggleIconMode(true),
        kind: HitKind::Click,
        tooltip: Some("Icons mode".to_string()),
    });
    hits.push(HitRegion {
        rect: (icons_x + icons_half_w, icons_y, icons_half_w, segment_h),
        event: ToolbarEvent::ToggleIconMode(false),
        kind: HitKind::Click,
        tooltip: Some("Text mode".to_string()),
    });

    // ========== ROW 2: Simple/Full + More ==========
    let row2_y = spec.side_header_row2_y();
    let row2_h = ToolbarLayoutSpec::SIDE_HEADER_ROW2_HEIGHT;

    let segment_h = ToolbarLayoutSpec::SIDE_SEGMENT_HEIGHT;
    let segment_y = row2_y + (row2_h - segment_h) / 2.0;

    // Segmented control: [Simple | Full]
    let layout_w = ToolbarLayoutSpec::SIDE_MODE_LAYOUT_WIDTH;
    let layout_x = x;
    let layout_active = match snapshot.layout_mode {
        ToolbarLayoutMode::Simple => 0,
        _ => 1,
    };
    let layout_hover = hover.and_then(|(hx, hy)| {
        if point_in_rect(hx, hy, layout_x, segment_y, layout_w, segment_h) {
            Some(if hx < layout_x + layout_w / 2.0 { 0 } else { 1 })
        } else {
            None
        }
    });
    draw_segmented_control(
        ctx,
        layout_x,
        segment_y,
        layout_w,
        segment_h,
        ("Simple", "Full"),
        layout_active,
        layout_hover,
        label_style,
    );

    let more_x = x + content_width - btn_size;
    let more_y = row2_y + (row2_h - btn_size) / 2.0;
    let more_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, more_x, more_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_button(
        ctx,
        more_x,
        more_y,
        btn_size,
        btn_size,
        snapshot.drawer_open,
        more_hover,
    );
    set_icon_color(ctx, more_hover);
    let icon_size = ToolbarLayoutSpec::SIDE_ACTION_ICON_SIZE;
    let icon_x = more_x + (btn_size - icon_size) / 2.0;
    let icon_y = more_y + (btn_size - icon_size) / 2.0;
    toolbar_icons::draw_icon_more(ctx, icon_x, icon_y, icon_size);
    if snapshot.show_drawer_hint {
        let dot_radius = 4.0;
        let dot_x = more_x + btn_size - dot_radius - 2.0;
        let dot_y = more_y + dot_radius + 2.0;
        ctx.arc(dot_x, dot_y, dot_radius, 0.0, std::f64::consts::TAU);
        ctx.set_source_rgba(0.95, 0.45, 0.15, 0.95);
        let _ = ctx.fill();
    }

    let half_w = layout_w / 2.0;
    let full_mode = if snapshot.layout_mode == ToolbarLayoutMode::Advanced {
        ToolbarLayoutMode::Advanced
    } else {
        ToolbarLayoutMode::Regular
    };
    hits.push(HitRegion {
        rect: (layout_x, segment_y, half_w, segment_h),
        event: ToolbarEvent::SetToolbarLayoutMode(ToolbarLayoutMode::Simple),
        kind: HitKind::Click,
        tooltip: Some("Simple mode".to_string()),
    });
    hits.push(HitRegion {
        rect: (layout_x + half_w, segment_y, half_w, segment_h),
        event: ToolbarEvent::SetToolbarLayoutMode(full_mode),
        kind: HitKind::Click,
        tooltip: Some("Full mode".to_string()),
    });
    hits.push(HitRegion {
        rect: (more_x, more_y, btn_size, btn_size),
        event: ToolbarEvent::ToggleDrawer(!snapshot.drawer_open),
        kind: HitKind::Click,
        tooltip: Some("More options".to_string()),
    });

    // ========== ROW 3: Board chip ==========
    let row3_y = spec.side_header_row3_y();
    let row3_h = ToolbarLayoutSpec::SIDE_HEADER_ROW3_HEIGHT;

    let chip_x = x;
    let chip_w = content_width;
    let chip_h = ToolbarLayoutSpec::SIDE_BOARD_CHIP_HEIGHT;
    let chip_y = row3_y + (row3_h - chip_h) / 2.0;

    let chip_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, chip_x, chip_y, chip_w, chip_h))
        .unwrap_or(false);
    let chip_bg = if chip_hover { 0.28 } else { 0.22 };
    draw_round_rect(ctx, chip_x, chip_y, chip_w, chip_h, 6.0);
    ctx.set_source_rgba(chip_bg, chip_bg + 0.02, chip_bg + 0.06, 0.95);
    let _ = ctx.fill();
    let border_alpha = if chip_hover { 0.7 } else { 0.45 };
    ctx.set_source_rgba(0.65, 0.7, 0.8, border_alpha);
    ctx.set_line_width(1.0);
    draw_round_rect(ctx, chip_x, chip_y, chip_w, chip_h, 6.0);
    let _ = ctx.stroke();

    // Color dot
    let dot_size = ToolbarLayoutSpec::SIDE_BOARD_COLOR_DOT_SIZE;
    let dot_x = chip_x + 6.0;
    let dot_y = chip_y + (chip_h - dot_size) * 0.5;
    if let Some(color) = snapshot.board_color {
        draw_swatch(ctx, dot_x, dot_y, dot_size, color, false);
    } else {
        ctx.set_source_rgba(0.62, 0.68, 0.76, 0.7);
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
    let label = board_chip_label(snapshot);
    let display_label = ellipsize_to_width(ctx, label_style, &label, label_w);
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
        rect: (chip_x, chip_y, chip_w, chip_h),
        event: ToolbarEvent::ToggleBoardPicker,
        kind: HitKind::Click,
        tooltip: Some("Board settings".to_string()),
    });

    spec.side_content_start_y()
}

/// Draw a right-pointing chevron
fn draw_chevron_right(ctx: &cairo::Context, x: f64, y: f64, size: f64, hover: bool) {
    let alpha = if hover { 0.95 } else { 0.7 };
    ctx.set_source_rgba(0.7, 0.72, 0.78, alpha);
    ctx.set_line_width(1.5);

    let margin = size * 0.3;
    let mid_y = y + size / 2.0;
    ctx.move_to(x + margin, y + margin);
    ctx.line_to(x + size - margin, mid_y);
    ctx.line_to(x + margin, y + size - margin);
    let _ = ctx.stroke();
}

fn board_chip_label(snapshot: &crate::ui::toolbar::ToolbarSnapshot) -> String {
    let board_index = snapshot.board_index + 1;
    let board_count = snapshot.board_count.max(1);
    let name = snapshot.board_name.trim();
    let board_label = if board_count > 1 {
        if name.is_empty() {
            format!("Boards · B{}/{}", board_index, board_count)
        } else {
            format!("Boards · B{}/{} {}", board_index, board_count, name)
        }
    } else if name.is_empty() {
        "Boards".to_string()
    } else {
        format!("Boards · {}", name)
    };
    let pages = snapshot.page_count.max(1);
    if pages > 1 {
        format!("{board_label} · {pages}p")
    } else {
        board_label
    }
}

fn ellipsize_to_width(
    ctx: &cairo::Context,
    style: UiTextStyle<'_>,
    text: &str,
    max_width: f64,
) -> String {
    if max_width <= 0.0 {
        return String::new();
    }
    if text_layout(ctx, style, text, None).ink_extents().width() <= max_width {
        return text.to_string();
    }
    let mut chars: Vec<char> = text.chars().collect();
    while chars.len() > 3 {
        chars.pop();
        let candidate: String = chars.iter().collect();
        let candidate = format!("{candidate}...");
        if text_layout(ctx, style, &candidate, None)
            .ink_extents()
            .width()
            <= max_width
        {
            return candidate;
        }
    }
    "...".to_string()
}
