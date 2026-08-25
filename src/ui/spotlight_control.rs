//! The on-canvas magnification slider that rides above a selected Spotlight.
//!
//! Painting only: geometry and hit testing belong to
//! [`crate::input::state::SpotlightMagnificationTrack`], and the caller decides
//! when the control is visible.

use crate::input::state::SpotlightMagnificationTrack;
use crate::ui::theme::{self, overlay};
use crate::ui::{draw_rounded_rect, text_extents_for};
use crate::ui_text::{UiTextStyle, draw_text_baseline};

/// Height of the drawn track bar, which is thinner than the knob it carries.
const TRACK_BAR_HEIGHT: f64 = 4.0;
/// Padding inside the readout plate.
const READOUT_PADDING: f64 = 5.0;
/// Gap between the plate and the track below it.
const READOUT_GAP: f64 = 4.0;

/// Paints the track, the travelled portion, the knob, and the factor readout.
///
/// `unavailable_reason` is the surface's own explanation for why a loupe cannot
/// preview right now. When present the readout carries it and dims, so a
/// factor that visibly does nothing still says why.
///
/// `visible` is the visible canvas rectangle. The readout plate is sized by the
/// text it carries, so it can be wider than the 120px track and would otherwise
/// be clipped at a screen edge; it is clamped into `visible` independently of
/// the track it is centred on.
pub(crate) fn render_spotlight_magnification_control(
    ctx: &cairo::Context,
    track: SpotlightMagnificationTrack,
    magnification: f64,
    unavailable_reason: Option<&str>,
    visible: crate::util::Rect,
) {
    let bounds = track.track;
    let knob = track.knob;
    let radius = f64::from(bounds.height) / 2.0;
    let bar_y = f64::from(bounds.y) + radius - TRACK_BAR_HEIGHT / 2.0;

    let _ = ctx.save();

    draw_rounded_rect(
        ctx,
        f64::from(bounds.x),
        bar_y,
        f64::from(bounds.width),
        TRACK_BAR_HEIGHT,
        TRACK_BAR_HEIGHT / 2.0,
    );
    theme::set_color(ctx, overlay::PROGRESS_TRACK);
    let _ = ctx.fill();

    // The fill stops under the knob's centre, so the two always agree about
    // where the current value is.
    let filled = f64::from(knob.x - bounds.x) + f64::from(knob.width) / 2.0;
    if filled > 0.0 {
        draw_rounded_rect(
            ctx,
            f64::from(bounds.x),
            bar_y,
            filled,
            TRACK_BAR_HEIGHT,
            TRACK_BAR_HEIGHT / 2.0,
        );
        theme::set_color(ctx, overlay::PROGRESS_FILL);
        let _ = ctx.fill();
    }

    ctx.arc(
        f64::from(knob.x) + radius,
        f64::from(knob.y) + radius,
        radius,
        0.0,
        std::f64::consts::TAU,
    );
    theme::set_color(ctx, overlay::TEXT_WHITE);
    let _ = ctx.fill_preserve();
    theme::set_color(ctx, overlay::ACCENT_BRIGHT);
    ctx.set_line_width(1.5);
    let _ = ctx.stroke();

    let label = match unavailable_reason {
        Some(reason) => format!(
            "{} - {reason}",
            crate::draw::format_spotlight_magnification(magnification)
        ),
        None => crate::draw::format_spotlight_magnification(magnification),
    };
    let style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: 12.0,
    };
    let extents = text_extents_for(
        ctx,
        style.family,
        style.slant,
        style.weight,
        style.size,
        &label,
    );
    let plate_w = extents.width() + READOUT_PADDING * 2.0;
    let plate_h = extents.height() + READOUT_PADDING * 2.0;
    let centred_x = f64::from(bounds.x) + f64::from(bounds.width) / 2.0 - plate_w / 2.0;
    // The plate is measured, not fixed-width, so an unavailable reason makes it
    // wider than the track. Clamp it into the visible canvas on its own, or the
    // reason is what gets cut off at an edge.
    let visible_left = f64::from(visible.x);
    let visible_right = visible_left + f64::from(visible.width);
    let plate_x = if plate_w >= visible_right - visible_left {
        centred_x
    } else {
        centred_x.clamp(visible_left, visible_right - plate_w)
    };
    let plate_y = f64::from(bounds.y) - plate_h - READOUT_GAP;

    // A plate, because the canvas underneath is whatever the desktop shows.
    draw_rounded_rect(ctx, plate_x, plate_y, plate_w, plate_h, 4.0);
    theme::set_color(ctx, overlay::PANEL_BG_CONTEXT_MENU);
    let _ = ctx.fill();
    theme::set_color(
        ctx,
        if unavailable_reason.is_some() {
            overlay::TEXT_HINT
        } else {
            overlay::TEXT_PRIMARY
        },
    );
    draw_text_baseline(
        ctx,
        style,
        &label,
        plate_x + READOUT_PADDING - extents.x_bearing(),
        plate_y + READOUT_PADDING - extents.y_bearing(),
        None,
    );

    let _ = ctx.restore();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::Rect;

    const TRACK: Rect = Rect {
        x: 100,
        y: 40,
        width: 120,
        height: 12,
    };

    fn track_with_knob_at(offset: i32) -> SpotlightMagnificationTrack {
        SpotlightMagnificationTrack {
            track: TRACK,
            knob: Rect::new(TRACK.x + offset, TRACK.y, 12, 12).expect("knob"),
        }
    }

    /// A visible canvas larger than the surface, so the plate clamp is inert
    /// unless a test asks for it.
    const VISIBLE: Rect = Rect {
        x: 0,
        y: 0,
        width: 320,
        height: 120,
    };

    fn render(track: SpotlightMagnificationTrack, factor: f64, reason: Option<&str>) -> Vec<u8> {
        render_in(track, factor, reason, VISIBLE)
    }

    fn render_in(
        track: SpotlightMagnificationTrack,
        factor: f64,
        reason: Option<&str>,
        visible: Rect,
    ) -> Vec<u8> {
        let surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, 320, 120).expect("surface");
        {
            let ctx = cairo::Context::new(&surface).expect("context");
            render_spotlight_magnification_control(&ctx, track, factor, reason, visible);
        }
        let mut surface = surface;
        surface.flush();
        surface.data().expect("pixels").to_vec()
    }

    /// Bounding box of every pixel the control touched, in surface coordinates.
    fn painted_bounds(pixels: &[u8]) -> Option<(i32, i32, i32, i32)> {
        let (width, height) = (320i32, 120i32);
        let mut bounds: Option<(i32, i32, i32, i32)> = None;
        for y in 0..height {
            for x in 0..width {
                let alpha = pixels[(y * width + x) as usize * 4 + 3];
                if alpha == 0 {
                    continue;
                }
                bounds = Some(match bounds {
                    None => (x, y, x, y),
                    Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                });
            }
        }
        bounds
    }

    #[test]
    fn each_demonstrated_state_paints_something_of_its_own() {
        // The four states the control was signed off on. Every one must paint,
        // and no two may be pixel-identical, or a factor or a missing source
        // would be invisible on canvas.
        let states = [
            ("1x", render(track_with_knob_at(0), 1.0, None)),
            ("2.25x", render(track_with_knob_at(45), 2.25, None)),
            ("4x", render(track_with_knob_at(108), 4.0, None)),
            (
                "3x unavailable",
                render(
                    track_with_knob_at(72),
                    3.0,
                    Some("Freeze screen to preview"),
                ),
            ),
        ];
        for (name, pixels) in &states {
            assert!(
                painted_bounds(pixels).is_some(),
                "{name} painted nothing at all"
            );
        }
        for (i, (left_name, left)) in states.iter().enumerate() {
            for (right_name, right) in &states[i + 1..] {
                assert_ne!(left, right, "{left_name} and {right_name} render alike");
            }
        }
    }

    #[test]
    fn the_knob_position_is_what_changes_between_factors() {
        // Same track, different factors: the pixels must differ, or the control
        // would show 1x and 4x identically.
        assert_ne!(
            render(track_with_knob_at(0), 1.0, None),
            render(track_with_knob_at(108), 4.0, None)
        );
    }

    #[test]
    fn an_unavailable_source_widens_the_readout_instead_of_hiding_it() {
        let plain = render(track_with_knob_at(54), 2.5, None);
        let with_reason = render(
            track_with_knob_at(54),
            2.5,
            Some("Freeze screen to preview"),
        );
        assert_ne!(plain, with_reason);

        let (plain_x0, _, plain_x1, _) = painted_bounds(&plain).expect("plain control paints");
        let (reason_x0, _, reason_x1, _) =
            painted_bounds(&with_reason).expect("unavailable control paints");
        assert!(
            reason_x1 - reason_x0 > plain_x1 - plain_x0,
            "the reason has to fit on the plate, not be clipped away"
        );
    }

    #[test]
    fn a_wide_readout_is_pulled_inside_the_visible_canvas() {
        // The plate is sized by its text, so an unavailable reason makes it
        // wider than the 120px track. Centred on a track near the right edge it
        // would hang off; clamping keeps the reason readable.
        let narrow = Rect {
            x: 0,
            y: 0,
            width: 260,
            height: 120,
        };
        let track = SpotlightMagnificationTrack {
            track: Rect::new(136, 40, 120, 12).expect("edge track"),
            knob: Rect::new(190, 40, 12, 12).expect("knob"),
        };
        let pixels = render_in(track, 2.5, Some("Freeze screen to preview"), narrow);
        let (_, _, x1, _) = painted_bounds(&pixels).expect("control paints");
        assert!(
            x1 < narrow.x + narrow.width,
            "the readout ran past the visible canvas at x={x1}"
        );
    }

    #[test]
    fn the_control_paints_above_its_track_and_never_below_it() {
        // The readout sits above the track, so the painted region reaches
        // higher than `track.y` but never past its bottom edge. Anything drawn
        // below would land on the loupe the control belongs to.
        let pixels = render(
            track_with_knob_at(54),
            2.5,
            Some("Freeze screen to preview"),
        );
        let (x0, y0, x1, y1) = painted_bounds(&pixels).expect("control paints");

        assert!(y0 < TRACK.y, "the readout plate must sit above the track");
        assert!(
            y1 <= TRACK.y + TRACK.height,
            "nothing may be drawn below the track, got {y1}"
        );
        // The plate is centred on the track and may be wider than it; the
        // damage region the control needs is this box, not the shape's bounds.
        assert!(x0 <= TRACK.x && x1 >= TRACK.x + TRACK.width - 1);
    }
}
