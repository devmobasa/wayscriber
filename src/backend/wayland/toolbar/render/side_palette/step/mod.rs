mod custom_rows;
mod delay_sliders;

use super::super::widgets::*;
use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::input::ToolbarDrawerTab;
use crate::ui::toolbar::ToolbarEvent;

pub(super) fn draw_step_section(layout: &mut SidePaletteLayout, y: &mut f64) {
    let ctx = layout.ctx;
    let snapshot = layout.snapshot;
    let hits = &mut layout.hits;
    let hover = layout.hover;
    let x = layout.x;
    let card_x = layout.card_x;
    let card_w = layout.card_w;
    let content_width = layout.content_width;
    let section_gap = layout.section_gap;

    if !snapshot.show_step_section
        || !snapshot.drawer_open
        || snapshot.drawer_tab != ToolbarDrawerTab::App
    {
        return;
    }

    let custom_toggle_h = ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT;
    let toggle_gap = ToolbarLayoutSpec::SIDE_TOGGLE_GAP;
    let toggles_h = custom_toggle_h * 2.0 + toggle_gap;
    let custom_content_h = if snapshot.custom_section_enabled {
        ToolbarLayoutSpec::SIDE_CUSTOM_SECTION_HEIGHT
    } else {
        0.0
    };
    let delay_sliders_h = if snapshot.show_delay_sliders {
        ToolbarLayoutSpec::SIDE_DELAY_SECTION_HEIGHT
    } else {
        0.0
    };
    let custom_card_h =
        ToolbarLayoutSpec::SIDE_STEP_HEADER_HEIGHT + toggles_h + custom_content_h + delay_sliders_h;
    draw_group_card(ctx, card_x, *y, card_w, custom_card_h);
    draw_section_label(
        ctx,
        x,
        *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
        "Step Undo/Redo",
    );

    let custom_toggle_y = *y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let toggle_w = content_width;

    let step_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, custom_toggle_y, toggle_w, custom_toggle_h))
        .unwrap_or(false);
    draw_checkbox(
        ctx,
        x,
        custom_toggle_y,
        toggle_w,
        custom_toggle_h,
        snapshot.custom_section_enabled,
        step_hover,
        "Step controls",
    );
    hits.push(HitRegion {
        rect: (x, custom_toggle_y, toggle_w, custom_toggle_h),
        event: ToolbarEvent::ToggleCustomSection(!snapshot.custom_section_enabled),
        kind: HitKind::Click,
        tooltip: Some("Step controls: multi-step undo/redo.".to_string()),
    });

    let delay_toggle_y = custom_toggle_y + custom_toggle_h + toggle_gap;
    let delay_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, delay_toggle_y, toggle_w, custom_toggle_h))
        .unwrap_or(false);
    draw_checkbox(
        ctx,
        x,
        delay_toggle_y,
        toggle_w,
        custom_toggle_h,
        snapshot.show_delay_sliders,
        delay_hover,
        "Delay sliders",
    );
    hits.push(HitRegion {
        rect: (x, delay_toggle_y, toggle_w, custom_toggle_h),
        event: ToolbarEvent::ToggleDelaySliders(!snapshot.show_delay_sliders),
        kind: HitKind::Click,
        tooltip: Some("Delay sliders: undo/redo delays.".to_string()),
    });

    let custom_y = delay_toggle_y + custom_toggle_h + toggle_gap;

    if snapshot.custom_section_enabled {
        custom_rows::draw_custom_rows(ctx, hits, x, custom_y, card_w, snapshot, hover);
    }

    if snapshot.show_delay_sliders {
        let slider_start_y = *y
            + ToolbarLayoutSpec::SIDE_STEP_HEADER_HEIGHT
            + toggles_h
            + custom_content_h
            + ToolbarLayoutSpec::SIDE_STEP_SLIDER_TOP_PADDING;
        delay_sliders::draw_delay_sliders(ctx, hits, x, slider_start_y, content_width, snapshot);
    }

    *y += custom_card_h + section_gap;
}
