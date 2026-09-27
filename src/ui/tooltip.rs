//! Small text tooltips painted on the overlay surface, in the toolbar tooltip
//! style: the color picker's action buttons and the status bar's segments.

use crate::ui::constants::{self, RADIUS_SM, TEXT_PRIMARY};
use crate::ui::primitives::draw_rounded_rect;
use crate::ui::theme::toolbar as toolbar_theme;
use crate::ui_text::{UiTextEngine, UiTextStyle};

pub(crate) const TOOLTIP_PADDING_X: f64 = 8.0;
pub(crate) const TOOLTIP_PADDING_Y: f64 = 5.0;
const TOOLTIP_SHADOW_OFFSET: f64 = 2.0;

pub(crate) fn tooltip_text_style() -> UiTextStyle<'static> {
    UiTextStyle {
        family: toolbar_theme::FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: toolbar_theme::FONT_SIZE_TOOLTIP,
    }
}

/// Width and height of a tooltip showing `text`.
pub(crate) fn tooltip_size(engine: &UiTextEngine, text: &str) -> Option<(f64, f64)> {
    let style = tooltip_text_style();
    let extents = engine.measure(style, text, None)?;

    Some((
        extents.width() + TOOLTIP_PADDING_X * 2.0,
        style.size + TOOLTIP_PADDING_Y * 2.0,
    ))
}

/// Paint a tooltip showing `text` into `rect` (x, y, width, height), with its
/// drop shadow just outside the rect's bottom-right edge.
pub(crate) fn draw_tooltip(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    text: &str,
    rect: (f64, f64, f64, f64),
) {
    let (x, y, width, height) = rect;
    let style = tooltip_text_style();

    constants::set_color(ctx, toolbar_theme::COLOR_TOOLTIP_SHADOW);
    draw_rounded_rect(
        ctx,
        x + TOOLTIP_SHADOW_OFFSET,
        y + TOOLTIP_SHADOW_OFFSET,
        width,
        height,
        RADIUS_SM,
    );
    let _ = ctx.fill();

    constants::set_color(ctx, toolbar_theme::COLOR_TOOLTIP_BACKGROUND);
    draw_rounded_rect(ctx, x, y, width, height, RADIUS_SM);
    let _ = ctx.fill_preserve();
    constants::set_color(ctx, toolbar_theme::COLOR_TOOLTIP_BORDER);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    constants::set_color(ctx, TEXT_PRIMARY);
    engine.draw_baseline(
        ctx,
        style,
        text,
        x + TOOLTIP_PADDING_X,
        y + TOOLTIP_PADDING_Y + style.size,
        None,
    );
}

/// How far a tooltip's painted footprint reaches past its rect (the shadow).
pub(crate) const TOOLTIP_PAINT_OUTSET: f64 = TOOLTIP_SHADOW_OFFSET;
