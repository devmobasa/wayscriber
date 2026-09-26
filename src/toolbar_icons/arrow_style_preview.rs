//! Arrow style previews for the style pill's arrow chip and the menu it
//! opens, painted the same way by both toolbar frontends.
//!
//! Each preview is the real arrow outline ([`crate::util::calculate_arrow_outline_styled`],
//! the one the canvas fills) for a short left-to-right arrow, so the menu
//! shows exactly what each style draws rather than a hand-drawn stand-in.

use crate::draw::ArrowStyle;
use crate::ui::theme::{Rgba, set_color};

/// Sample space, authored large so the outline's whole-unit endpoints stay
/// far below a preview pixel once scaled down.
const SAMPLE_W: f64 = 240.0;
const SAMPLE_H: f64 = 96.0;
const SAMPLE_INSET: i32 = 14;
const SAMPLE_THICKNESS: f64 = 14.0;
const SAMPLE_HEAD_LENGTH: f64 = 66.0;
const SAMPLE_HEAD_ANGLE: f64 = 30.0;

/// Paint `style` as a left-to-right arrow filling `rect` (logical units),
/// in `color`.
pub(crate) fn draw_arrow_style_preview(
    ctx: &cairo::Context,
    rect: (f64, f64, f64, f64),
    style: ArrowStyle,
    color: Rgba,
) {
    let (x, y, w, h) = rect;
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let Some(outline) = sample_outline(style) else {
        return;
    };

    let scale = (w / SAMPLE_W).min(h / SAMPLE_H);
    let origin = (
        x + (w - SAMPLE_W * scale) / 2.0,
        y + (h - SAMPLE_H * scale) / 2.0,
    );
    let _ = ctx.save();
    ctx.new_path();
    for &(px, py) in &outline {
        ctx.line_to(origin.0 + px * scale, origin.1 + py * scale);
    }
    ctx.close_path();
    set_color(ctx, color);
    let _ = ctx.fill();
    let _ = ctx.restore();
}

/// The style's outline in sample units. A curved arrow's arc bulges up, so
/// its ends sit low enough for the bulge to stay inside the sample.
fn sample_outline(style: ArrowStyle) -> Option<Vec<(f64, f64)>> {
    let baseline = if style.is_curved() {
        (SAMPLE_H * 0.68) as i32
    } else {
        (SAMPLE_H / 2.0) as i32
    };
    crate::util::calculate_arrow_outline_styled(
        SAMPLE_W as i32 - SAMPLE_INSET,
        baseline,
        SAMPLE_INSET,
        baseline,
        SAMPLE_THICKNESS,
        SAMPLE_HEAD_LENGTH,
        SAMPLE_HEAD_ANGLE,
        style,
        style.effective_bend(crate::util::DEFAULT_ARROW_BEND),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_style_fits_its_sample() {
        for style in ArrowStyle::ALL {
            let outline = sample_outline(style).expect("outline");
            for (x, y) in outline {
                assert!(
                    (0.0..=SAMPLE_W).contains(&x) && (0.0..=SAMPLE_H).contains(&y),
                    "{style:?} point ({x}, {y}) leaves the sample"
                );
            }
        }
    }

    #[test]
    fn the_styles_draw_four_different_shapes() {
        let outlines: Vec<_> = ArrowStyle::ALL.into_iter().map(sample_outline).collect();

        for (index, outline) in outlines.iter().enumerate() {
            assert!(
                outlines[index + 1..].iter().all(|other| other != outline),
                "{:?} matches another style",
                ArrowStyle::ALL[index]
            );
        }
    }
}
