use super::types::RegionCaptureWindowVisual;
use super::{PANEL_FILL, PANEL_RADIUS};
use crate::ui::primitives::{draw_rounded_rect, text_extents_for_with_engine};
use crate::ui_text::UiTextEngine;

const LEGEND_FONT_SIZE: f64 = 12.0;
pub(super) const AREA_LEGEND_TEXT: &str =
    "Drag to select   Shift: square   Ctrl+A: all   Esc: cancel";
pub(super) const AREA_WITH_WINDOWS_LEGEND_TEXT: &str =
    "Drag to select   Shift: square   Ctrl+A: all   Space: window   Esc: cancel";
/// Recognition offers no square modifier, and `Ctrl+A` reads everything rather
/// than selecting everything, so it says what it does rather than borrowing
/// the capture picker's wording.
pub(crate) const OCR_LEGEND_TEXT: &str = "Drag to read text   Ctrl+A: whole screen   Esc: cancel";
const WINDOW_LEGEND_TEXT: &str =
    "Click: select   Super+Arrows: choose   Enter: select   Space: area   Esc: cancel";

pub(super) fn picker_legend_text(window: RegionCaptureWindowVisual<'_>) -> &'static str {
    if window.active {
        WINDOW_LEGEND_TEXT
    } else if window.available {
        AREA_WITH_WINDOWS_LEGEND_TEXT
    } else {
        AREA_LEGEND_TEXT
    }
}

/// The hint strip along the top of a region selector. Shared so every selector
/// teaches its keys the same way and in the same place.
pub(crate) fn render_region_legend(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    screen: (u32, u32),
    text: &str,
) {
    let extents = text_extents_for_with_engine(
        engine,
        ctx,
        "Sans",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        LEGEND_FONT_SIZE,
        text,
    );
    let screen_width = f64::from(screen.0);
    let screen_height = f64::from(screen.1);
    let width = (extents.width() + 24.0).min((screen_width - 12.0).max(0.0));
    let height = 28.0_f64.min((screen_height - 12.0).max(0.0));
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let x = ((screen_width - width) / 2.0).max(6.0);
    let y = 12.0_f64.min((screen_height - height).max(0.0));
    let radius = PANEL_RADIUS.min(width / 2.0).min(height / 2.0);
    ctx.set_source_rgba(PANEL_FILL.0, PANEL_FILL.1, PANEL_FILL.2, PANEL_FILL.3);
    draw_rounded_rect(ctx, x, y, width, height, radius);
    let _ = ctx.fill();
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.88);
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(LEGEND_FONT_SIZE);
    let text_x = x + ((width - extents.width()) / 2.0).max(6.0) - extents.x_bearing();
    let baseline = y + (height - extents.height()) / 2.0 - extents.y_bearing();
    let _ = ctx.save();
    ctx.rectangle(x, y, width, height);
    ctx.clip();
    ctx.move_to(text_x, baseline);
    let _ = ctx.show_text(text);
    let _ = ctx.restore();
}
