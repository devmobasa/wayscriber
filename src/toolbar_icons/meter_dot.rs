//! Shared zero-level meter dot, used by Cairo and GTK toolbars.

use crate::ui::theme::set_color;
use crate::ui::theme::toolbar::{COLOR_ACCENT, COLOR_METER_TRACK_HOVER};

pub(crate) fn draw_meter_dot(
    ctx: &cairo::Context,
    rect: (f64, f64, f64, f64),
    active: bool,
    hover: bool,
    enabled: bool,
) {
    let (x, y, _, h) = rect;
    let alpha = if enabled { 1.0 } else { 0.35 };
    let faded = |(r, g, b, a)| (r, g, b, a * alpha);
    let _ = ctx.save();
    ctx.new_path();
    set_color(ctx, faded(COLOR_ACCENT));
    ctx.arc(x + 6.0, y + h / 2.0, 5.0, 0.0, std::f64::consts::TAU);
    let _ = ctx.fill();

    if hover && !active && enabled {
        set_color(ctx, COLOR_METER_TRACK_HOVER);
        ctx.set_line_width(1.5);
        ctx.arc(x + 6.0, y + h / 2.0, 7.5, 0.0, std::f64::consts::TAU);
        let _ = ctx.stroke();
    }

    let _ = ctx.restore();
}
