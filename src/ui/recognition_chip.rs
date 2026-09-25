//! Pure layout and painting for the chip that names what Shape Pen recognized.

use crate::ui_text::{UiTextEngine, UiTextStyle};

const FONT_SIZE: f64 = 12.0;
const PADDING_X: f64 = 8.0;
const CHIP_HEIGHT: f64 = 22.0;
const CORNER_RADIUS: f64 = 6.0;
/// Space between the shape and the chip.
const SHAPE_GAP: f64 = 8.0;
const SCREEN_MARGIN: f64 = 6.0;

/// Where the chip sits this frame and how opaque it is.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RecognitionChipVisual {
    pub text: String,
    pub bounds: (f64, f64, f64, f64),
    pub baseline: (f64, f64),
    pub opacity: f64,
}

fn recognition_chip_text_style() -> UiTextStyle<'static> {
    UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: FONT_SIZE,
    }
}

/// Lays the chip out centred under the shape's screen bounds `anchor`, or
/// above it when the shape reaches the bottom of the screen, and keeps it
/// inside the screen. `None` once it has faded out or cannot fit.
pub(crate) fn recognition_chip_layout(
    engine: &UiTextEngine,
    label: &str,
    anchor: (f64, f64, f64, f64),
    opacity: f64,
    screen_width: u32,
    screen_height: u32,
) -> Option<RecognitionChipVisual> {
    if opacity <= 0.0 {
        return None;
    }

    let extents = engine.measure(recognition_chip_text_style(), label, None)?;
    let screen_width = f64::from(screen_width);
    let screen_height = f64::from(screen_height);
    let width = (extents.width() + PADDING_X * 2.0).min(screen_width - SCREEN_MARGIN * 2.0);
    let height = CHIP_HEIGHT.min(screen_height - SCREEN_MARGIN * 2.0);
    if width <= 0.0 || height <= 0.0 {
        return None;
    }

    let (anchor_x, anchor_y, anchor_width, anchor_height) = anchor;
    let x = (anchor_x + (anchor_width - width) / 2.0)
        .clamp(SCREEN_MARGIN, screen_width - SCREEN_MARGIN - width);
    let below = anchor_y + anchor_height + SHAPE_GAP;
    let above = anchor_y - SHAPE_GAP - height;
    let bottom_limit = screen_height - SCREEN_MARGIN - height;
    let y = if below <= bottom_limit || above < SCREEN_MARGIN {
        below
    } else {
        above
    }
    .clamp(SCREEN_MARGIN, bottom_limit);
    let baseline = (
        x + (width - extents.width()) / 2.0 - extents.x_bearing(),
        y + (height - extents.height()) / 2.0 - extents.y_bearing(),
    );

    Some(RecognitionChipVisual {
        text: label.to_string(),
        bounds: (x, y, width, height),
        baseline,
        opacity: opacity.min(1.0),
    })
}

/// Paints the chip in the same quiet dark pill as the live shape readout it
/// follows, faded by its opacity.
pub(crate) fn render_recognition_chip(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    visual: &RecognitionChipVisual,
) {
    let (x, y, width, height) = visual.bounds;
    let alpha = visual.opacity;
    let _ = ctx.save();

    super::draw_pill(
        ctx,
        x,
        y,
        width,
        height,
        CORNER_RADIUS,
        (12.0 / 255.0, 12.0 / 255.0, 15.0 / 255.0, 0.92 * alpha),
        (1.0, 1.0, 1.0, 0.16 * alpha),
        None,
    );

    ctx.set_source_rgba(1.0, 1.0, 1.0, alpha);
    ctx.rectangle(x, y, width, height);
    ctx.clip();
    engine
        .layout(ctx, recognition_chip_text_style(), &visual.text, None)
        .show_at_baseline(ctx, visual.baseline.0, visual.baseline.1);

    let _ = ctx.restore();
}

#[cfg(test)]
mod tests {
    use super::*;

    const LABEL: &str = "Circle · Ctrl+Z keeps ink";

    fn layout(anchor: (f64, f64, f64, f64), opacity: f64) -> Option<RecognitionChipVisual> {
        recognition_chip_layout(&UiTextEngine::default(), LABEL, anchor, opacity, 800, 600)
    }

    #[test]
    fn the_chip_sits_centred_under_the_shape() {
        let chip = layout((300.0, 100.0, 200.0, 150.0), 1.0).expect("chip layout");
        let (x, y, width, _) = chip.bounds;

        assert_eq!(chip.text, LABEL);
        assert!(
            (x + width / 2.0 - 400.0).abs() < 0.5,
            "centred on the shape"
        );
        assert_eq!(y, 100.0 + 150.0 + SHAPE_GAP);
    }

    #[test]
    fn a_shape_at_the_bottom_puts_the_chip_above_it() {
        let chip = layout((300.0, 450.0, 200.0, 140.0), 1.0).expect("chip layout");
        let (_, y, _, height) = chip.bounds;

        assert_eq!(y + height, 450.0 - SHAPE_GAP);
    }

    #[test]
    fn a_shape_at_the_edge_keeps_the_chip_on_screen() {
        let chip = layout((760.0, 200.0, 60.0, 60.0), 1.0).expect("chip layout");
        let (x, _, width, _) = chip.bounds;

        assert!(x + width <= 800.0 - SCREEN_MARGIN);
    }

    #[test]
    fn a_faded_chip_has_no_layout() {
        assert!(layout((300.0, 100.0, 200.0, 150.0), 0.0).is_none());
        assert_eq!(
            layout((300.0, 100.0, 200.0, 150.0), 0.4)
                .expect("fading chip")
                .opacity,
            0.4
        );
    }
}
