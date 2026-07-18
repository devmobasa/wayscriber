use crate::domain::BoardBackground;

/// Renders board background for solid board modes.
///
/// This function fills the entire canvas with a solid color when in
/// whiteboard or blackboard mode. For transparent mode, it does nothing
/// (background remains transparent).
///
/// Should be called after clearing the canvas but before rendering shapes.
///
/// # Arguments
/// * `ctx` - Cairo drawing context to render to
/// * `background` - Current board background
#[allow(dead_code)]
pub fn render_board_background(ctx: &cairo::Context, background: &BoardBackground) {
    if let BoardBackground::Solid(bg_color) = background {
        ctx.set_source_rgba(bg_color.r, bg_color.g, bg_color.b, bg_color.a);
        let _ = ctx.paint(); // Ignore errors - if paint fails, we'll just have transparent bg
    }
    // If None (Transparent mode), do nothing - background stays transparent
}

/// Fills the entire surface with a semi-transparent tinted background.
///
/// Creates a barely visible dark tint (0.05 alpha) to confirm the overlay is active
/// without obscuring the screen content. This function is kept for potential future use.
///
/// # Arguments
/// * `ctx` - Cairo drawing context to fill
/// * `width` - Surface width in pixels
/// * `height` - Surface height in pixels
#[allow(dead_code)]
pub fn fill_transparent(ctx: &cairo::Context, width: i32, height: i32) {
    // Use a very slight tint so we can see the overlay is there
    // 0.05 alpha = barely visible, just enough to confirm it's working
    ctx.set_source_rgba(0.1, 0.1, 0.1, 0.05);
    ctx.set_operator(cairo::Operator::Source);
    ctx.rectangle(0.0, 0.0, width as f64, height as f64);
    let _ = ctx.fill();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Color;

    fn rendered_bytes(background: BoardBackground) -> Vec<u8> {
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 2, 2).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        render_board_background(&ctx, &background);
        drop(ctx);
        surface.flush();
        surface.data().unwrap().to_vec()
    }

    #[test]
    fn transparent_background_does_not_paint_but_solid_background_does() {
        let transparent = rendered_bytes(BoardBackground::Transparent);
        let solid = rendered_bytes(BoardBackground::Solid(Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }));

        assert!(transparent.iter().all(|byte| *byte == 0));
        assert!(solid.iter().any(|byte| *byte != 0));
        assert_ne!(solid, transparent);
    }
}
