//! Rendering for the help overlay header hint line and version pill.
//!
//! The hint line mirrors the keycap styling used throughout the grid rows so the
//! header reads as part of the same visual system instead of flat plain text.

use super::super::super::primitives::{draw_rounded_rect, text_extents_for};
use super::super::keycaps::{KeyComboStyle, draw_key_combo, measure_key_combo};
use crate::ui_text::{UiTextStyle, draw_text_baseline};

/// Vertical padding a keycap chip adds above and below the text baseline.
/// Mirrors `padding_y` in [`super::super::keycaps`].
pub(super) const KEYCAP_PAD_Y: f64 = 4.0;

const SEP_GAP: f64 = 12.0;
/// Gap between a keycap chip and its action label. Kept smaller than the
/// inter-hint bullet spacing so `chip + label` reads as one group.
const CHIP_LABEL_GAP: f64 = 10.0;
const BULLET: &str = "\u{2022}";

const PILL_PADDING_X: f64 = 9.0;
const PILL_PADDING_Y: f64 = 3.0;
const PILL_RADIUS: f64 = 6.0;
const TITLE_PILL_GAP: f64 = 12.0;

/// A single "keys → action" hint shown in the header.
pub(super) struct HeaderHint<'a> {
    pub(super) keys: &'a str,
    pub(super) label: &'a str,
}

/// Everything the header needs beyond the title itself.
pub(super) struct HeaderContent<'a> {
    /// Short version badge text, e.g. `v0.9.21`.
    pub(super) version: &'a str,
    /// Optional leading phrase drawn before the hints (quick-reference mode).
    pub(super) intro: Option<&'a str>,
    pub(super) hints: &'a [HeaderHint<'a>],
}

fn normal_style(font_family: &str, font_size: f64) -> UiTextStyle<'_> {
    UiTextStyle {
        family: font_family,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: font_size,
    }
}

/// Measure the total width the hint line occupies (intro + hints + separators).
pub(super) fn measure_hints(
    ctx: &cairo::Context,
    font_family: &str,
    font_size: f64,
    content: &HeaderContent<'_>,
) -> f64 {
    let text_width = |text: &str| {
        text_extents_for(
            ctx,
            font_family,
            cairo::FontSlant::Normal,
            cairo::FontWeight::Normal,
            font_size,
            text,
        )
        .width()
    };

    let mut width = 0.0;
    let mut has_leading = false;

    if let Some(intro) = content.intro {
        width += text_width(intro);
        has_leading = true;
    }

    for hint in content.hints {
        if has_leading {
            width += SEP_GAP + text_width(BULLET) + SEP_GAP;
        }
        width += measure_key_combo(ctx, hint.keys, font_family, font_size);
        width += CHIP_LABEL_GAP + text_width(hint.label);
        has_leading = true;
    }

    width
}

/// Draw the hint line starting at `x`, on `baseline`. Returns the width drawn.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_hints(
    ctx: &cairo::Context,
    x: f64,
    baseline: f64,
    font_family: &str,
    font_size: f64,
    content: &HeaderContent<'_>,
    key_combo_style: &KeyComboStyle<'_>,
    label_color: [f64; 4],
    muted_color: [f64; 4],
) -> f64 {
    let style = normal_style(font_family, font_size);
    let mut cursor_x = x;
    let mut has_leading = false;

    let draw_separator = |ctx: &cairo::Context, cursor_x: &mut f64| {
        *cursor_x += SEP_GAP;
        ctx.set_source_rgba(
            muted_color[0],
            muted_color[1],
            muted_color[2],
            muted_color[3],
        );
        let ext = draw_text_baseline(ctx, style, BULLET, *cursor_x, baseline, None);
        *cursor_x += ext.width() + SEP_GAP;
    };

    if let Some(intro) = content.intro {
        ctx.set_source_rgba(
            label_color[0],
            label_color[1],
            label_color[2],
            label_color[3],
        );
        let ext = draw_text_baseline(ctx, style, intro, cursor_x, baseline, None);
        cursor_x += ext.width();
        has_leading = true;
    }

    for hint in content.hints {
        if has_leading {
            draw_separator(ctx, &mut cursor_x);
        }

        let combo_width = draw_key_combo(ctx, cursor_x, baseline, hint.keys, key_combo_style);
        cursor_x += combo_width + CHIP_LABEL_GAP;

        ctx.set_source_rgba(
            label_color[0],
            label_color[1],
            label_color[2],
            label_color[3],
        );
        let label_ext = draw_text_baseline(ctx, style, hint.label, cursor_x, baseline, None);
        cursor_x += label_ext.width();

        has_leading = true;
    }

    cursor_x - x
}

/// Width of the version pill (rounded chip) for the given text.
pub(super) fn measure_version_pill(
    ctx: &cairo::Context,
    font_family: &str,
    font_size: f64,
    version: &str,
) -> f64 {
    let text_width = text_extents_for(
        ctx,
        font_family,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
        font_size,
        version,
    )
    .width();
    text_width + PILL_PADDING_X * 2.0
}

/// Extra width the title row needs so the title and version pill never overlap.
pub(super) fn title_row_width(
    ctx: &cairo::Context,
    font_family: &str,
    title_font_size: f64,
    pill_font_size: f64,
    title: &str,
    version: &str,
) -> f64 {
    let title_width = text_extents_for(
        ctx,
        font_family,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
        title_font_size,
        title,
    )
    .width();
    let pill_width = measure_version_pill(ctx, font_family, pill_font_size, version);
    title_width + TITLE_PILL_GAP + pill_width
}

/// Draw the version pill so its right edge sits at `right_edge`, vertically
/// centred against a title whose baseline is `title_baseline`.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_version_pill(
    ctx: &cairo::Context,
    right_edge: f64,
    title_baseline: f64,
    title_font_size: f64,
    font_family: &str,
    font_size: f64,
    version: &str,
    accent: [f64; 4],
    text_color: [f64; 4],
) {
    let pill_width = measure_version_pill(ctx, font_family, font_size, version);
    let pill_height = font_size + PILL_PADDING_Y * 2.0;
    let pill_x = right_edge - pill_width;
    // Centre the pill against the title's optical centre (~0.34 of the cap height
    // above the baseline) rather than the baseline itself.
    let title_center = title_baseline - title_font_size * 0.34;
    let pill_y = title_center - pill_height / 2.0;

    draw_rounded_rect(ctx, pill_x, pill_y, pill_width, pill_height, PILL_RADIUS);
    ctx.set_source_rgba(accent[0], accent[1], accent[2], 0.14);
    let _ = ctx.fill();

    draw_rounded_rect(ctx, pill_x, pill_y, pill_width, pill_height, PILL_RADIUS);
    ctx.set_source_rgba(accent[0], accent[1], accent[2], 0.38);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    let text_style = UiTextStyle {
        family: font_family,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: font_size,
    };
    let text_baseline = pill_y + PILL_PADDING_Y + font_size * 0.82;
    ctx.set_source_rgba(text_color[0], text_color[1], text_color[2], text_color[3]);
    draw_text_baseline(
        ctx,
        text_style,
        version,
        pill_x + PILL_PADDING_X,
        text_baseline,
        None,
    );
}
