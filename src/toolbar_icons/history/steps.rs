use cairo::Context;

/// Step-undo button glyph. The adjacent count states the multi-step behavior,
/// so this uses the standard undo symbol without a tiny numeric decoration.
pub(super) fn draw_step_undo(ctx: &Context, x: f64, y: f64, size: f64) {
    super::super::svg::render_undo(ctx, x, y, size);
}

/// Step-redo button glyph: the mirror of the step-undo arrow.
pub(super) fn draw_step_redo(ctx: &Context, x: f64, y: f64, size: f64) {
    super::super::svg::render_redo(ctx, x, y, size);
}
