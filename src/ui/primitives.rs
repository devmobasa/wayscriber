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

// ============================================================================
// Floating status badges
// ============================================================================

/// Interior padding used by floating status badges.
pub(crate) const BADGE_PADDING: f64 = 12.0;
/// Corner radius used by floating status badges.
pub(crate) const BADGE_RADIUS: f64 = 8.0;
/// Vertical gap between stacked floating badges.
pub(crate) const BADGE_STACK_GAP: f64 = 8.0;

/// Horizontal anchoring for [`draw_badge`].
pub(crate) enum BadgeAlign {
    /// `anchor_x` is the badge's left edge.
    Left,
    /// `anchor_x` is the badge's right edge.
    Right,
}

/// Draw a rounded, tinted status badge with a bold `label` and an optional
/// dimmer `(text, font_size)` hint line below it. Returns the measured badge
/// height so callers can stack badges without hardcoding heights.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_badge(
    ctx: &cairo::Context,
    anchor_x: f64,
    top_y: f64,
    align: BadgeAlign,
    label: &str,
    label_font_size: f64,
    hint: Option<(&str, f64)>,
    tint: [f64; 4],
) -> f64 {
    let padding = BADGE_PADDING;
    let label_layout = text_layout(
        ctx,
        UiTextStyle {
            family: "Sans",
            slant: cairo::FontSlant::Normal,
            weight: cairo::FontWeight::Bold,
            size: label_font_size,
        },
        label,
        None,
    );
    let label_extents = label_layout.ink_extents();

    let hint_layout = hint.map(|(text, font_size)| {
        let layout = text_layout(
            ctx,
            UiTextStyle {
                family: "Sans",
                slant: cairo::FontSlant::Normal,
                weight: cairo::FontWeight::Normal,
                size: font_size,
            },
            text,
            None,
        );
        let extents = layout.ink_extents();
        (layout, extents)
    });

    let (width, height, text_inset) = match &hint_layout {
        Some((_, hint_extents)) => (
            label_extents.width().max(hint_extents.width()) + padding * 1.6,
            label_extents.height() + hint_extents.height() + padding * 1.2,
            padding * 0.8,
        ),
        None => (
            label_extents.width() + padding * 1.4,
            label_extents.height() + padding,
            padding * 0.7,
        ),
    };

    let x = match align {
        BadgeAlign::Left => anchor_x,
        BadgeAlign::Right => anchor_x - width,
    };

    let [r, g, b, a] = tint;
    ctx.set_source_rgba(r, g, b, a);
    draw_rounded_rect(ctx, x, top_y, width, height, BADGE_RADIUS);
    let _ = ctx.fill();

    ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    match &hint_layout {
        Some((hint_text_layout, hint_extents)) => {
            label_layout.show_at_baseline(
                ctx,
                x + text_inset,
                top_y + label_extents.height() + padding * 0.3,
            );
            // Hint text (dimmer)
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.7);
            hint_text_layout.show_at_baseline(
                ctx,
                x + text_inset,
                top_y + label_extents.height() + hint_extents.height() + padding * 0.6,
            );
        }
        None => {
            label_layout.show_at_baseline(ctx, x + text_inset, top_y + height - padding * 0.35);
        }
    }

    height
}
