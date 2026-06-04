use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSideSection};
use crate::ui_text::UiTextStyle;

use super::super::widgets::constants::{COLOR_LABEL_SECTION, set_color};
use super::super::widgets::{draw_section_label, point_in_rect};

pub(super) fn draw_collapsible_header(
    layout: &mut SidePaletteLayout,
    y: f64,
    label_style: UiTextStyle<'static>,
    section: ToolbarSideSection,
    label: &str,
    label_offset_y: f64,
) {
    let collapsed = layout.snapshot.side_section_collapsed(section);
    let hit_h = ToolbarLayoutSpec::SIDE_COLLAPSE_HEADER_HIT_HEIGHT;
    let hover = layout
        .hover
        .map(|(hx, hy)| point_in_rect(hx, hy, layout.card_x, y, layout.card_w, hit_h))
        .unwrap_or(false);

    draw_section_label(layout.ctx, label_style, layout.x, y + label_offset_y, label);

    let size = ToolbarLayoutSpec::SIDE_COLLAPSE_CHEVRON_SIZE;
    let chevron_x = layout.x + layout.content_width - size;
    let chevron_y = y + (hit_h - size) / 2.0;
    draw_header_chevron(layout.ctx, chevron_x, chevron_y, size, collapsed, hover);

    layout.hits.push(HitRegion {
        rect: (layout.card_x, y, layout.card_w, hit_h),
        event: ToolbarEvent::ToggleSideSectionCollapsed(section, !collapsed),
        kind: HitKind::Click,
        tooltip: Some(format!(
            "{} {}",
            if collapsed { "Expand" } else { "Collapse" },
            label
        )),
    });
}

fn draw_header_chevron(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    collapsed: bool,
    hover: bool,
) {
    set_color(
        ctx,
        (
            COLOR_LABEL_SECTION.0,
            COLOR_LABEL_SECTION.1,
            COLOR_LABEL_SECTION.2,
            if hover { 1.0 } else { COLOR_LABEL_SECTION.3 },
        ),
    );
    ctx.set_line_width(1.6);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);
    let margin = size * 0.28;
    if collapsed {
        let mid_y = y + size / 2.0;
        ctx.move_to(x + margin, y + margin);
        ctx.line_to(x + size - margin, mid_y);
        ctx.line_to(x + margin, y + size - margin);
    } else {
        let mid_x = x + size / 2.0;
        ctx.move_to(x + margin, y + margin);
        ctx.line_to(mid_x, y + size - margin);
        ctx.line_to(x + size - margin, y + margin);
    }
    let _ = ctx.stroke();
}
