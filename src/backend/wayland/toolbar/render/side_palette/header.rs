use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;
use crate::ui_text::{UiTextStyle, text_layout};

use super::super::widgets::constants::{FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL};
use super::super::widgets::*;

pub(super) fn draw_header(layout: &mut SidePaletteLayout) -> f64 {
    let ctx = layout.ctx;
    let snapshot = layout.snapshot;
    let hits = &mut layout.hits;
    let hover = layout.hover;
    let spec = &layout.spec;
    let x = layout.x;
    let y = ToolbarLayoutSpec::SIDE_TOP_PADDING;
    let width = layout.width;
    let label_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: FONT_SIZE_LABEL,
    };

    let btn_size = ToolbarLayoutSpec::SIDE_HEADER_BUTTON_SIZE;
    let handle_w = ToolbarLayoutSpec::SIDE_HEADER_HANDLE_SIZE;
    let handle_h = ToolbarLayoutSpec::SIDE_HEADER_HANDLE_SIZE;

    // Place handle above the header row to avoid widening the palette.
    let handle_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, y, handle_w, handle_h))
        .unwrap_or(false);
    draw_drag_handle(ctx, x, y, handle_w, handle_h, handle_hover);
    hits.push(HitRegion {
        rect: (x, y, handle_w, handle_h),
        event: ToolbarEvent::MoveSideToolbar { x: 0.0, y: 0.0 },
        kind: HitKind::DragMoveSide,
        tooltip: Some("Drag toolbar".to_string()),
    });

    let header_y = spec.side_header_y();
    let icons_w = ToolbarLayoutSpec::SIDE_HEADER_TOGGLE_WIDTH;
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
        snapshot.use_icons,
        icons_hover,
        label_style,
        "Icons",
    );
    hits.push(HitRegion {
        rect: (x, header_y, icons_w, icons_h),
        event: ToolbarEvent::ToggleIconMode(!snapshot.use_icons),
        kind: HitKind::Click,
        tooltip: None,
    });

    let mode_w = ToolbarLayoutSpec::SIDE_HEADER_MODE_WIDTH;
    let mode_x = x + icons_w + ToolbarLayoutSpec::SIDE_HEADER_MODE_GAP;
    let mode_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, mode_x, header_y, mode_w, icons_h))
        .unwrap_or(false);
    draw_button(ctx, mode_x, header_y, mode_w, icons_h, false, mode_hover);
    let (mode_label, next_mode) = match snapshot.layout_mode {
        crate::config::ToolbarLayoutMode::Simple => {
            ("Mode: S", crate::config::ToolbarLayoutMode::Regular)
        }
        crate::config::ToolbarLayoutMode::Regular | crate::config::ToolbarLayoutMode::Advanced => {
            ("Mode: F", crate::config::ToolbarLayoutMode::Simple)
        }
    };
    draw_label_center(
        ctx,
        label_style,
        mode_x,
        header_y,
        mode_w,
        icons_h,
        mode_label,
    );
    let mode_tooltip = "Mode: Simple/Full".to_string();
    hits.push(HitRegion {
        rect: (mode_x, header_y, mode_w, icons_h),
        event: ToolbarEvent::SetToolbarLayoutMode(next_mode),
        kind: HitKind::Click,
        tooltip: Some(mode_tooltip),
    });

    let (more_x, pin_x, close_x, header_btn_y) = spec.side_header_button_positions(width);
    let more_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, more_x, header_btn_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_button(
        ctx,
        more_x,
        header_btn_y,
        btn_size,
        btn_size,
        snapshot.drawer_open,
        more_hover,
    );
    set_icon_color(ctx, more_hover);
    let icon_size = ToolbarLayoutSpec::SIDE_ACTION_ICON_SIZE;
    let icon_x = more_x + (btn_size - icon_size) / 2.0;
    let icon_y = header_btn_y + (btn_size - icon_size) / 2.0;
    toolbar_icons::draw_icon_more(ctx, icon_x, icon_y, icon_size);
    // Draw attention dot only for the onboarding hint
    if snapshot.show_drawer_hint {
        let dot_radius = 4.0;
        let dot_x = more_x + btn_size - dot_radius - 2.0;
        let dot_y = header_btn_y + dot_radius + 2.0;
        ctx.arc(dot_x, dot_y, dot_radius, 0.0, std::f64::consts::TAU);
        ctx.set_source_rgba(0.95, 0.45, 0.15, 0.95); // Orange attention color
        let _ = ctx.fill();
    }
    hits.push(HitRegion {
        rect: (more_x, header_btn_y, btn_size, btn_size),
        event: ToolbarEvent::ToggleDrawer(!snapshot.drawer_open),
        kind: HitKind::Click,
        tooltip: Some("More (Canvas/Settings)".to_string()),
    });
    let pin_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, pin_x, header_btn_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_pin_button(
        ctx,
        pin_x,
        header_btn_y,
        btn_size,
        snapshot.side_pinned,
        pin_hover,
    );
    hits.push(HitRegion {
        rect: (pin_x, header_btn_y, btn_size, btn_size),
        event: ToolbarEvent::PinSideToolbar(!snapshot.side_pinned),
        kind: HitKind::Click,
        tooltip: Some(if snapshot.side_pinned {
            "Unpin".to_string()
        } else {
            "Pin".to_string()
        }),
    });

    let close_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, close_x, header_btn_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_close_button(ctx, close_x, header_btn_y, btn_size, close_hover);
    hits.push(HitRegion {
        rect: (close_x, header_btn_y, btn_size, btn_size),
        event: ToolbarEvent::CloseSideToolbar,
        kind: HitKind::Click,
        tooltip: Some("Close".to_string()),
    });

    let chip_y = spec.side_header_board_y();
    let chip_h = ToolbarLayoutSpec::SIDE_HEADER_BOARD_ROW_HEIGHT;
    let chip_w = layout.content_width;
    let chip_x = x;
    let chip_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, chip_x, chip_y, chip_w, chip_h))
        .unwrap_or(false);
    let chip_bg = if chip_hover { 0.28 } else { 0.22 };
    draw_round_rect(ctx, chip_x, chip_y, chip_w, chip_h, 8.0);
    ctx.set_source_rgba(chip_bg, chip_bg + 0.02, chip_bg + 0.06, 0.95);
    let _ = ctx.fill();
    ctx.set_source_rgba(0.08, 0.1, 0.13, 0.7);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    let dot_size = ToolbarLayoutSpec::SIDE_BOARD_COLOR_DOT_SIZE;
    let dot_x = chip_x + 8.0;
    let dot_y = chip_y + (chip_h - dot_size) * 0.5;
    if let Some(color) = snapshot.board_color {
        draw_swatch(ctx, dot_x, dot_y, dot_size, color, false);
    } else {
        ctx.set_source_rgba(0.62, 0.68, 0.76, 0.7);
        draw_round_rect(ctx, dot_x, dot_y, dot_size, dot_size, 3.0);
        let _ = ctx.stroke();
        ctx.move_to(dot_x, dot_y);
        ctx.line_to(dot_x + dot_size, dot_y + dot_size);
        ctx.move_to(dot_x + dot_size, dot_y);
        ctx.line_to(dot_x, dot_y + dot_size);
        let _ = ctx.stroke();
    }

    let label_x = dot_x + dot_size + 8.0;
    let label_w = chip_x + chip_w - 8.0 - label_x;
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
        tooltip: Some("Boards".to_string()),
    });

    // Draw onboarding hint for the "More" button (first-time users)
    if snapshot.show_drawer_hint {
        draw_onboarding_hint(ctx, more_x, header_btn_y, btn_size);
    }

    spec.side_content_start_y()
}

fn board_chip_label(snapshot: &crate::ui::toolbar::ToolbarSnapshot) -> String {
    let board_index = snapshot.board_index + 1;
    let board_count = snapshot.board_count.max(1);
    let name = snapshot.board_name.trim();
    let board_label = if board_count > 1 {
        if name.is_empty() {
            format!("B{}/{}", board_index, board_count)
        } else {
            format!("B{}/{} {}", board_index, board_count, name)
        }
    } else if name.is_empty() {
        "Board".to_string()
    } else {
        format!("Board {}", name)
    };
    let pages = snapshot.page_count.max(1);
    if pages > 1 {
        format!("{board_label} - {pages}p")
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

/// Draws a floating onboarding hint pointing to the More button.
fn draw_onboarding_hint(ctx: &cairo::Context, more_x: f64, more_y: f64, btn_size: f64) {
    let hint_text = "More options here!";
    let padding_h = 8.0;
    let padding_v = 6.0;
    let arrow_size = 6.0;
    let corner_radius = 4.0;
    let hint_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 12.0,
    };

    let layout = text_layout(ctx, hint_style, hint_text, None);
    let extents = layout.ink_extents();
    let box_w = extents.width() + padding_h * 2.0;
    let box_h = extents.height() + padding_v * 2.0;

    // Position: below and to the left of the More button
    let box_x = more_x + btn_size / 2.0 - box_w / 2.0;
    let box_y = more_y + btn_size + arrow_size + 4.0;

    // Draw arrow pointing up
    let arrow_x = more_x + btn_size / 2.0;
    let arrow_y = box_y - arrow_size;
    ctx.move_to(arrow_x, arrow_y);
    ctx.line_to(arrow_x - arrow_size, box_y);
    ctx.line_to(arrow_x + arrow_size, box_y);
    ctx.close_path();
    ctx.set_source_rgba(0.95, 0.45, 0.15, 0.95);
    let _ = ctx.fill();

    // Draw rounded rectangle background
    let x = box_x;
    let y = box_y;
    let w = box_w;
    let h = box_h;
    let r = corner_radius;
    ctx.new_path();
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
        3.0 * std::f64::consts::FRAC_PI_2,
    );
    ctx.close_path();
    ctx.set_source_rgba(0.95, 0.45, 0.15, 0.95);
    let _ = ctx.fill();

    // Draw text
    ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    let text_x = box_x + padding_h - extents.x_bearing();
    let text_y = box_y + padding_v - extents.y_bearing();
    layout.show_at_baseline(ctx, text_x, text_y);
}
