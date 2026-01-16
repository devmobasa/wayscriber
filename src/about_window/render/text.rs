use crate::ui_text::{UiTextStyle, text_layout};

pub(super) fn draw_text(
    ctx: &cairo::Context,
    style: UiTextStyle<'_>,
    x: f64,
    y: f64,
    text: &str,
) -> (f64, f64, f64, f64) {
    let layout = text_layout(ctx, style, text, None);
    let extents = layout.ink_extents();
    layout.show_at_baseline(ctx, x, y);
    (
        x + extents.x_bearing(),
        y + extents.y_bearing(),
        extents.width(),
        extents.height(),
    )
}
