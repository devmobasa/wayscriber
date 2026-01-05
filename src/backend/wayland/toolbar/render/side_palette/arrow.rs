use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::input::Tool;
use crate::ui::toolbar::ToolbarEvent;

use super::super::widgets::*;

pub(super) fn draw_arrow_section(layout: &mut SidePaletteLayout, y: &mut f64) {
    let ctx = layout.ctx;
    let snapshot = layout.snapshot;
    let hits = &mut layout.hits;
    let hover = layout.hover;
    let x = layout.x;
    let card_x = layout.card_x;
    let card_w = layout.card_w;
    let content_width = layout.content_width;
    let section_gap = layout.section_gap;

    let show_arrow_controls = snapshot.active_tool == Tool::Arrow || snapshot.arrow_label_enabled;
    if !show_arrow_controls {
        return;
    }

    let card_h = ToolbarLayoutSpec::SIDE_TOGGLE_CARD_HEIGHT;
    draw_group_card(ctx, card_x, *y, card_w, card_h);
    draw_section_label(
        ctx,
        x,
        *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
        "Arrow",
    );

    let toggle_h = ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT;
    let toggle_y = *y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let toggle_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, toggle_y, content_width, toggle_h))
        .unwrap_or(false);
    draw_checkbox(
        ctx,
        x,
        toggle_y,
        content_width,
        toggle_h,
        snapshot.arrow_label_enabled,
        toggle_hover,
        "Numbered arrows",
    );
    hits.push(HitRegion {
        rect: (x, toggle_y, content_width, toggle_h),
        event: ToolbarEvent::ToggleArrowLabels(!snapshot.arrow_label_enabled),
        kind: HitKind::Click,
        tooltip: Some("Auto-label arrows 1, 2, 3.".to_string()),
    });

    *y += card_h + section_gap;
}
