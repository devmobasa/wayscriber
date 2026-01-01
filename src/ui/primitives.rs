use std::f64::consts::{FRAC_PI_2, PI};

pub(crate) fn fallback_text_extents(font_size: f64, text: &str) -> cairo::TextExtents {
    let width = text.len() as f64 * font_size * 0.5;
    cairo::TextExtents::new(0.0, -font_size, width, font_size, width, 0.0)
}

pub(crate) fn text_extents_for(
    ctx: &cairo::Context,
    family: &str,
    slant: cairo::FontSlant,
    weight: cairo::FontWeight,
    size: f64,
    text: &str,
) -> cairo::TextExtents {
    ctx.select_font_face(family, slant, weight);
    ctx.set_font_size(size);
    match ctx.text_extents(text) {
        Ok(extents) => extents,
        Err(err) => {
            log::warn!(
                "Failed to measure text '{}': {}, using fallback metrics",
                text,
                err
            );
            fallback_text_extents(size, text)
        }
    }
}

pub(crate) fn draw_rounded_rect(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
) {
    let r = radius.min(width / 2.0).min(height / 2.0);
    ctx.new_sub_path();
    ctx.arc(x + width - r, y + r, r, -FRAC_PI_2, 0.0);
    ctx.arc(x + width - r, y + height - r, r, 0.0, FRAC_PI_2);
    ctx.arc(x + r, y + height - r, r, FRAC_PI_2, PI);
    ctx.arc(x + r, y + r, r, PI, 3.0 * FRAC_PI_2);
    ctx.close_path();
}
