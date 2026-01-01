pub(super) fn draw_text(ctx: &cairo::Context, x: f64, y: f64, text: &str) -> (f64, f64, f64, f64) {
    ctx.move_to(x, y);
    let _ = ctx.show_text(text);
    let extents = match ctx.text_extents(text) {
        Ok(extents) => extents,
        Err(_) => fallback_text_extents(ctx, text),
    };
    (
        x + extents.x_bearing(),
        y + extents.y_bearing(),
        extents.width(),
        extents.height(),
    )
}

fn fallback_text_extents(ctx: &cairo::Context, text: &str) -> cairo::TextExtents {
    let height = ctx
        .font_extents()
        .map(|extents| extents.height())
        .unwrap_or(14.0);
    let width = text.len() as f64 * height * 0.5;
    cairo::TextExtents::new(0.0, -height, width, height, width, 0.0)
}
