use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::ui::toolbar::model::{ToolbarSliderSpec, delay_secs_from_t, delay_t_from_ms};
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot};
use crate::ui_text::{UiTextStyle, draw_text_baseline};

use super::super::super::widgets::constants::{
    FONT_FAMILY_DEFAULT, FONT_SIZE_SECONDARY, set_color,
};
use super::super::super::widgets::draw_round_rect;
use super::{COLOR_DELAY_KNOB, COLOR_DELAY_TRACK};
use crate::ui::theme::Rgba;

/// Slider label text: COLOR_LABEL_HINT gray at +0.1 alpha — kept to avoid
/// dimming these labels.
const COLOR_SLIDER_LABEL: Rgba = (0.7, 0.7, 0.75, 0.9);

pub(super) fn draw_delay_sliders(
    ctx: &cairo::Context,
    hits: &mut Vec<HitRegion>,
    x: f64,
    slider_start_y: f64,
    sliders_w: f64,
    snapshot: &ToolbarSnapshot,
) {
    let slider_h = ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HEIGHT;
    let slider_knob_r = ToolbarLayoutSpec::SIDE_DELAY_SLIDER_KNOB_RADIUS;
    let label_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: FONT_SIZE_SECONDARY,
    };

    let undo_label = format!(
        "Undo delay: {:.1}s",
        snapshot.undo_all_delay_ms as f64 / 1000.0
    );
    set_color(ctx, COLOR_SLIDER_LABEL);
    draw_text_baseline(
        ctx,
        label_style,
        &undo_label,
        x,
        slider_start_y + 10.0,
        None,
    );

    let undo_slider_y = slider_start_y + ToolbarLayoutSpec::SIDE_DELAY_SLIDER_UNDO_OFFSET_Y;
    set_color(ctx, COLOR_DELAY_TRACK);
    draw_round_rect(ctx, x, undo_slider_y, sliders_w, slider_h, 3.0);
    let _ = ctx.fill();
    let undo_t = delay_t_from_ms(snapshot.undo_all_delay_ms);
    let undo_knob_x = ToolbarSliderSpec::DELAY_SECONDS.knob_center_x(
        x,
        sliders_w,
        slider_knob_r,
        snapshot.undo_all_delay_ms as f64 / 1000.0,
    );
    set_color(ctx, COLOR_DELAY_KNOB);
    ctx.arc(
        undo_knob_x,
        undo_slider_y + slider_h / 2.0,
        slider_knob_r,
        0.0,
        std::f64::consts::PI * 2.0,
    );
    let _ = ctx.fill();
    let hit_pad = ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HIT_PADDING;
    hits.push(HitRegion {
        focus_id: None,
        rect: (
            x,
            undo_slider_y - hit_pad,
            sliders_w,
            slider_h + hit_pad * 2.0,
        ),
        event: ToolbarEvent::SetUndoDelay(delay_secs_from_t(undo_t)),
        kind: HitKind::DragUndoDelay,
        tooltip: Some(format!(
            "Undo-all delay: {:.1}s (drag)",
            snapshot.undo_all_delay_ms as f64 / 1000.0
        )),
    });

    let redo_label = format!(
        "Redo delay: {:.1}s",
        snapshot.redo_all_delay_ms as f64 / 1000.0
    );
    set_color(ctx, COLOR_SLIDER_LABEL);
    draw_text_baseline(
        ctx,
        label_style,
        &redo_label,
        x + sliders_w / 2.0 + 10.0,
        slider_start_y + 10.0,
        None,
    );

    let redo_slider_y = slider_start_y + ToolbarLayoutSpec::SIDE_DELAY_SLIDER_REDO_OFFSET_Y;
    set_color(ctx, COLOR_DELAY_TRACK);
    draw_round_rect(ctx, x, redo_slider_y, sliders_w, slider_h, 3.0);
    let _ = ctx.fill();
    let redo_t = delay_t_from_ms(snapshot.redo_all_delay_ms);
    let redo_knob_x = ToolbarSliderSpec::DELAY_SECONDS.knob_center_x(
        x,
        sliders_w,
        slider_knob_r,
        snapshot.redo_all_delay_ms as f64 / 1000.0,
    );
    set_color(ctx, COLOR_DELAY_KNOB);
    ctx.arc(
        redo_knob_x,
        redo_slider_y + slider_h / 2.0,
        slider_knob_r,
        0.0,
        std::f64::consts::PI * 2.0,
    );
    let _ = ctx.fill();
    hits.push(HitRegion {
        focus_id: None,
        rect: (
            x,
            redo_slider_y - hit_pad,
            sliders_w,
            slider_h + hit_pad * 2.0,
        ),
        event: ToolbarEvent::SetRedoDelay(delay_secs_from_t(redo_t)),
        kind: HitKind::DragRedoDelay,
        tooltip: Some(format!(
            "Redo-all delay: {:.1}s (drag)",
            snapshot.redo_all_delay_ms as f64 / 1000.0
        )),
    });
}
