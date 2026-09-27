//! Reference ticks on a slider track, painted the same way by both toolbar
//! frontends.
//!
//! A thickness value alone does not say how the curved track is laid out, so
//! a few faint ticks at familiar widths mark where they fall.

use crate::ui::theme::set_color;
use crate::ui::theme::toolbar::COLOR_TRACK_TICK;

/// Paint a tick at each track position in `positions` (each in `[0, 1]`)
/// across the track inside `rect`, on the knob's inset travel so a tick sits
/// exactly where the knob would for that value.
pub(crate) fn draw_slider_ticks(
    ctx: &cairo::Context,
    rect: (f64, f64, f64, f64),
    positions: impl IntoIterator<Item = f64>,
) {
    let (x, y, w, h) = rect;
    let knob_r = (h / 2.0).min(7.0);
    let travel = (w - knob_r * 2.0).max(0.0);
    let track_h = (h * 0.5).min(8.0);
    let top = y + (h - track_h) / 2.0 + 1.0;
    let bottom = top + track_h - 2.0;

    for t in positions {
        let tick_x = (x + knob_r + t.clamp(0.0, 1.0) * travel).round() + 0.5;
        ctx.move_to(tick_x, top);
        ctx.line_to(tick_x, bottom);
    }
    set_color(ctx, COLOR_TRACK_TICK);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();
}
