//! How bright is what is already painted under a rectangle?
//!
//! Text on the overlay needs a halo that contrasts with its background. The
//! only thing that knows the background is the surface itself, after the
//! backdrop and every earlier shape have been painted onto it and before the
//! text goes on top. So the probe reads the render target rather than the
//! captured desktop image: a label over a blur, over a filled rectangle, or
//! over a board colour all answer correctly, and none of them would if the
//! probe looked at the raw screen capture instead.
//!
//! On a transparent board with no frozen or zoomed capture there is nothing
//! painted under the label — the desktop shows through the compositor and its
//! pixels were never ours. The probe reports `None` there, which is honest, and
//! the caller keeps its previous behaviour.

use crate::draw::Color;

/// Width and height of the scratch surface the region is downsampled into.
///
/// The probe wants one average, not detail, so the region is scaled into a
/// fixed tiny surface. Cairo does the averaging during the paint, and the cost
/// stops depending on how large the text is.
const PROBE_WIDTH: i32 = 8;
const PROBE_HEIGHT: i32 = 4;

/// Alpha at or above which a sampled pixel counts as background rather than as
/// a hole the desktop shows through.
const OPAQUE_ALPHA: u8 = 200;

/// Fraction of samples that must be opaque before the average means anything.
/// Under this the label straddles an edge or floats over live desktop, and a
/// guess would be worse than the caller's fallback.
const MIN_OPAQUE_FRACTION: f64 = 0.5;

/// Relative luminance of what is painted under `bounds`, in user-space
/// coordinates, or `None` when too little of it is opaque to judge.
pub fn painted_luminance(ctx: &cairo::Context, bounds: (f64, f64, f64, f64)) -> Option<f64> {
    let (x, y, width, height) = bounds;
    if !(x.is_finite() && y.is_finite() && width.is_finite() && height.is_finite())
        || width <= 0.0
        || height <= 0.0
    {
        return None;
    }

    let target = ctx.target();
    let (target_width, target_height) = target_size(&target)?;

    // The region is in user space and the target is in device pixels; the
    // canvas transform between them can be a zoom, a pan, or both.
    let (device_x, device_y) = ctx.user_to_device(x, y);
    let (far_x, far_y) = ctx.user_to_device(x + width, y + height);
    let left = device_x.min(far_x).floor().max(0.0);
    let top = device_y.min(far_y).floor().max(0.0);
    let right = device_x.max(far_x).ceil().min(f64::from(target_width));
    let bottom = device_y.max(far_y).ceil().min(f64::from(target_height));
    if right - left < 1.0 || bottom - top < 1.0 {
        return None;
    }

    let probe =
        cairo::ImageSurface::create(cairo::Format::ARgb32, PROBE_WIDTH, PROBE_HEIGHT).ok()?;
    {
        let copy = cairo::Context::new(&probe).ok()?;
        copy.set_operator(cairo::Operator::Source);
        copy.scale(
            f64::from(PROBE_WIDTH) / (right - left),
            f64::from(PROBE_HEIGHT) / (bottom - top),
        );
        target.flush();
        copy.set_source_surface(&target, -left, -top).ok()?;
        copy.paint().ok()?;
    }
    let mut probe = probe;
    probe.flush();

    average_luminance(&mut probe)
}

fn target_size(target: &cairo::Surface) -> Option<(i32, i32)> {
    let image = cairo::ImageSurface::try_from(target.clone()).ok()?;
    let (width, height) = (image.width(), image.height());
    (width > 0 && height > 0).then_some((width, height))
}

/// Mean relative luminance of the opaque samples, or `None`.
fn average_luminance(probe: &mut cairo::ImageSurface) -> Option<f64> {
    let stride = usize::try_from(probe.stride()).ok()?;
    let width = usize::try_from(probe.width()).ok()?;
    let height = usize::try_from(probe.height()).ok()?;
    let data = probe.data().ok()?;

    let mut total = 0.0;
    let mut opaque = 0usize;
    for row in 0..height {
        for column in 0..width {
            let offset = row * stride + column * 4;
            let Some(pixel) = data.get(offset..offset + 4) else {
                continue;
            };
            // Cairo ARGB32 is native-endian and premultiplied.
            let alpha = pixel[3];
            if alpha < OPAQUE_ALPHA {
                continue;
            }
            let scale = f64::from(alpha) / 255.0;
            let blue = f64::from(pixel[0]) / 255.0 / scale;
            let green = f64::from(pixel[1]) / 255.0 / scale;
            let red = f64::from(pixel[2]) / 255.0 / scale;
            total += relative_luminance(red, green, blue);
            opaque += 1;
        }
    }

    let samples = width * height;
    if samples == 0 || (opaque as f64) < samples as f64 * MIN_OPAQUE_FRACTION {
        return None;
    }
    Some(total / opaque as f64)
}

/// Weighted luminance, the same formula the board pen-contrast helper uses.
pub(super) fn relative_luminance(red: f64, green: f64, blue: f64) -> f64 {
    red * 0.299 + green * 0.587 + blue * 0.114
}

/// Relative luminance of a colour, ignoring its alpha.
pub(super) fn color_luminance(color: Color) -> f64 {
    relative_luminance(color.r, color.g, color.b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled_target(color: (f64, f64, f64, f64)) -> (cairo::ImageSurface, cairo::Context) {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 60).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        ctx.set_source_rgba(color.0, color.1, color.2, color.3);
        let _ = ctx.paint();
        (surface, ctx)
    }

    #[test]
    fn a_white_background_reads_as_bright_and_a_black_one_as_dark() {
        let (_white, ctx) = filled_target((1.0, 1.0, 1.0, 1.0));
        let bright = painted_luminance(&ctx, (10.0, 10.0, 40.0, 20.0)).expect("opaque");
        assert!(bright > 0.9, "got {bright}");

        let (_black, ctx) = filled_target((0.0, 0.0, 0.0, 1.0));
        let dark = painted_luminance(&ctx, (10.0, 10.0, 40.0, 20.0)).expect("opaque");
        assert!(dark < 0.1, "got {dark}");
    }

    #[test]
    fn a_transparent_target_reports_that_it_does_not_know() {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 60).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();

        assert!(
            painted_luminance(&ctx, (10.0, 10.0, 40.0, 20.0)).is_none(),
            "live desktop shows through here; the pixels were never ours to read"
        );
    }

    #[test]
    fn the_probe_reads_what_was_painted_rather_than_the_whole_surface() {
        let (_surface, ctx) = filled_target((0.0, 0.0, 0.0, 1.0));
        ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
        ctx.rectangle(0.0, 0.0, 50.0, 60.0);
        let _ = ctx.fill();

        let over_white = painted_luminance(&ctx, (5.0, 10.0, 40.0, 20.0)).expect("opaque");
        let over_black = painted_luminance(&ctx, (55.0, 10.0, 40.0, 20.0)).expect("opaque");

        assert!(over_white > 0.9, "got {over_white}");
        assert!(over_black < 0.1, "got {over_black}");
    }

    #[test]
    fn the_probe_follows_the_canvas_transform() {
        let (_surface, ctx) = filled_target((0.0, 0.0, 0.0, 1.0));
        ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
        ctx.rectangle(50.0, 0.0, 50.0, 60.0);
        let _ = ctx.fill();

        // Under a 2x zoom, user-space x=25 is device x=50: the white half.
        ctx.scale(2.0, 2.0);
        let sampled = painted_luminance(&ctx, (26.0, 5.0, 20.0, 10.0)).expect("opaque");

        assert!(
            sampled > 0.9,
            "the probe must map user space through the transform, got {sampled}"
        );
    }

    #[test]
    fn a_degenerate_or_offscreen_rectangle_asks_for_nothing() {
        let (_surface, ctx) = filled_target((1.0, 1.0, 1.0, 1.0));

        assert!(painted_luminance(&ctx, (10.0, 10.0, 0.0, 20.0)).is_none());
        assert!(painted_luminance(&ctx, (10.0, 10.0, f64::NAN, 20.0)).is_none());
        assert!(painted_luminance(&ctx, (500.0, 500.0, 40.0, 20.0)).is_none());
    }
}
