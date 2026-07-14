mod arrows;
mod clock;
mod steps;

use cairo::Context;

/// Draw an undo icon (curved arrow left)
pub fn draw_icon_undo(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_undo(ctx, x, y, size);
}

/// Draw a redo icon (curved arrow right)
pub fn draw_icon_redo(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_redo(ctx, x, y, size);
}

/// Draw an undo all icon (double curved arrow left)
pub fn draw_icon_undo_all(ctx: &Context, x: f64, y: f64, size: f64) {
    arrows::draw_double_curved_arrow(ctx, x, y, size);
}

/// Draw a redo all icon (double curved arrow right)
pub fn draw_icon_redo_all(ctx: &Context, x: f64, y: f64, size: f64) {
    with_horizontal_mirror(ctx, x, y, size, |ctx| {
        arrows::draw_double_curved_arrow(ctx, 0.0, 0.0, size);
    });
}

/// Draw an undo all delay icon (double curved arrow left with clock)
pub fn draw_icon_undo_all_delay(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let arrow_size = s * 0.82;

    // Leave the lower-right quadrant for the clock badge.
    arrows::draw_double_curved_arrow(ctx, x, y, arrow_size);

    // Keep the badge and its stroke inside the GTK icon surface.
    ctx.set_line_width((s * 0.075).max(1.0));
    ctx.set_line_cap(cairo::LineCap::Round);
    clock::draw_clock(ctx, x + s * 0.78, y + s * 0.78, s * 0.12);
}

/// Draw a redo all delay icon (double curved arrow right with clock)
pub fn draw_icon_redo_all_delay(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let arrow_size = s * 0.82;

    // Mirror and right-align the reduced arrow, leaving room for the badge.
    with_horizontal_mirror(ctx, x, y, s, |ctx| {
        arrows::draw_double_curved_arrow(ctx, 0.0, 0.0, arrow_size);
    });

    // Keep the badge and its stroke inside the GTK icon surface.
    ctx.set_line_width((s * 0.075).max(1.0));
    ctx.set_line_cap(cairo::LineCap::Round);
    clock::draw_clock(ctx, x + s * 0.22, y + s * 0.78, s * 0.12);
}

/// Draw a step undo icon.
pub fn draw_icon_step_undo(ctx: &Context, x: f64, y: f64, size: f64) {
    steps::draw_step_undo(ctx, x, y, size);
}

/// Draw a step redo icon.
pub fn draw_icon_step_redo(ctx: &Context, x: f64, y: f64, size: f64) {
    steps::draw_step_redo(ctx, x, y, size);
}

fn with_horizontal_mirror(ctx: &Context, x: f64, y: f64, size: f64, draw: impl FnOnce(&Context)) {
    ctx.save().ok();
    ctx.translate(x + size, y);
    ctx.scale(-1.0, 1.0);
    draw(ctx);
    ctx.restore().ok();
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairo::{Format, ImageSurface};

    type IconPainter = fn(&Context, f64, f64, f64);

    const HISTORY_ICONS: [(&str, IconPainter); 6] = [
        ("undo all", draw_icon_undo_all),
        ("redo all", draw_icon_redo_all),
        ("undo all delayed", draw_icon_undo_all_delay),
        ("redo all delayed", draw_icon_redo_all_delay),
        ("step undo", draw_icon_step_undo),
        ("step redo", draw_icon_step_redo),
    ];

    fn render_icon(draw: IconPainter, size: i32) -> ImageSurface {
        let surface = ImageSurface::create(Format::ARgb32, size, size).expect("surface");
        let ctx = Context::new(&surface).expect("context");
        ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
        draw(&ctx, 0.0, 0.0, f64::from(size));
        drop(ctx);
        surface.flush();
        surface
    }

    #[test]
    fn history_icons_leave_the_gtk_surface_edge_clear() {
        for (name, draw) in HISTORY_ICONS {
            for size in [18, 20, 24] {
                let surface = render_icon(draw, size);
                let stride = surface.stride() as usize;
                surface
                    .with_data(|pixels| {
                        let alpha_at = |x: usize, y: usize| pixels[y * stride + x * 4 + 3];
                        let last = size as usize - 1;
                        assert!(
                            pixels.chunks_exact(4).any(|pixel| pixel[3] != 0),
                            "{name} rendered empty at {size}px"
                        );
                        let touches_edge = (0..=last).any(|position| {
                            alpha_at(position, 0) != 0
                                || alpha_at(position, last) != 0
                                || alpha_at(0, position) != 0
                                || alpha_at(last, position) != 0
                        });
                        assert!(!touches_edge, "{name} touches the {size}px GTK icon edge");
                    })
                    .expect("surface data");
            }
        }
    }
}
