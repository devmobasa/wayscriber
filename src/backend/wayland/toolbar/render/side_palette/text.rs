use super::super::widgets::constants::{
    COLOR_TEXT_PRIMARY, COLOR_TRACK_BACKGROUND, COLOR_TRACK_KNOB, FONT_FAMILY_DEFAULT,
    FONT_SIZE_LABEL, SPACING_STD, set_color,
};
use super::super::widgets::*;
use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::draw::FontDescriptor;
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;
use crate::ui_text::UiTextStyle;

pub(super) fn draw_text_controls_section(layout: &mut SidePaletteLayout, y: &mut f64) {
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

    let show_text_controls =
        snapshot.text_active || snapshot.note_active || snapshot.show_text_controls;
    if !show_text_controls {
        return;
    }

    let slider_card_h = ToolbarLayoutSpec::SIDE_SLIDER_CARD_HEIGHT;
    let btn_size = ToolbarLayoutSpec::SIDE_NUDGE_SIZE;
    let nudge_icon_size = ToolbarLayoutSpec::SIDE_NUDGE_ICON_SIZE;
    let value_w = ToolbarLayoutSpec::SIDE_SLIDER_VALUE_WIDTH;
    let track_h = ToolbarLayoutSpec::SIDE_TRACK_HEIGHT;
    let knob_r = ToolbarLayoutSpec::SIDE_TRACK_KNOB_RADIUS;

    draw_group_card(ctx, card_x, *y, card_w, slider_card_h);
    draw_section_label(
        ctx,
        label_style,
        x,
        *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_Y,
        "Text size",
    );

    let fs_min = 8.0;
    let fs_max = 72.0;
    let fs_slider_row_y = *y + ToolbarLayoutSpec::SIDE_SLIDER_ROW_OFFSET;

    let fs_minus_x = x;
    draw_button(
        ctx,
        fs_minus_x,
        fs_slider_row_y,
        btn_size,
        btn_size,
        false,
        false,
    );
    set_color(ctx, COLOR_TEXT_PRIMARY);
    toolbar_icons::draw_icon_minus(
        ctx,
        fs_minus_x + (btn_size - nudge_icon_size) / 2.0,
        fs_slider_row_y + (btn_size - nudge_icon_size) / 2.0,
        nudge_icon_size,
    );
    hits.push(HitRegion {
        rect: (fs_minus_x, fs_slider_row_y, btn_size, btn_size),
        event: ToolbarEvent::SetFontSize((snapshot.font_size - 2.0).max(fs_min)),
        kind: HitKind::Click,
        tooltip: None,
    });

    let fs_plus_x = width - x - btn_size - value_w - 4.0;
    draw_button(
        ctx,
        fs_plus_x,
        fs_slider_row_y,
        btn_size,
        btn_size,
        false,
        false,
    );
    set_color(ctx, COLOR_TEXT_PRIMARY);
    toolbar_icons::draw_icon_plus(
        ctx,
        fs_plus_x + (btn_size - nudge_icon_size) / 2.0,
        fs_slider_row_y + (btn_size - nudge_icon_size) / 2.0,
        nudge_icon_size,
    );
    hits.push(HitRegion {
        rect: (fs_plus_x, fs_slider_row_y, btn_size, btn_size),
        event: ToolbarEvent::SetFontSize((snapshot.font_size + 2.0).min(fs_max)),
        kind: HitKind::Click,
        tooltip: None,
    });

    let fs_track_x = fs_minus_x + btn_size + SPACING_STD;
    let fs_track_w = fs_plus_x - fs_track_x - SPACING_STD;
    let fs_track_y = fs_slider_row_y + (btn_size - track_h) / 2.0;
    let fs_t = ((snapshot.font_size - fs_min) / (fs_max - fs_min)).clamp(0.0, 1.0);
    let fs_knob_x = fs_track_x + fs_t * (fs_track_w - knob_r * 2.0) + knob_r;

    set_color(ctx, COLOR_TRACK_BACKGROUND);
    draw_round_rect(ctx, fs_track_x, fs_track_y, fs_track_w, track_h, 4.0);
    let _ = ctx.fill();
    set_color(ctx, COLOR_TRACK_KNOB);
    ctx.arc(
        fs_knob_x,
        fs_track_y + track_h / 2.0,
        knob_r,
        0.0,
        std::f64::consts::PI * 2.0,
    );
    let _ = ctx.fill();

    hits.push(HitRegion {
        rect: (fs_track_x, fs_track_y - 6.0, fs_track_w, track_h + 12.0),
        event: ToolbarEvent::SetFontSize(snapshot.font_size),
        kind: HitKind::DragSetFontSize,
        tooltip: None,
    });

    let fs_text = format!("{:.0}pt", snapshot.font_size);
    draw_label_center(
        ctx,
        label_style,
        width - x - value_w,
        fs_slider_row_y,
        value_w,
        btn_size,
        &fs_text,
    );

    *y += slider_card_h + section_gap;

    let font_card_h = ToolbarLayoutSpec::SIDE_FONT_CARD_HEIGHT;
    draw_group_card(ctx, card_x, *y, card_w, font_card_h);
    draw_section_label(
        ctx,
        label_style,
        x,
        *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
        "Font",
    );

    let font_btn_h = ToolbarLayoutSpec::SIDE_FONT_BUTTON_HEIGHT;
    let font_gap = ToolbarLayoutSpec::SIDE_FONT_BUTTON_GAP;
    let font_btn_w = (content_width - font_gap) / 2.0;
    let fonts = [
        FontDescriptor::new("Sans".to_string(), "bold".to_string(), "normal".to_string()),
        FontDescriptor::new(
            "Monospace".to_string(),
            "normal".to_string(),
            "normal".to_string(),
        ),
    ];
    let mut fx = x;
    let fy = *y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    for font in fonts {
        let is_active = font.family == snapshot.font.family;
        let font_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, fx, fy, font_btn_w, font_btn_h))
            .unwrap_or(false);
        draw_button(ctx, fx, fy, font_btn_w, font_btn_h, is_active, font_hover);
        draw_label_left(
            ctx,
            label_style,
            fx + 8.0,
            fy,
            font_btn_w,
            font_btn_h,
            &font.family,
        );
        hits.push(HitRegion {
            rect: (fx, fy, font_btn_w, font_btn_h),
            event: ToolbarEvent::SetFont(font.clone()),
            kind: HitKind::Click,
            tooltip: None,
        });
        fx += font_btn_w + font_gap;
    }

    *y += font_card_h + section_gap;
}
