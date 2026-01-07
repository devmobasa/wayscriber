use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;

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
    draw_label_center(ctx, mode_x, header_y, mode_w, icons_h, mode_label);
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
    // Draw attention dot when drawer is closed
    if !snapshot.drawer_open {
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

    // Draw onboarding hint for the "More" button (first-time users)
    if snapshot.show_drawer_hint {
        draw_onboarding_hint(ctx, more_x, header_btn_y, btn_size);
    }

    spec.side_content_start_y()
}

/// Draws a floating onboarding hint pointing to the More button.
fn draw_onboarding_hint(ctx: &cairo::Context, more_x: f64, more_y: f64, btn_size: f64) {
    let hint_text = "More options here!";
    let padding_h = 8.0;
    let padding_v = 6.0;
    let arrow_size = 6.0;
    let corner_radius = 4.0;

    ctx.set_font_size(12.0);
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    let Ok(extents) = ctx.text_extents(hint_text) else {
        return;
    };

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
    let text_x = box_x + padding_h;
    let text_y = box_y + padding_v + extents.height();
    ctx.move_to(text_x, text_y);
    let _ = ctx.show_text(hint_text);
}
