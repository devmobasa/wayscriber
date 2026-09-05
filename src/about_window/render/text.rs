use crate::ui_text::{UiTextEngine, UiTextStyle};

pub(super) fn draw_text(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    style: UiTextStyle<'_>,
    x: f64,
    y: f64,
    text: &str,
) -> (f64, f64, f64, f64) {
    let extents = engine.draw_baseline(ctx, style, text, x, y, None);
    (
        x + extents.x_bearing(),
        y + extents.y_bearing(),
        extents.width(),
        extents.height(),
    )
}
