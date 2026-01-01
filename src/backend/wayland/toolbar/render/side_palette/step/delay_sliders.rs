use crate::backend::wayland::toolbar::events::{HitKind, delay_secs_from_t, delay_t_from_ms};
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot};

use super::super::super::widgets::draw_round_rect;

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

    let undo_label = format!(
        "Undo delay: {:.1}s",
        snapshot.undo_all_delay_ms as f64 / 1000.0
    );
    ctx.set_source_rgba(0.7, 0.7, 0.75, 0.9);
    ctx.set_font_size(11.0);
    ctx.move_to(x, slider_start_y + 10.0);
    let _ = ctx.show_text(&undo_label);

    let undo_slider_y = slider_start_y + ToolbarLayoutSpec::SIDE_DELAY_SLIDER_UNDO_OFFSET_Y;
    ctx.set_source_rgba(0.4, 0.4, 0.45, 0.7);
    draw_round_rect(ctx, x, undo_slider_y, sliders_w, slider_h, 3.0);
    let _ = ctx.fill();
    let undo_t = delay_t_from_ms(snapshot.undo_all_delay_ms);
    let undo_knob_x = x + undo_t * (sliders_w - slider_knob_r * 2.0) + slider_knob_r;
    ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
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
    ctx.set_source_rgba(0.7, 0.7, 0.75, 0.9);
    ctx.move_to(x + sliders_w / 2.0 + 10.0, slider_start_y + 10.0);
    let _ = ctx.show_text(&redo_label);

    let redo_slider_y = slider_start_y + ToolbarLayoutSpec::SIDE_DELAY_SLIDER_REDO_OFFSET_Y;
    ctx.set_source_rgba(0.4, 0.4, 0.45, 0.7);
    draw_round_rect(ctx, x, redo_slider_y, sliders_w, slider_h, 3.0);
    let _ = ctx.fill();
    let redo_t = delay_t_from_ms(snapshot.redo_all_delay_ms);
    let redo_knob_x = x + redo_t * (sliders_w - slider_knob_r * 2.0) + slider_knob_r;
    ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
    ctx.arc(
        redo_knob_x,
        redo_slider_y + slider_h / 2.0,
        slider_knob_r,
        0.0,
        std::f64::consts::PI * 2.0,
    );
    let _ = ctx.fill();
    hits.push(HitRegion {
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
