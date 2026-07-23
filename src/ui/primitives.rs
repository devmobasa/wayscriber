use std::f64::consts::{FRAC_PI_2, PI};

use crate::ui::theme::{self, Rgba};
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
// Floating island surfaces (M1 foundation; consumed by the HUD and island
// chrome from M2 on)
// ============================================================================

/// Number of layered strokes used to approximate a soft shadow (no gaussian —
/// this stays cheap on the 120fps no-vsync path).
const PILL_SHADOW_LAYERS: u32 = 3;

/// Draw a floating pill/panel surface: optional layered soft shadow, fill,
/// and a 1px hairline border. The canonical chrome surface for islands,
/// HUD segments, and popovers as they migrate to the theme.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_pill(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
    fill: Rgba,
    hairline: Rgba,
    shadow: Option<Rgba>,
) {
    if let Some((sr, sg, sb, sa)) = shadow {
        for layer in (1..=PILL_SHADOW_LAYERS).rev() {
            let spread = layer as f64;
            let alpha = sa * (1.0 - (layer as f64 - 1.0) / PILL_SHADOW_LAYERS as f64) * 0.35;
            ctx.set_source_rgba(sr, sg, sb, alpha);
            draw_rounded_rect(
                ctx,
                x - spread,
                y - spread + 1.5,
                width + spread * 2.0,
                height + spread * 2.0,
                radius + spread,
            );
            let _ = ctx.fill();
        }
    }

    theme::set_color(ctx, fill);
    draw_rounded_rect(ctx, x, y, width, height, radius);
    let _ = ctx.fill();

    theme::set_color(ctx, hairline);
    ctx.set_line_width(1.0);
    draw_rounded_rect(
        ctx,
        x + 0.5,
        y + 0.5,
        width - 1.0,
        height - 1.0,
        radius - 0.5,
    );
    let _ = ctx.stroke();
}

/// Keycap chip interior padding, as fractions of the label font size.
/// Shared by [`keycap_size`] and [`draw_keycap`] so pre-measured centering
/// can never drift from the drawn chip.
const KEYCAP_PAD_X_FACTOR: f64 = 0.5;
const KEYCAP_PAD_Y_FACTOR: f64 = 0.3;

/// Measured (width, height) the [`draw_keycap`] chip occupies for `label` at
/// `font_size`, for callers that need to center the chip before drawing it.
pub(crate) fn keycap_size(ctx: &cairo::Context, label: &str, font_size: f64) -> (f64, f64) {
    let layout = text_layout(
        ctx,
        UiTextStyle {
            family: "Sans",
            slant: cairo::FontSlant::Normal,
            weight: cairo::FontWeight::Bold,
            size: font_size,
        },
        label,
        None,
    );
    let extents = layout.ink_extents();
    (
        extents.width() + font_size * KEYCAP_PAD_X_FACTOR * 2.0,
        extents.height() + font_size * KEYCAP_PAD_Y_FACTOR * 2.0,
    )
}

/// Draw a flat keycap chip (rounded rect + centered label) and return its
/// (width, height). The single keycap language that replaces the per-surface
/// badge renderings as surfaces migrate (M2+).
pub(crate) fn draw_keycap(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    label: &str,
    font_size: f64,
    fill: Rgba,
    text_color: Rgba,
) -> (f64, f64) {
    let layout = text_layout(
        ctx,
        UiTextStyle {
            family: "Sans",
            slant: cairo::FontSlant::Normal,
            weight: cairo::FontWeight::Bold,
            size: font_size,
        },
        label,
        None,
    );
    let extents = layout.ink_extents();
    let pad_x = font_size * KEYCAP_PAD_X_FACTOR;
    let pad_y = font_size * KEYCAP_PAD_Y_FACTOR;
    let width = extents.width() + pad_x * 2.0;
    let height = extents.height() + pad_y * 2.0;

    theme::set_color(ctx, fill);
    draw_rounded_rect(ctx, x, y, width, height, theme::overlay::RADIUS_SM);
    let _ = ctx.fill();

    theme::set_color(ctx, text_color);
    layout.show_at_baseline(
        ctx,
        x + pad_x - extents.x_bearing(),
        y + pad_y - extents.y_bearing(),
    );
    (width, height)
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

/// Badge box `(width, height, text_inset)` from measured label/hint extents.
/// Shared by [`draw_badge`] and [`measure_badge`] so layout and rendering can
/// never disagree about badge geometry.
fn badge_box(
    label_extents: &crate::ui_text::UiTextExtents,
    hint_extents: Option<&crate::ui_text::UiTextExtents>,
) -> (f64, f64, f64) {
    let padding = BADGE_PADDING;
    match hint_extents {
        Some(hint_extents) => (
            label_extents.width().max(hint_extents.width()) + padding * 1.6,
            label_extents.height() + hint_extents.height() + padding * 1.2,
            padding * 0.8,
        ),
        None => (
            label_extents.width() + padding * 1.4,
            label_extents.height() + padding,
            padding * 0.7,
        ),
    }
}

/// Measure the `(width, height)` [`draw_badge`] would occupy, without a
/// rendering context (used for HUD badge stacking and damage geometry).
pub(crate) fn measure_badge(
    label: &str,
    label_font_size: f64,
    hint: Option<(&str, f64)>,
) -> Option<(f64, f64)> {
    let label_extents = crate::ui_text::measure_text(
        UiTextStyle {
            family: "Sans",
            slant: cairo::FontSlant::Normal,
            weight: cairo::FontWeight::Bold,
            size: label_font_size,
        },
        label,
        None,
    )?;
    let hint_extents = match hint {
        Some((text, font_size)) => Some(crate::ui_text::measure_text(
            UiTextStyle {
                family: "Sans",
                slant: cairo::FontSlant::Normal,
                weight: cairo::FontWeight::Normal,
                size: font_size,
            },
            text,
            None,
        )?),
        None => None,
    };
    let (width, height, _) = badge_box(&label_extents, hint_extents.as_ref());
    Some((width, height))
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

    let (width, height, text_inset) =
        badge_box(&label_extents, hint_layout.as_ref().map(|(_, ext)| ext));

    let x = match align {
        BadgeAlign::Left => anchor_x,
        BadgeAlign::Right => anchor_x - width,
    };

    let [r, g, b, a] = tint;
    ctx.set_source_rgba(r, g, b, a);
    draw_rounded_rect(ctx, x, top_y, width, height, BADGE_RADIUS);
    let _ = ctx.fill();

    theme::set_color(ctx, theme::overlay::TEXT_WHITE);
    match &hint_layout {
        Some((hint_text_layout, hint_extents)) => {
            label_layout.show_at_baseline(
                ctx,
                x + text_inset,
                top_y + label_extents.height() + padding * 0.3,
            );
            // Hint text (dimmer)
            theme::set_color(ctx, theme::with_alpha(theme::overlay::TEXT_WHITE, 0.7));
            hint_text_layout.show_at_baseline(
                ctx,
                x + text_inset,
                top_y + label_extents.height() + hint_extents.height() + padding * 0.6,
            );
        }
        None => {
            // Center the label ink vertically. A fixed baseline offset from
            // the pill bottom assumed all ink sits above the baseline (true
            // for all-caps FROZEN/ZOOM), but mixed-case labels like the
            // board/page badge's "Overlay | Page 1/2" have descenders that
            // dipped into the bottom padding and nearly touched the edge.
            let baseline =
                top_y + (height - label_extents.height()) / 2.0 - label_extents.y_bearing();
            label_layout.show_at_baseline(ctx, x + text_inset, baseline);
        }
    }

    height
}
