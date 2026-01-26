use super::super::widgets::constants::{
    COLOR_TRACK_BACKGROUND, COLOR_TRACK_KNOB, FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL, SPACING_STD,
    set_color,
};
use super::super::widgets::*;
use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::config::Action;
use crate::input::EraserMode;
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;
use crate::ui_text::UiTextStyle;

pub(super) fn draw_thickness_section(layout: &mut SidePaletteLayout, y: &mut f64) {
    let ctx = layout.ctx;
    let snapshot = layout.snapshot;
    let hits = &mut layout.hits;
    let hover = layout.hover;
    let x = layout.x;
    let card_x = layout.card_x;
    let card_w = layout.card_w;
    let content_width = layout.content_width;
    let section_gap = layout.section_gap;
    let width = layout.width;
    let label_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: FONT_SIZE_LABEL,
    };

    let slider_card_h = ToolbarLayoutSpec::SIDE_SLIDER_CARD_HEIGHT;
    draw_group_card(ctx, card_x, *y, card_w, slider_card_h);
    let thickness_label = if snapshot.thickness_targets_eraser {
        "Eraser size"
    } else {
        "Thickness"
    };
    draw_section_label(
        ctx,
        label_style,
        x,
        *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_Y,
        thickness_label,
    );

    let btn_size = ToolbarLayoutSpec::SIDE_NUDGE_SIZE;
    let nudge_icon_size = ToolbarLayoutSpec::SIDE_NUDGE_ICON_SIZE;
    let value_w = ToolbarLayoutSpec::SIDE_SLIDER_VALUE_WIDTH;
    let thickness_slider_row_y = *y + ToolbarLayoutSpec::SIDE_SLIDER_ROW_OFFSET;
    let track_h = ToolbarLayoutSpec::SIDE_TRACK_HEIGHT;
    let knob_r = ToolbarLayoutSpec::SIDE_TRACK_KNOB_RADIUS;
    let (min_thick, max_thick, nudge_step) = (1.0, 50.0, 1.0);

    let minus_x = x;
    let minus_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, minus_x, thickness_slider_row_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_button(
        ctx,
        minus_x,
        thickness_slider_row_y,
        btn_size,
        btn_size,
        false,
        minus_hover,
    );
    set_icon_color(ctx, minus_hover);
    toolbar_icons::draw_icon_minus(
        ctx,
        minus_x + (btn_size - nudge_icon_size) / 2.0,
        thickness_slider_row_y + (btn_size - nudge_icon_size) / 2.0,
        nudge_icon_size,
    );
    hits.push(HitRegion {
        rect: (minus_x, thickness_slider_row_y, btn_size, btn_size),
        event: ToolbarEvent::NudgeThickness(-nudge_step),
        kind: HitKind::Click,
        tooltip: Some("Decrease thickness".to_string()),
    });

    let plus_x = width - x - btn_size - value_w - 4.0;
    let plus_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, plus_x, thickness_slider_row_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_button(
        ctx,
        plus_x,
        thickness_slider_row_y,
        btn_size,
        btn_size,
        false,
        plus_hover,
    );
    set_icon_color(ctx, plus_hover);
    toolbar_icons::draw_icon_plus(
        ctx,
        plus_x + (btn_size - nudge_icon_size) / 2.0,
        thickness_slider_row_y + (btn_size - nudge_icon_size) / 2.0,
        nudge_icon_size,
    );
    hits.push(HitRegion {
        rect: (plus_x, thickness_slider_row_y, btn_size, btn_size),
        event: ToolbarEvent::NudgeThickness(nudge_step),
        kind: HitKind::Click,
        tooltip: Some("Increase thickness".to_string()),
    });

    let track_x = minus_x + btn_size + SPACING_STD;
    let track_w = plus_x - track_x - SPACING_STD;
    let thickness_track_y = thickness_slider_row_y + (btn_size - track_h) / 2.0;
    let t = ((snapshot.thickness - min_thick) / (max_thick - min_thick)).clamp(0.0, 1.0);
    let knob_x = track_x + t * (track_w - knob_r * 2.0) + knob_r;

    set_color(ctx, COLOR_TRACK_BACKGROUND);
    draw_round_rect(ctx, track_x, thickness_track_y, track_w, track_h, 4.0);
    let _ = ctx.fill();
    set_color(ctx, COLOR_TRACK_KNOB);
    ctx.arc(
        knob_x,
        thickness_track_y + track_h / 2.0,
        knob_r,
        0.0,
        std::f64::consts::PI * 2.0,
    );
    let _ = ctx.fill();

    hits.push(HitRegion {
        rect: (track_x, thickness_track_y - 6.0, track_w, track_h + 12.0),
        event: ToolbarEvent::SetThickness(snapshot.thickness),
        kind: HitKind::DragSetThickness {
            min: min_thick,
            max: max_thick,
        },
        tooltip: None,
    });

    let thickness_text = format!("{:.0}px", snapshot.thickness);
    let value_x = width - x - value_w;
    draw_label_center(
        ctx,
        label_style,
        value_x,
        thickness_slider_row_y,
        value_w,
        btn_size,
        &thickness_text,
    );
    *y += slider_card_h + section_gap;

    if snapshot.thickness_targets_eraser {
        let eraser_card_h = ToolbarLayoutSpec::SIDE_ERASER_MODE_CARD_HEIGHT;
        let toggle_h = ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT;
        let toggle_w = content_width;
        draw_group_card(ctx, card_x, *y, card_w, eraser_card_h);
        draw_section_label(
            ctx,
            label_style,
            x,
            *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
            "Eraser mode",
        );

        let toggle_y = *y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
        let toggle_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, toggle_y, toggle_w, toggle_h))
            .unwrap_or(false);
        let stroke_active = snapshot.eraser_mode == EraserMode::Stroke;
        draw_checkbox(
            ctx,
            x,
            toggle_y,
            toggle_w,
            toggle_h,
            stroke_active,
            toggle_hover,
            label_style,
            "Erase by stroke",
        );
        let toggle_tooltip = format_binding_label(
            "Erase by stroke",
            snapshot
                .binding_hints
                .binding_for_action(Action::ToggleEraserMode),
        );
        hits.push(HitRegion {
            rect: (x, toggle_y, toggle_w, toggle_h),
            event: ToolbarEvent::SetEraserMode(if stroke_active {
                EraserMode::Brush
            } else {
                EraserMode::Stroke
            }),
            kind: HitKind::Click,
            tooltip: Some(toggle_tooltip),
        });
        *y += eraser_card_h + section_gap;
    }
}
