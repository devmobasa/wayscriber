use std::f64::consts::{FRAC_PI_2, PI};

use crate::ui::theme::{self, Rgba};
use crate::ui_text::{UiTextEngine, UiTextStyle, measure_text, text_layout, with_legacy_engine};

pub(crate) fn text_extents_for(
    ctx: &cairo::Context,
    family: &str,
    slant: cairo::FontSlant,
    weight: cairo::FontWeight,
    size: f64,
    text: &str,
) -> cairo::TextExtents {
    with_legacy_engine(|engine| {
        text_extents_for_with_engine(engine, ctx, family, slant, weight, size, text)
    })
}

pub(crate) fn text_extents_for_with_engine(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    family: &str,
    slant: cairo::FontSlant,
    weight: cairo::FontWeight,
    size: f64,
    text: &str,
) -> cairo::TextExtents {
    let layout = engine.layout(
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

/// Ellipsis used when text is trimmed to fit a measured width.
pub(crate) const ELLIPSIS: &str = "\u{2026}";

/// Trim `text` to `max_width` logical pixels, appending an ellipsis. The
/// complete string is measured as it will be shaped, so wide glyphs and
/// non-Latin scripts cannot slip past a per-character budget.
pub(crate) fn ellipsize_to_fit(
    ctx: &cairo::Context,
    text: &str,
    font_family: &str,
    font_size: f64,
    weight: cairo::FontWeight,
    max_width: f64,
) -> String {
    with_legacy_engine(|engine| {
        ellipsize_to_fit_with_engine(engine, ctx, text, font_family, font_size, weight, max_width)
    })
}

pub(crate) fn ellipsize_to_fit_with_engine(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    text: &str,
    font_family: &str,
    font_size: f64,
    weight: cairo::FontWeight,
    max_width: f64,
) -> String {
    let extents = text_extents_for_with_engine(
        engine,
        ctx,
        font_family,
        cairo::FontSlant::Normal,
        weight,
        font_size,
        text,
    );
    if extents.width() <= max_width {
        return text.to_string();
    }

    let ellipsis = ELLIPSIS;
    let ellipsis_extents = text_extents_for_with_engine(
        engine,
        ctx,
        font_family,
        cairo::FontSlant::Normal,
        weight,
        font_size,
        ellipsis,
    );
    if ellipsis_extents.width() > max_width {
        return String::new();
    }

    let mut end = text.len();
    while end > 0 {
        if !text.is_char_boundary(end) {
            end -= 1;
            continue;
        }
        let candidate = format!("{}{}", &text[..end], ellipsis);
        let candidate_extents = text_extents_for_with_engine(
            engine,
            ctx,
            font_family,
            cairo::FontSlant::Normal,
            weight,
            font_size,
            &candidate,
        );
        if candidate_extents.width() <= max_width {
            return candidate;
        }
        end -= 1;
    }

    ellipsis.to_string()
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

/// Alpha-checkerboard tiles. Mid-grays rather than the usual white/light-gray
/// pair because every surface that shows them is a dark panel.
const CHECKER_LIGHT: Rgba = (0.6, 0.6, 0.6, 1.0);
const CHECKER_DARK: Rgba = (0.4, 0.4, 0.4, 1.0);
/// Checkerboard tile edge length.
const CHECKER_TILE: f64 = 6.0;

/// Lay the alpha checkerboard behind a translucent fill, clipped to the shape
/// `path` builds. A no-op at full alpha, so swatch painters can call it
/// unconditionally before their fill.
///
/// This owns the clip/extents/restore dance every non-rectangular swatch would
/// otherwise repeat: rounded squares would spill tiles past their corners, and
/// the radial ring's annular segments have no rectangle to fill at all. Plain
/// rectangles need no clip and call [`draw_alpha_checkerboard`] directly.
///
/// `path` must leave a path on `ctx`; it is consumed by the clip, so callers
/// rebuild it for their own fill.
pub(crate) fn checkerboard_behind(
    ctx: &cairo::Context,
    alpha: f64,
    path: impl Fn(&cairo::Context),
) {
    if alpha >= 1.0 {
        return;
    }
    let _ = ctx.save();
    path(ctx);
    ctx.clip();
    // The checkerboard only has to cover the clipped shape, and the clip's
    // bounding box is exactly that without the caller measuring its own path.
    if let Ok((x0, y0, x1, y1)) = ctx.clip_extents() {
        draw_alpha_checkerboard(ctx, x0, y0, x1 - x0, y1 - y0);
    }
    let _ = ctx.restore();
}

/// Fill `(x, y, width, height)` with the alpha checkerboard; the caller paints
/// its translucent color on top. Shared by the picker popup and both toolbar
/// frontends so translucency reads the same everywhere. Tiles are clipped to
/// the rectangle; for a non-rectangular swatch use [`checkerboard_behind`].
pub(crate) fn draw_alpha_checkerboard(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) {
    theme::set_color(ctx, CHECKER_LIGHT);
    ctx.rectangle(x, y, width, height);
    let _ = ctx.fill();

    theme::set_color(ctx, CHECKER_DARK);
    let mut tile_y = y;
    let mut row = 0;
    while tile_y < y + height {
        let mut tile_x = x + if row % 2 == 0 { 0.0 } else { CHECKER_TILE };
        while tile_x < x + width {
            ctx.rectangle(
                tile_x,
                tile_y,
                (x + width - tile_x).min(CHECKER_TILE),
                (y + height - tile_y).min(CHECKER_TILE),
            );
            tile_x += CHECKER_TILE * 2.0;
        }
        tile_y += CHECKER_TILE;
        row += 1;
    }
    let _ = ctx.fill();
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
    with_legacy_engine(|engine| keycap_size_with_engine(engine, ctx, label, font_size))
}

pub(crate) fn keycap_size_with_engine(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    label: &str,
    font_size: f64,
) -> (f64, f64) {
    let layout = engine.layout(
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
    with_legacy_engine(|engine| {
        draw_keycap_with_engine(engine, ctx, x, y, label, font_size, fill, text_color)
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_keycap_with_engine(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    label: &str,
    font_size: f64,
    fill: Rgba,
    text_color: Rgba,
) -> (f64, f64) {
    let layout = engine.layout(
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

/// Font style shared by every keycap chip, so headless measurement and
/// rendering shape the same layout.
pub(crate) fn keycap_text_style(font_size: f64) -> UiTextStyle<'static> {
    UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: font_size,
    }
}

/// [`keycap_size`] without a rendering context, for callers that lay out
/// before a frame buffer exists (damage geometry). Goes through the shared
/// measurement cache, so it agrees with the drawn chip exactly.
pub(crate) fn keycap_box_size(label: &str, font_size: f64) -> Option<(f64, f64)> {
    let extents = measure_text(keycap_text_style(font_size), label, None)?;
    Some((
        extents.width() + font_size * KEYCAP_PAD_X_FACTOR * 2.0,
        extents.height() + font_size * KEYCAP_PAD_Y_FACTOR * 2.0,
    ))
}

/// Draw a keycap chip into a caller-provided box, centering the label inside
/// it. Rows of chips use this so a shared row height survives labels with
/// different ascenders and descenders; [`draw_keycap`] is the natural-size
/// shorthand over the same chrome.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_keycap_in_box(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    label: &str,
    font_size: f64,
    fill: Rgba,
    text_color: Rgba,
) {
    let layout = text_layout(ctx, keycap_text_style(font_size), label, None);
    let extents = layout.ink_extents();

    theme::set_color(ctx, fill);
    draw_rounded_rect(ctx, x, y, width, height, theme::overlay::RADIUS_SM);
    let _ = ctx.fill();

    theme::set_color(ctx, text_color);
    layout.show_at_baseline(
        ctx,
        x + (width - extents.width()) / 2.0 - extents.x_bearing(),
        y + (height - extents.height()) / 2.0 - extents.y_bearing(),
    );
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
/// Shared by [`draw_badge`] and [`measure_badge_with_engine`] so layout and rendering can
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
pub(crate) fn measure_badge_with_engine(
    engine: &UiTextEngine,
    label: &str,
    label_font_size: f64,
    hint: Option<(&str, f64)>,
) -> Option<(f64, f64)> {
    let label_extents = engine.measure(
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
        Some((text, font_size)) => Some(engine.measure(
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
    with_legacy_engine(|engine| {
        draw_badge_with_engine(
            engine,
            ctx,
            anchor_x,
            top_y,
            align,
            label,
            label_font_size,
            hint,
            tint,
        )
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_badge_with_engine(
    engine: &UiTextEngine,
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
    let label_layout = engine.layout(
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
        let layout = engine.layout(
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

#[cfg(test)]
mod tests {
    use super::*;
    use cairo::{Context, Format, ImageSurface};

    /// `(r, g, b)` of one pixel, 0-255. Rgb24 packs pixels as native-endian
    /// 32-bit words, so on little-endian the byte order is B, G, R, unused.
    fn pixel_at(surface: &mut ImageSurface, x: i32, y: i32) -> (u8, u8, u8) {
        let stride = surface.stride() as usize;
        let offset = y as usize * stride + x as usize * 4;
        let data = surface.data().expect("pixel data");
        (data[offset + 2], data[offset + 1], data[offset])
    }

    /// Run `checkerboard_behind` at `alpha` over a white surface, clipped to a
    /// circle centered at (32, 32) with radius 20, and sample a point inside
    /// the circle and a corner outside it.
    fn inside_and_outside(alpha: f64) -> ((u8, u8, u8), (u8, u8, u8)) {
        let surface = ImageSurface::create(Format::Rgb24, 64, 64).expect("surface");
        {
            let ctx = Context::new(&surface).expect("context");
            ctx.set_source_rgb(1.0, 1.0, 1.0);
            let _ = ctx.paint();
            checkerboard_behind(&ctx, alpha, |ctx| {
                ctx.arc(32.0, 32.0, 20.0, 0.0, PI * 2.0);
            });
        }
        let mut surface = surface;
        (pixel_at(&mut surface, 32, 32), pixel_at(&mut surface, 1, 1))
    }

    #[test]
    fn checkerboard_behind_fills_only_inside_the_path() {
        let (inside, outside) = inside_and_outside(0.5);
        assert_ne!(inside, (255, 255, 255), "no checkerboard inside the path");
        assert_eq!(
            outside,
            (255, 255, 255),
            "checkerboard leaked outside the path: {outside:?}"
        );
    }

    #[test]
    fn checkerboard_behind_is_a_no_op_at_full_alpha() {
        let (inside, outside) = inside_and_outside(1.0);
        assert_eq!(inside, (255, 255, 255));
        assert_eq!(outside, (255, 255, 255));
    }

    #[test]
    fn checkerboard_behind_paints_both_tile_shades() {
        // A single flat gray would read as a dark color rather than as
        // transparency, so the pattern has to alternate.
        let surface = ImageSurface::create(Format::Rgb24, 64, 64).expect("surface");
        {
            let ctx = Context::new(&surface).expect("context");
            checkerboard_behind(&ctx, 0.5, |ctx| ctx.rectangle(0.0, 0.0, 64.0, 64.0));
        }
        let mut surface = surface;
        let first = pixel_at(&mut surface, 2, 2);
        let neighbor = pixel_at(&mut surface, 2 + CHECKER_TILE as i32, 2);
        assert_ne!(first, neighbor, "tiles did not alternate: {first:?}");
    }
}
