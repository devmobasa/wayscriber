//! The hover preview for the marker's snap-to-text mode.
//!
//! Two marks, both in canvas coordinates so the caller can draw them inside the
//! same transform as the annotations:
//!
//! - a faint band along the whole detected row, showing how far the highlight
//!   can reach before the pointer is committed to anything;
//! - an I-beam at the pointer, sized to the row, showing exactly which row a
//!   press would take and where the stroke would start.
//!
//! The band answers "which line?" and the I-beam answers "which line, and from
//! where?". Drawing only the I-beam leaves the reach a guess; drawing only the
//! band leaves the start point a guess.

use crate::draw::Color;

/// The preview's geometry, already mapped into canvas coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkerSnapPreview {
    /// Left and right ends of the row the highlight is clamped to.
    pub left: f64,
    pub right: f64,
    /// Vertical center of the row: where the stroke's spine would sit.
    pub center_y: f64,
    /// Stroke thickness a committed highlight would use.
    pub thickness: f64,
    /// Pointer x, where a press would start the stroke.
    pub pointer_x: f64,
}

/// Alpha of the reach band, as a fraction of the marker's own alpha. Low
/// enough that it never competes with committed highlights already on screen.
const BAND_ALPHA_FACTOR: f64 = 0.22;
/// Alpha of the I-beam, as a fraction of the marker's own alpha.
const BEAM_ALPHA_FACTOR: f64 = 0.95;
/// I-beam stem width in canvas pixels, before scale.
const BEAM_WIDTH: f64 = 2.0;
/// How far the I-beam's serifs reach either side of its stem.
const BEAM_SERIF_REACH: f64 = 3.5;
/// How far past the row's thickness the I-beam extends, so it reads as a caret
/// standing on the row rather than as part of the highlight.
const BEAM_OVERSHOOT: f64 = 2.0;

/// Draw the band and the I-beam for `preview` in the marker's `color`.
///
/// `color`'s alpha is the marker's own ink alpha; both marks derive from it so
/// the preview tracks opacity changes instead of drifting from the tool.
pub(crate) fn render_marker_snap_preview(
    ctx: &cairo::Context,
    preview: MarkerSnapPreview,
    color: Color,
) {
    if !preview_is_drawable(preview) {
        return;
    }

    let _ = ctx.save();
    ctx.set_line_cap(cairo::LineCap::Butt);

    let half = preview.thickness / 2.0;
    ctx.rectangle(
        preview.left,
        preview.center_y - half,
        preview.right - preview.left,
        preview.thickness,
    );
    ctx.set_source_rgba(color.r, color.g, color.b, color.a * BAND_ALPHA_FACTOR);
    let _ = ctx.fill();

    let beam_top = preview.center_y - half - BEAM_OVERSHOOT;
    let beam_bottom = preview.center_y + half + BEAM_OVERSHOOT;
    let x = preview.pointer_x.clamp(preview.left, preview.right);
    ctx.set_source_rgba(color.r, color.g, color.b, color.a * BEAM_ALPHA_FACTOR);

    ctx.set_line_width(BEAM_WIDTH);
    ctx.move_to(x, beam_top);
    ctx.line_to(x, beam_bottom);
    let _ = ctx.stroke();

    // Serifs, so the caret reads as a text I-beam rather than as a stray rule.
    ctx.set_line_width(BEAM_WIDTH * 0.75);
    for y in [beam_top, beam_bottom] {
        ctx.move_to(x - BEAM_SERIF_REACH, y);
        ctx.line_to(x + BEAM_SERIF_REACH, y);
    }
    let _ = ctx.stroke();

    let _ = ctx.restore();
}

fn preview_is_drawable(preview: MarkerSnapPreview) -> bool {
    preview.left.is_finite()
        && preview.right.is_finite()
        && preview.center_y.is_finite()
        && preview.thickness.is_finite()
        && preview.pointer_x.is_finite()
        && preview.right > preview.left
        && preview.thickness > 0.0
}

/// The damage rectangle the preview occupies, in canvas coordinates.
///
/// Padded past the I-beam's serifs and overshoot; the caller marks this dirty
/// so a preview that moves to another row leaves nothing behind.
pub(crate) fn marker_snap_preview_bounds(preview: MarkerSnapPreview) -> Option<crate::util::Rect> {
    if !preview_is_drawable(preview) {
        return None;
    }
    let half = preview.thickness / 2.0;
    let pad = BEAM_SERIF_REACH.max(BEAM_OVERSHOOT) + BEAM_WIDTH;
    let left = clamped_i32((preview.left - pad).floor())?;
    let top = clamped_i32((preview.center_y - half - pad).floor())?;
    let right = clamped_i32((preview.right + pad).ceil())?;
    let bottom = clamped_i32((preview.center_y + half + pad).ceil())?;
    crate::util::Rect::from_min_max(left, top, right, bottom)
}

/// A finite canvas coordinate as `i32`, or `None` when it cannot be one.
fn clamped_i32(value: f64) -> Option<i32> {
    value
        .is_finite()
        .then(|| value.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preview() -> MarkerSnapPreview {
        MarkerSnapPreview {
            left: 100.0,
            right: 400.0,
            center_y: 110.0,
            thickness: 23.0,
            pointer_x: 250.0,
        }
    }

    fn surface() -> cairo::ImageSurface {
        cairo::ImageSurface::create(cairo::Format::ARgb32, 500, 200).unwrap()
    }

    fn alpha_at(surface: &mut cairo::ImageSurface, x: i32, y: i32) -> u8 {
        surface.flush();
        let stride = surface.stride() as usize;
        let offset = y as usize * stride + x as usize * 4;
        surface.data().unwrap()[offset + 3]
    }

    fn marker() -> Color {
        Color {
            r: 1.0,
            g: 1.0,
            b: 0.0,
            a: 0.4,
        }
    }

    #[test]
    fn the_band_covers_the_row_and_the_beam_stands_on_it() {
        let mut surface = surface();
        let ctx = cairo::Context::new(&surface).unwrap();

        render_marker_snap_preview(&ctx, preview(), marker());
        drop(ctx);

        let band = alpha_at(&mut surface, 150, 110);
        let beam = alpha_at(&mut surface, 250, 110);
        assert!(band > 0, "the row band is drawn");
        assert!(beam > band, "the I-beam reads stronger than the band");
        assert_eq!(
            alpha_at(&mut surface, 450, 110),
            0,
            "nothing is drawn past the row"
        );
        assert_eq!(
            alpha_at(&mut surface, 150, 180),
            0,
            "nothing is drawn below the row"
        );
    }

    #[test]
    fn the_beam_is_clamped_into_the_row_it_belongs_to() {
        let mut surface = surface();
        let ctx = cairo::Context::new(&surface).unwrap();

        render_marker_snap_preview(
            &ctx,
            MarkerSnapPreview {
                pointer_x: 480.0,
                ..preview()
            },
            marker(),
        );
        drop(ctx);

        assert!(
            alpha_at(&mut surface, 398, 110) > 0,
            "the beam is pulled back to the row's end"
        );
    }

    #[test]
    fn a_degenerate_preview_draws_nothing_and_has_no_bounds() {
        let mut surface = surface();
        let ctx = cairo::Context::new(&surface).unwrap();

        for broken in [
            MarkerSnapPreview {
                right: 100.0,
                ..preview()
            },
            MarkerSnapPreview {
                thickness: 0.0,
                ..preview()
            },
            MarkerSnapPreview {
                center_y: f64::NAN,
                ..preview()
            },
        ] {
            render_marker_snap_preview(&ctx, broken, marker());
            assert!(marker_snap_preview_bounds(broken).is_none());
        }
        drop(ctx);

        assert_eq!(alpha_at(&mut surface, 150, 110), 0);
    }

    #[test]
    fn the_damage_bounds_contain_every_mark_the_preview_draws() {
        let bounds = marker_snap_preview_bounds(preview()).expect("bounds");

        assert!(bounds.x < 100);
        assert!(bounds.x + bounds.width > 400);
        assert!(bounds.y < 110 - 12);
        assert!(bounds.y + bounds.height > 110 + 12);
    }
}
