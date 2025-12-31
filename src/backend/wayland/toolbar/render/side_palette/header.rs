use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
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
    let mode_label = match snapshot.layout_mode {
        crate::config::ToolbarLayoutMode::Simple => "Mode: S",
        crate::config::ToolbarLayoutMode::Regular => "Mode: R",
        crate::config::ToolbarLayoutMode::Advanced => "Mode: A",
    };
    draw_label_center(ctx, mode_x, header_y, mode_w, icons_h, mode_label);
    let next_mode = snapshot.layout_mode.next();
    let mode_tooltip = format!(
        "Mode: S/R/A = {}/{}/{}",
        crate::config::ToolbarLayoutMode::Simple.label(),
        crate::config::ToolbarLayoutMode::Regular.label(),
        crate::config::ToolbarLayoutMode::Advanced.label(),
    );
    hits.push(HitRegion {
        rect: (mode_x, header_y, mode_w, icons_h),
        event: ToolbarEvent::SetToolbarLayoutMode(next_mode),
        kind: HitKind::Click,
        tooltip: Some(mode_tooltip),
    });

    let (pin_x, close_x, header_btn_y) = spec.side_header_button_positions(width);
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

    spec.side_content_start_y()
}
