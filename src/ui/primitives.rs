use std::f64::consts::{FRAC_PI_2, PI};

use crate::ui_text::{UiTextStyle, text_layout};

pub(crate) fn text_extents_for(
    ctx: &cairo::Context,
    family: &str,
    slant: cairo::FontSlant,
    weight: cairo::FontWeight,
    size: f64,
    text: &str,
) -> cairo::TextExtents {
    let layout = text_layout(
        ctx,
        UiTextStyle {
            family,
            slant,
            weight,
            size,
        },
        text,
        None,
    );
    layout.ink_extents().to_cairo()
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
