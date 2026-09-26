//! Level meter bars: a slim rounded bar centered in each bar's hit slot.

use super::draw_round_rect;
use crate::ui::theme::set_color;
use crate::ui::theme::toolbar::{COLOR_ACCENT, COLOR_METER_TRACK_HOVER, COLOR_TRACK_BACKGROUND};

/// Height of a drawn bar; matches the slider track so meters and sliders read
/// as one family.
const BAR_H: f64 = 8.0;
/// Gap between neighbouring bars, split across both sides of each slot.
const BAR_GAP: f64 = 3.0;
/// Disabled meters keep their shape but fade out.
const DISABLED_ALPHA: f64 = 0.35;

pub(in crate::backend::wayland::toolbar::render) fn draw_meter_bar(
    ctx: &cairo::Context,
    rect: (f64, f64, f64, f64),
    filled: bool,
    hover: bool,
    enabled: bool,
) {
    let (x, y, w, h) = rect;
    let bar_h = BAR_H.min(h);
    let bar_x = x + BAR_GAP / 2.0;
    let bar_w = (w - BAR_GAP).max(1.0);
    let bar_y = y + (h - bar_h) / 2.0;

    let (r, g, b, a) = if filled {
        COLOR_ACCENT
    } else if hover {
        COLOR_METER_TRACK_HOVER
    } else {
        COLOR_TRACK_BACKGROUND
    };
    let alpha = if enabled { a } else { a * DISABLED_ALPHA };

    set_color(ctx, (r, g, b, alpha));
    draw_round_rect(ctx, bar_x, bar_y, bar_w, bar_h, bar_h / 2.0);
    let _ = ctx.fill();
}
