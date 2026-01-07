use super::super::widgets::constants::{
    COLOR_TEXT_PRIMARY, COLOR_TRACK_BACKGROUND, COLOR_TRACK_KNOB, SPACING_STD, set_color,
};
use super::super::widgets::*;
use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;

pub(super) fn draw_marker_opacity_section(layout: &mut SidePaletteLayout, y: &mut f64) {
    let ctx = layout.ctx;
    let snapshot = layout.snapshot;
    let hits = &mut layout.hits;
    let x = layout.x;
    let card_x = layout.card_x;
    let card_w = layout.card_w;
    let section_gap = layout.section_gap;
    let width = layout.width;

    let show_marker_opacity =
        snapshot.show_marker_opacity_section || snapshot.thickness_targets_marker;
    if !show_marker_opacity {
        return;
    }

    let slider_card_h = ToolbarLayoutSpec::SIDE_SLIDER_CARD_HEIGHT;
    let btn_size = ToolbarLayoutSpec::SIDE_NUDGE_SIZE;
    let nudge_icon_size = ToolbarLayoutSpec::SIDE_NUDGE_ICON_SIZE;
    let value_w = ToolbarLayoutSpec::SIDE_SLIDER_VALUE_WIDTH;
    let track_h = ToolbarLayoutSpec::SIDE_TRACK_HEIGHT;
    let knob_r = ToolbarLayoutSpec::SIDE_TRACK_KNOB_RADIUS;

    let marker_slider_row_y = *y + ToolbarLayoutSpec::SIDE_SLIDER_ROW_OFFSET;
    draw_group_card(ctx, card_x, *y, card_w, slider_card_h);
    draw_section_label(
        ctx,
        x,
        *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_Y,
        "Marker opacity",
    );

    let minus_x = x;
    draw_button(
        ctx,
        minus_x,
        marker_slider_row_y,
        btn_size,
        btn_size,
        false,
        false,
    );
    set_color(ctx, COLOR_TEXT_PRIMARY);
    toolbar_icons::draw_icon_minus(
        ctx,
        minus_x + (btn_size - nudge_icon_size) / 2.0,
        marker_slider_row_y + (btn_size - nudge_icon_size) / 2.0,
        nudge_icon_size,
    );
    hits.push(HitRegion {
        rect: (minus_x, marker_slider_row_y, btn_size, btn_size),
        event: ToolbarEvent::NudgeMarkerOpacity(-0.05),
        kind: HitKind::Click,
        tooltip: None,
    });

    let plus_x = width - x - btn_size - value_w - 4.0;
    draw_button(
        ctx,
        plus_x,
        marker_slider_row_y,
        btn_size,
        btn_size,
        false,
        false,
    );
    set_color(ctx, COLOR_TEXT_PRIMARY);
    toolbar_icons::draw_icon_plus(
        ctx,
        plus_x + (btn_size - nudge_icon_size) / 2.0,
        marker_slider_row_y + (btn_size - nudge_icon_size) / 2.0,
        nudge_icon_size,
    );
    hits.push(HitRegion {
        rect: (plus_x, marker_slider_row_y, btn_size, btn_size),
        event: ToolbarEvent::NudgeMarkerOpacity(0.05),
        kind: HitKind::Click,
        tooltip: None,
    });

    let track_x = minus_x + btn_size + SPACING_STD;
    let track_w = plus_x - track_x - SPACING_STD;
    let marker_track_y = marker_slider_row_y + (btn_size - track_h) / 2.0;
    let min_opacity = 0.05;
    let max_opacity = 0.9;
    let t = ((snapshot.marker_opacity - min_opacity) / (max_opacity - min_opacity)).clamp(0.0, 1.0);
    let knob_x = track_x + t * (track_w - knob_r * 2.0) + knob_r;

    set_color(ctx, COLOR_TRACK_BACKGROUND);
    draw_round_rect(ctx, track_x, marker_track_y, track_w, track_h, 4.0);
    let _ = ctx.fill();
    set_color(ctx, COLOR_TRACK_KNOB);
    ctx.arc(
        knob_x,
        marker_track_y + track_h / 2.0,
        knob_r,
        0.0,
        std::f64::consts::PI * 2.0,
    );
    let _ = ctx.fill();

    hits.push(HitRegion {
        rect: (track_x, marker_track_y - 6.0, track_w, track_h + 12.0),
        event: ToolbarEvent::SetMarkerOpacity(snapshot.marker_opacity),
        kind: HitKind::DragSetMarkerOpacity {
            min: min_opacity,
            max: max_opacity,
        },
        tooltip: None,
    });

    let opacity_text = format!("{:.0}%", snapshot.marker_opacity * 100.0);
    let value_x = width - x - value_w;
    draw_label_center(
        ctx,
        value_x,
        marker_slider_row_y,
        value_w,
        btn_size,
        &opacity_text,
    );

    *y += slider_card_h + section_gap;
}
