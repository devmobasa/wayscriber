//! The bend handle drawn on a selected curved arrow.

use crate::util::Rect;

/// Border width of the handle, matching the selection box's handles so the two
/// read as one family of grips.
const HANDLE_BORDER: f64 = 1.5;

/// Draws the bend grip centred in `rect`.
///
/// Round rather than square, which is what separates it at a glance from the
/// eight square resize handles it sits among: those scale the whole selection,
/// this one reshapes a single arc.
pub(crate) fn render_arrow_bend_handle(ctx: &cairo::Context, rect: Rect) {
    let radius = f64::from(rect.width.min(rect.height)) / 2.0;
    if radius <= 0.0 {
        return;
    }
    let cx = f64::from(rect.x) + f64::from(rect.width) / 2.0;
    let cy = f64::from(rect.y) + f64::from(rect.height) / 2.0;

    let _ = ctx.save();
    ctx.new_path();
    ctx.arc(cx, cy, radius, 0.0, std::f64::consts::TAU);
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    let _ = ctx.fill_preserve();
    ctx.set_source_rgba(0.2, 0.45, 1.0, 0.95);
    ctx.set_line_width(HANDLE_BORDER);
    let _ = ctx.stroke();
    let _ = ctx.restore();
}
