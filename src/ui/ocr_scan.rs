use crate::input::state::{OcrScanOutcome, result_opacity};
use crate::ui::theme::{self, Rgba, overlay};
use crate::util::Rect;

use super::primitives::draw_rounded_rect;
use crate::ui_text::{UiTextStyle, measure_text, text_layout};

/// Tint held over the region for the whole sweep, so the scanned area stays
/// identifiable even at the moment the band is off its top edge.
const TINT: Rgba = theme::rgba(theme::ACCENT_RGB, 0.14);
const FRAME: Rgba = theme::rgba(theme::ACCENT_RGB, 0.95);
const FRAME_WIDTH: f64 = 1.5;
/// The band is a fraction of the region's height, bounded so it neither
/// disappears on a tall region nor swamps a short one.
const BAND_FRACTION: f64 = 0.35;
const BAND_MIN: f64 = 18.0;
const BAND_MAX: f64 = 64.0;

const CARD_RADIUS: f64 = overlay::RADIUS_LG;
const CARD_PAD: f64 = 12.0;
const CARD_GAP: f64 = 6.0;
const CARD_MARGIN: f64 = 12.0;
const CARD_OFFSET: f64 = 14.0;
const HEADLINE_SIZE: f64 = 12.5;
const DETAIL_SIZE: f64 = 11.0;

/// Where the outcome card sits, in logical surface pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OcrScanCard {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

fn headline_style() -> UiTextStyle<'static> {
    UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: HEADLINE_SIZE,
    }
}

fn detail_style() -> UiTextStyle<'static> {
    UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: DETAIL_SIZE,
    }
}

/// Card text block size, measured without a drawing context so damage
/// geometry and the drawn card cannot disagree. Both go through the shared
/// measurement cache.
fn card_text_size(outcome: OcrScanOutcome) -> Option<(f64, f64)> {
    let headline = measure_text(headline_style(), outcome.headline(), None)?;
    let Some(detail) = outcome.detail() else {
        return Some((headline.width(), headline.height()));
    };
    let detail = measure_text(detail_style(), &detail, None)?;
    Some((
        headline.width().max(detail.width()),
        headline.height() + CARD_GAP + detail.height(),
    ))
}

/// Place the outcome card just under the scanned region, flipping above it when
/// there is no room and clamping onto the surface either way.
pub(crate) fn ocr_scan_card(
    region: Rect,
    outcome: OcrScanOutcome,
    screen: (u32, u32),
) -> Option<OcrScanCard> {
    let (text_width, text_height) = card_text_size(outcome)?;
    let screen_width = f64::from(screen.0);
    let screen_height = f64::from(screen.1);
    let width = (text_width + CARD_PAD * 2.0).min((screen_width - CARD_MARGIN * 2.0).max(1.0));
    let height = (text_height + CARD_PAD * 2.0).min((screen_height - CARD_MARGIN * 2.0).max(1.0));

    let bottom = f64::from(region.y.saturating_add(region.height));
    let below = bottom + CARD_OFFSET;
    let preferred_y = if below + height + CARD_MARGIN <= screen_height {
        below
    } else {
        f64::from(region.y) - CARD_OFFSET - height
    };
    Some(OcrScanCard {
        x: f64::from(region.x).clamp(
            CARD_MARGIN,
            (screen_width - width - CARD_MARGIN).max(CARD_MARGIN),
        ),
        y: preferred_y.clamp(
            CARD_MARGIN,
            (screen_height - height - CARD_MARGIN).max(CARD_MARGIN),
        ),
        width,
        height,
    })
}

/// Union of everything the overlay paints, for targeted damage.
pub(crate) fn ocr_scan_geometry(
    region: Rect,
    outcome: Option<OcrScanOutcome>,
    screen: (u32, u32),
) -> (f64, f64, f64, f64) {
    let region_box = (
        f64::from(region.x) - FRAME_WIDTH,
        f64::from(region.y) - FRAME_WIDTH,
        f64::from(region.width) + FRAME_WIDTH * 2.0,
        f64::from(region.height) + FRAME_WIDTH * 2.0,
    );
    let Some(card) = outcome.and_then(|outcome| ocr_scan_card(region, outcome, screen)) else {
        return region_box;
    };
    let left = region_box.0.min(card.x);
    let top = region_box.1.min(card.y);
    let right = (region_box.0 + region_box.2).max(card.x + card.width);
    let bottom = (region_box.1 + region_box.3).max(card.y + card.height);
    (left, top, right - left, bottom - top)
}

/// The region marked as being read, with no moving parts. Used under
/// `[ui] reduced_motion`, where a band sweeping the screen is exactly the kind
/// of motion the setting exists to suppress (WCAG 2.3.3).
pub(crate) fn render_ocr_scan_still(ctx: &cairo::Context, region: Rect) {
    if region.width <= 0 || region.height <= 0 {
        return;
    }
    let _ = ctx.save();
    theme::set_color(ctx, TINT);
    ctx.rectangle(
        f64::from(region.x),
        f64::from(region.y),
        f64::from(region.width),
        f64::from(region.height),
    );
    let _ = ctx.fill();
    let _ = ctx.restore();
    draw_frame(
        ctx,
        f64::from(region.x),
        f64::from(region.y),
        f64::from(region.width),
        f64::from(region.height),
        1.0,
    );
}

/// The sweeping band, drawn while recognition runs. `progress` walks `0.0..1.0`
/// once per pass.
pub(crate) fn render_ocr_scan_sweep(ctx: &cairo::Context, region: Rect, progress: f64) {
    if region.width <= 0 || region.height <= 0 {
        return;
    }
    let x = f64::from(region.x);
    let y = f64::from(region.y);
    let width = f64::from(region.width);
    let height = f64::from(region.height);

    let _ = ctx.save();
    theme::set_color(ctx, TINT);
    ctx.rectangle(x, y, width, height);
    let _ = ctx.fill();

    // The band enters from above the region and leaves below it, so the sweep
    // covers the top and bottom edges instead of appearing to start inset.
    let band = (height * BAND_FRACTION).clamp(BAND_MIN, BAND_MAX);
    let band_y = y - band + progress.clamp(0.0, 1.0) * (height + band);
    ctx.rectangle(x, y, width, height);
    ctx.clip();
    let gradient = build_band(band_y, band);
    let _ = ctx.set_source(&gradient);
    ctx.rectangle(x, band_y, width, band);
    let _ = ctx.fill();
    let _ = ctx.restore();

    draw_frame(ctx, x, y, width, height, 1.0);
}

fn build_band(top: f64, height: f64) -> cairo::LinearGradient {
    let gradient = cairo::LinearGradient::new(0.0, top, 0.0, top + height);
    let (r, g, b) = theme::ACCENT_RGB;
    gradient.add_color_stop_rgba(0.0, r, g, b, 0.0);
    gradient.add_color_stop_rgba(0.8, r, g, b, 0.5);
    gradient.add_color_stop_rgba(1.0, 1.0, 1.0, 1.0, 0.9);
    gradient
}

fn draw_frame(ctx: &cairo::Context, x: f64, y: f64, width: f64, height: f64, alpha: f64) {
    theme::set_color(ctx, (FRAME.0, FRAME.1, FRAME.2, FRAME.3 * alpha));
    ctx.set_line_width(FRAME_WIDTH);
    ctx.rectangle(x, y, width, height);
    let _ = ctx.stroke();
}

/// The outcome card. `shown` is how long it has been up, which drives its fade.
pub(crate) fn render_ocr_scan_result(
    ctx: &cairo::Context,
    region: Rect,
    outcome: OcrScanOutcome,
    shown: std::time::Duration,
    screen: (u32, u32),
) {
    let opacity = result_opacity(shown);
    if opacity <= 0.0 {
        return;
    }
    let Some(card) = ocr_scan_card(region, outcome, screen) else {
        return;
    };
    let _ = ctx.save();
    ctx.push_group();

    draw_frame(
        ctx,
        f64::from(region.x),
        f64::from(region.y),
        f64::from(region.width),
        f64::from(region.height),
        1.0,
    );

    theme::set_color(ctx, crate::ui::theme::popup::bg_context_menu());
    draw_rounded_rect(ctx, card.x, card.y, card.width, card.height, CARD_RADIUS);
    let _ = ctx.fill();
    theme::set_color(ctx, crate::ui::theme::popup::border_context_menu());
    ctx.set_line_width(1.0);
    draw_rounded_rect(
        ctx,
        card.x + 0.5,
        card.y + 0.5,
        card.width - 1.0,
        card.height - 1.0,
        CARD_RADIUS - 0.5,
    );
    let _ = ctx.stroke();

    let _ = ctx.save();
    ctx.rectangle(card.x, card.y, card.width, card.height);
    ctx.clip();
    let headline = text_layout(ctx, headline_style(), outcome.headline(), None);
    let headline_extents = headline.ink_extents();
    theme::set_color(ctx, overlay::TEXT_PRIMARY);
    headline.show_at_baseline(
        ctx,
        card.x + CARD_PAD - headline_extents.x_bearing(),
        card.y + CARD_PAD - headline_extents.y_bearing(),
    );
    if let Some(detail) = outcome.detail() {
        let layout = text_layout(ctx, detail_style(), &detail, None);
        let extents = layout.ink_extents();
        theme::set_color(ctx, overlay::TEXT_TERTIARY);
        layout.show_at_baseline(
            ctx,
            card.x + CARD_PAD - extents.x_bearing(),
            card.y + CARD_PAD + headline_extents.height() + CARD_GAP - extents.y_bearing(),
        );
    }
    let _ = ctx.restore();

    let _ = ctx.pop_group_to_source();
    let _ = ctx.paint_with_alpha(opacity);
    let _ = ctx.restore();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn region() -> Rect {
        Rect::new(60, 50, 200, 140).expect("a scan region")
    }

    fn copied() -> OcrScanOutcome {
        OcrScanOutcome::Copied {
            character_count: 128,
            replaced_invalid_utf8: false,
        }
    }

    fn alpha_at(surface: &mut cairo::ImageSurface, x: usize, y: usize) -> u8 {
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().expect("pixels");
        data[y * stride + x * 4 + 3]
    }

    #[test]
    fn the_band_covers_the_regions_edges_at_the_ends_of_its_pass() {
        // The band enters from above and leaves below, so a sweep touches the
        // first and last rows rather than starting inset.
        let render = |progress: f64| {
            let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 320, 260).unwrap();
            let ctx = cairo::Context::new(&surface).unwrap();
            render_ocr_scan_sweep(&ctx, region(), progress);
            drop(ctx);
            surface
        };

        let mut start = render(0.0);
        assert!(alpha_at(&mut start, 160, 51) > 0, "the top row is lit");
        let mut end = render(1.0);
        assert!(alpha_at(&mut end, 160, 188) > 0, "the bottom row is lit");

        // The tint holds over the whole region throughout, so the scanned area
        // stays identifiable between passes.
        let mut mid = render(0.5);
        assert!(alpha_at(&mut mid, 160, 60) > 0);
        assert_eq!(alpha_at(&mut mid, 10, 10), 0, "nothing outside the region");
    }

    #[test]
    fn the_still_indicator_marks_the_region_without_a_band() {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 320, 260).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        render_ocr_scan_still(&ctx, region());
        drop(ctx);
        let mut surface = surface;

        // The tint and frame mark the area, evenly: no row is brighter than
        // another, because nothing is travelling across it.
        assert!(alpha_at(&mut surface, 160, 60) > 0, "the region is tinted");
        assert_eq!(
            alpha_at(&mut surface, 160, 60),
            alpha_at(&mut surface, 160, 180),
            "no band, so every row inside reads the same"
        );
        assert_eq!(alpha_at(&mut surface, 10, 10), 0, "nothing outside");
    }

    #[test]
    fn the_card_sits_under_the_region_and_flips_above_it_when_it_must() {
        let below = ocr_scan_card(region(), copied(), (320, 400)).expect("a card");
        assert!(
            below.y >= f64::from(region().y + region().height),
            "the usual place is under the scanned area"
        );

        let low = Rect::new(60, 300, 200, 90).expect("a low region");
        let flipped = ocr_scan_card(low, copied(), (320, 400)).expect("a card");
        assert!(
            flipped.y + flipped.height <= f64::from(low.y),
            "no room below, so it goes above"
        );
    }

    #[test]
    fn a_card_with_nowhere_to_go_is_still_placed_on_the_surface() {
        let full = Rect::new(0, 0, 200, 200).expect("a full-surface region");
        let card = ocr_scan_card(full, copied(), (200, 200)).expect("a card");
        assert!(card.x >= 0.0 && card.y >= 0.0);
        assert!(card.x + card.width <= 200.0);
        assert!(card.y + card.height <= 200.0);
    }

    #[test]
    fn damage_covers_the_region_alone_while_scanning_and_the_card_once_settled() {
        let scanning = ocr_scan_geometry(region(), None, (320, 400));
        assert!(scanning.0 <= f64::from(region().x));
        assert!(scanning.1 <= f64::from(region().y));
        assert!(scanning.0 + scanning.2 >= f64::from(region().x + region().width));

        let settled = ocr_scan_geometry(region(), Some(copied()), (320, 400));
        let card = ocr_scan_card(region(), copied(), (320, 400)).expect("a card");
        assert!(
            settled.1 + settled.3 >= card.y + card.height,
            "the union has to reach the card or its pixels are never cleared"
        );
        assert!(settled.3 > scanning.3, "settling grows the damaged area");
    }

    #[test]
    fn the_result_card_paints_its_outcome_and_fades_to_nothing() {
        let render = |shown: Duration| {
            let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 320, 400).unwrap();
            let ctx = cairo::Context::new(&surface).unwrap();
            render_ocr_scan_result(&ctx, region(), copied(), shown, (320, 400));
            drop(ctx);
            surface
        };
        let card = ocr_scan_card(region(), copied(), (320, 400)).expect("a card");
        let probe = (
            (card.x + card.width / 2.0) as usize,
            (card.y + card.height / 2.0) as usize,
        );

        let mut fresh = render(Duration::ZERO);
        assert!(alpha_at(&mut fresh, probe.0, probe.1) > 0, "the card is up");

        let mut expired = render(Duration::from_secs(60));
        assert_eq!(
            alpha_at(&mut expired, probe.0, probe.1),
            0,
            "an expired card paints nothing at all, whatever the motion setting"
        );
    }
}
