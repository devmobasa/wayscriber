use cairo::Context;

const VIEWBOX_SIZE: f64 = 24.0;

/// Draw an "undo all" glyph in the same open, rounded style as the primary
/// undo icon. The second chevron communicates "all" without turning the glyph
/// into a circular refresh symbol. Callers mirror this painter for redo.
pub(super) fn draw_double_curved_arrow(ctx: &Context, x: f64, y: f64, size: f64) {
    if !size.is_finite() || size <= 0.0 {
        return;
    }

    let _ = ctx.save();
    ctx.translate(x, y);
    ctx.scale(size / VIEWBOX_SIZE, size / VIEWBOX_SIZE);
    ctx.set_line_width(2.0);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Two left-pointing chevrons. The inner one owns the return path while the
    // outer one remains separate, so both heads stay legible at 18–20 px.
    draw_chevron(ctx, 4.5);
    draw_chevron(ctx, 9.5);

    ctx.new_path();
    ctx.move_to(9.5, 11.0);
    ctx.line_to(14.0, 11.0);
    ctx.curve_to(17.31, 11.0, 20.0, 13.69, 20.0, 17.0);
    let _ = ctx.stroke();

    let _ = ctx.restore();
}

fn draw_chevron(ctx: &Context, tip_x: f64) {
    ctx.new_path();
    ctx.move_to(tip_x + 4.0, 7.0);
    ctx.line_to(tip_x, 11.0);
    ctx.line_to(tip_x + 4.0, 15.0);
    let _ = ctx.stroke();
}
