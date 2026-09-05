//! Pure layout for the live rectangle/ellipse size readout.

use crate::ui_text::{UiTextEngine, UiTextStyle};

const FONT_SIZE: f64 = 12.0;
const PADDING_X: f64 = 8.0;
const POINTER_GAP: f64 = 15.0;
const SCREEN_MARGIN: f64 = 6.0;
const BADGE_HEIGHT: f64 = 22.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ShapeMeasureBadge {
    pub text: String,
    pub bounds: (f64, f64, f64, f64),
    pub baseline: (f64, f64),
}

pub(crate) fn shape_measure_badge_text_style() -> UiTextStyle<'static> {
    UiTextStyle {
        family: "monospace",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: FONT_SIZE,
    }
}

/// Lay out a fixed-size badge near the pointer, flipping it away from screen
/// edges. The displayed dimensions are logical canvas pixels, independent of
/// output scale and the current zoom transform.
pub(crate) fn measure_shape_badge(
    engine: &UiTextEngine,
    enabled: bool,
    size: (u32, u32),
    pointer: (f64, f64),
    screen_width: u32,
    screen_height: u32,
) -> Option<ShapeMeasureBadge> {
    if !enabled {
        return None;
    }
    let text = format!("{} × {}", size.0, size.1);
    let extents = engine.measure(shape_measure_badge_text_style(), &text, None)?;
    let width = (extents.width() + PADDING_X * 2.0)
        .min((screen_width as f64 - SCREEN_MARGIN * 2.0).max(0.0));
    let height = BADGE_HEIGHT.min((screen_height as f64 - SCREEN_MARGIN * 2.0).max(0.0));
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    let x = trailing_axis(pointer.0, width, screen_width as f64);
    let y = trailing_axis(pointer.1, height, screen_height as f64);
    let baseline = (
        x + (width - extents.width()) / 2.0 - extents.x_bearing(),
        y + (height - extents.height()) / 2.0 - extents.y_bearing(),
    );

    Some(ShapeMeasureBadge {
        text,
        bounds: (x, y, width, height),
        baseline,
    })
}

fn trailing_axis(pointer: f64, extent: f64, screen_extent: f64) -> f64 {
    let available_max = (screen_extent - extent - SCREEN_MARGIN).max(0.0);
    let preferred = if pointer + POINTER_GAP + extent + SCREEN_MARGIN <= screen_extent {
        pointer + POINTER_GAP
    } else {
        pointer - POINTER_GAP - extent
    };
    preferred.clamp(SCREEN_MARGIN.min(available_max), available_max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_text_uses_logical_width_and_height() {
        let badge = measure_shape_badge(
            &UiTextEngine::default(),
            true,
            (120, 80),
            (100.0, 100.0),
            1920,
            1080,
        )
        .expect("text measurement");
        let zero = measure_shape_badge(
            &UiTextEngine::default(),
            true,
            (0, 0),
            (100.0, 100.0),
            1920,
            1080,
        )
        .expect("text measurement");

        assert_eq!(badge.text, "120 × 80");
        assert_eq!(zero.text, "0 × 0");
    }

    #[test]
    fn disabled_badge_has_no_visual() {
        assert!(
            measure_shape_badge(
                &UiTextEngine::default(),
                false,
                (120, 80),
                (100.0, 100.0),
                1920,
                1080
            )
            .is_none()
        );
    }

    #[test]
    fn badge_handles_every_horizontal_and_vertical_flip_combination() {
        let layout = |pointer| {
            measure_shape_badge(&UiTextEngine::default(), true, (120, 80), pointer, 400, 300)
                .expect("text measurement")
                .bounds
        };
        let below_right = layout((20.0, 30.0));
        assert!(below_right.0 > 20.0 && below_right.1 > 30.0);

        let below_left = layout((390.0, 30.0));
        assert!(below_left.0 < 390.0 && below_left.1 > 30.0);

        let above_right = layout((20.0, 290.0));
        assert!(above_right.0 > 20.0 && above_right.1 < 290.0);

        let above_left = layout((390.0, 290.0));
        assert!(above_left.0 < 390.0 && above_left.1 < 290.0);
    }

    #[test]
    fn badge_clamps_to_a_tiny_surface() {
        let (x, y, width, height) = measure_shape_badge(
            &UiTextEngine::default(),
            true,
            (3840, 2160),
            (2.0, 2.0),
            80,
            20,
        )
        .expect("text measurement")
        .bounds;

        assert_eq!((x, y), (SCREEN_MARGIN, SCREEN_MARGIN));
        assert_eq!(width, 68.0);
        assert_eq!(height, 8.0);
    }

    #[test]
    fn retained_measurement_owner_keeps_geometry_after_scaled_label_paint() {
        let engine = UiTextEngine::default();
        for size in [(120, 80), (3840, 2160), (120, 80)] {
            let badge = measure_shape_badge(&engine, true, size, (100.0, 100.0), 800, 600).unwrap();
            for density in [1, 2, 1] {
                let paint = |owner: &UiTextEngine| {
                    let mut surface = cairo::ImageSurface::create(
                        cairo::Format::ARgb32,
                        800 * density,
                        600 * density,
                    )
                    .unwrap();
                    {
                        let ctx = cairo::Context::new(&surface).unwrap();
                        ctx.scale(f64::from(density), f64::from(density));
                        owner
                            .layout(&ctx, shape_measure_badge_text_style(), &badge.text, None)
                            .show_at_baseline(&ctx, badge.baseline.0, badge.baseline.1);
                    }
                    surface.flush();
                    surface.data().unwrap().to_vec()
                };
                let retained = paint(&engine);
                assert!(
                    retained == paint(&UiTextEngine::default()),
                    "size {size:?} differs at density {density}"
                );
                assert!(retained.iter().any(|byte| *byte != 0));
                let repeated =
                    measure_shape_badge(&engine, true, size, (100.0, 100.0), 800, 600).unwrap();
                assert_eq!(repeated.bounds, badge.bounds);
                assert_eq!(repeated.baseline, badge.baseline);
                assert_eq!(repeated.text, badge.text);
            }
        }
    }
}
