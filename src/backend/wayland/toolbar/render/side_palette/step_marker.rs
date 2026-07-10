use super::super::widgets::constants::{
    COLOR_LABEL_HINT, FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL, FONT_SIZE_SMALL, set_color,
};
use super::super::widgets::*;
use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::ui::toolbar::{ToolContext, ToolbarEvent, ToolbarSideSection};
use crate::ui_text::{UiTextStyle, text_layout};

use super::section_header::draw_collapsible_header;

pub(super) fn draw_step_marker_section(layout: &mut SidePaletteLayout, y: &mut f64) {
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
    let hint_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: FONT_SIZE_SMALL,
    };

    if snapshot.side_section_hidden(ToolbarSideSection::StepMarkers)
        || !ToolContext::from_snapshot(snapshot).show_step_counter
    {
        return;
    }

    let card_h = layout.spec.side_step_markers_height(snapshot);
    draw_group_card(ctx, card_x, *y, card_w, card_h);
    draw_collapsible_header(
        layout,
        *y,
        label_style,
        ToolbarSideSection::StepMarkers,
        "Step markers",
        ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
    );
    let hint = format!("Next: {}", snapshot.step_marker_next);
    let layout_text = text_layout(ctx, hint_style, &hint, None);
    let ext = layout_text.ink_extents();
    let chevron_reserve = ToolbarLayoutSpec::SIDE_COLLAPSE_CHEVRON_SIZE + 10.0;
    let hint_x = card_x + card_w - ext.width() - chevron_reserve - ext.x_bearing();
    let hint_y = *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL;
    set_color(ctx, COLOR_LABEL_HINT);
    layout_text.show_at_baseline(ctx, hint_x, hint_y);
    if snapshot.side_section_collapsed(ToolbarSideSection::StepMarkers) {
        *y += card_h + section_gap;
        return;
    }

    let hits = &mut layout.hits;
    let reset_y = *y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let reset_h = ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT;
    let reset_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, reset_y, content_width, reset_h))
        .unwrap_or(false);
    draw_button(ctx, x, reset_y, content_width, reset_h, false, reset_hover);
    draw_label_center(
        ctx,
        label_style,
        x,
        reset_y,
        content_width,
        reset_h,
        "Reset",
    );
    hits.push(HitRegion {
        focus_id: None,
        rect: (x, reset_y, content_width, reset_h),
        event: ToolbarEvent::ResetStepMarkerCounter,
        kind: HitKind::Click,
        tooltip: Some("Reset numbering to 1.".to_string()),
    });

    *y += card_h + section_gap;
}
