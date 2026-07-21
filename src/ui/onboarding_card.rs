use super::primitives::draw_rounded_rect;
use super::theme::{self, Rgba, overlay};
use crate::ui_text::{UiTextStyle, draw_text_baseline, text_layout};

pub struct OnboardingChecklistItem {
    pub label: String,
    pub done: bool,
}

pub struct OnboardingCard {
    pub eyebrow: String,
    pub title: String,
    pub body: String,
    pub items: Vec<OnboardingChecklistItem>,
    pub footer: String,
}

const CARD_MARGIN: f64 = 20.0;
const CARD_MAX_WIDTH: f64 = 460.0;
const CARD_MIN_WIDTH: f64 = 300.0;
const CARD_PADDING: f64 = overlay::SPACING_LG;
const CARD_RADIUS: f64 = overlay::RADIUS_PANEL;
const ITEM_DOT_SIZE: f64 = 10.0;
const ITEM_GAP_Y: f64 = 24.0;
const TEXT_OFFSET_Y: f64 = 5.0;
const EYEBROW_CONTENT_HEIGHT: f64 = 20.0;
const TITLE_CONTENT_HEIGHT: f64 = 30.0;
const BODY_BOTTOM_GAP: f64 = overlay::SPACING_STD;
const FOOTER_CONTENT_HEIGHT: f64 = 20.0;
/// Type-scale multiplier applied uniformly to every metric and font size on
/// this card. The layout constants above are designed at 1x; the first-run
/// card renders 1.3x larger so it stays legible from a distance on an
/// otherwise empty overlay (there is no user-configurable scaling here).
const CARD_TYPE_SCALE: f64 = 1.3;
const ELLIPSIS: &str = "...";

// ---- Colors ----
// File-local values carry the card's slightly deeper, cooler look; only the
// exact matches point at theme tokens (see `theme::overlay`).
/// Card fill: deeper than the standard panel backgrounds so the card reads
/// without a backdrop dim.
const CARD_BG: Rgba = (0.07, 0.09, 0.12, 0.96);
/// Card hairline border (cool steel; no matching theme token).
const CARD_BORDER: Rgba = (0.36, 0.46, 0.58, 0.8);
/// Eyebrow/kicker line above the title (desaturated accent tint).
const TEXT_EYEBROW: Rgba = (0.65, 0.74, 0.88, 1.0);
/// Body copy (sits between the overlay primary and secondary text tones).
const TEXT_BODY: Rgba = (0.78, 0.84, 0.92, 1.0);
/// Footer hint text.
const TEXT_FOOTER: Rgba = (0.60, 0.68, 0.78, 1.0);
/// Checklist dot for a completed item (success green).
const DOT_DONE: Rgba = (0.30, 0.82, 0.52, 1.0);
/// Checklist dot for a pending item.
const DOT_PENDING: Rgba = (0.44, 0.52, 0.62, 1.0);
/// Checkmark stroke drawn over a completed dot.
const CHECKMARK: Rgba = (0.96, 1.0, 0.97, 1.0);

pub fn render_onboarding_card(
    ctx: &cairo::Context,
    width: u32,
    height: u32,
    card: &OnboardingCard,
) {
    let margin = CARD_MARGIN * CARD_TYPE_SCALE;
    let card_max_width = CARD_MAX_WIDTH * CARD_TYPE_SCALE;
    let card_min_width = CARD_MIN_WIDTH * CARD_TYPE_SCALE;
    let card_padding = CARD_PADDING * CARD_TYPE_SCALE;
    let card_radius = CARD_RADIUS * CARD_TYPE_SCALE;
    let item_dot_size = ITEM_DOT_SIZE * CARD_TYPE_SCALE;
    let item_gap_y = ITEM_GAP_Y * CARD_TYPE_SCALE;
    let text_offset_y = TEXT_OFFSET_Y * CARD_TYPE_SCALE;

    let available_width = (width as f64 - margin * 2.0).max(1.0);
    let card_width = available_width
        .min(card_max_width)
        .max(card_min_width.min(available_width));
    let x = (width as f64 - card_width - margin).max(margin);
    let min_y = margin;
    let max_y = (height as f64 - margin).max(min_y);
    let y = (height as f64 * 0.06).clamp(min_y, max_y);
    let content_x = x + card_padding;
    let content_w = (card_width - card_padding * 2.0).max(1.0);

    let eyebrow_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 12.0 * CARD_TYPE_SCALE,
    };
    let title_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: 20.0 * CARD_TYPE_SCALE,
    };
    let body_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 13.0 * CARD_TYPE_SCALE,
    };
    let item_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 13.0 * CARD_TYPE_SCALE,
    };
    let footer_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 11.0 * CARD_TYPE_SCALE,
    };

    let body_height = text_layout(ctx, body_style, &card.body, Some(content_w))
        .ink_extents()
        .height()
        .max(body_style.size);
    let content_height =
        (EYEBROW_CONTENT_HEIGHT + TITLE_CONTENT_HEIGHT + BODY_BOTTOM_GAP + FOOTER_CONTENT_HEIGHT)
            * CARD_TYPE_SCALE
            + body_height
            + card.items.len() as f64 * item_gap_y;
    let card_height = content_height + card_padding * 2.0;

    draw_rounded_rect(ctx, x, y, card_width, card_height, card_radius);
    theme::set_color(ctx, CARD_BG);
    let _ = ctx.fill_preserve();
    theme::set_color(ctx, CARD_BORDER);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    let mut cursor_y = y + card_padding;

    theme::set_color(ctx, TEXT_EYEBROW);
    draw_text_baseline(
        ctx,
        eyebrow_style,
        &fit_text(ctx, &card.eyebrow, eyebrow_style, content_w),
        content_x,
        cursor_y + 12.0 * CARD_TYPE_SCALE,
        None,
    );
    cursor_y += EYEBROW_CONTENT_HEIGHT * CARD_TYPE_SCALE;

    theme::set_color(ctx, overlay::TEXT_ACTIVE);
    draw_text_baseline(
        ctx,
        title_style,
        &fit_text(ctx, &card.title, title_style, content_w),
        content_x,
        cursor_y + 20.0 * CARD_TYPE_SCALE,
        None,
    );
    cursor_y += TITLE_CONTENT_HEIGHT * CARD_TYPE_SCALE;

    theme::set_color(ctx, TEXT_BODY);
    draw_text_baseline(
        ctx,
        body_style,
        &card.body,
        content_x,
        cursor_y + 13.0 * CARD_TYPE_SCALE,
        Some(content_w),
    );
    cursor_y += body_height + BODY_BOTTOM_GAP * CARD_TYPE_SCALE;

    for item in &card.items {
        let dot_x = content_x + item_dot_size * 0.5;
        let dot_y = cursor_y + item_dot_size * 0.5 + 1.0 * CARD_TYPE_SCALE;
        ctx.arc(
            dot_x,
            dot_y,
            item_dot_size * 0.5,
            0.0,
            std::f64::consts::TAU,
        );
        if item.done {
            theme::set_color(ctx, DOT_DONE);
        } else {
            theme::set_color(ctx, DOT_PENDING);
        }
        let _ = ctx.fill();

        if item.done {
            theme::set_color(ctx, CHECKMARK);
            draw_checkmark(ctx, dot_x, dot_y, item_dot_size * 0.5);
        }

        theme::set_color(ctx, overlay::TEXT_SECONDARY);
        let item_x = content_x + item_dot_size + 8.0 * CARD_TYPE_SCALE;
        let item_w = content_w - item_dot_size - 8.0 * CARD_TYPE_SCALE;
        draw_text_baseline(
            ctx,
            item_style,
            &fit_text(ctx, &item.label, item_style, item_w),
            item_x,
            cursor_y + text_offset_y + item_style.size,
            None,
        );
        cursor_y += item_gap_y;
    }

    theme::set_color(ctx, TEXT_FOOTER);
    draw_text_baseline(
        ctx,
        footer_style,
        &fit_text(ctx, &card.footer, footer_style, content_w),
        content_x,
        y + card_height - card_padding + 2.0 * CARD_TYPE_SCALE,
        None,
    );
}

fn fit_text(ctx: &cairo::Context, text: &str, style: UiTextStyle<'_>, max_width: f64) -> String {
    if text.is_empty() || max_width <= 0.0 {
        return String::new();
    }
    let text_width = |s: &str| text_layout(ctx, style, s, None).ink_extents().width();
    if text_width(text) <= max_width {
        return text.to_string();
    }

    let mut current = text.to_string();
    while !current.is_empty() {
        current.pop();
        let candidate = format!("{current}{ELLIPSIS}");
        if text_width(&candidate) <= max_width {
            return candidate;
        }
    }
    ELLIPSIS.to_string()
}

fn draw_checkmark(ctx: &cairo::Context, cx: f64, cy: f64, radius: f64) {
    let left_x = cx - radius * 0.55;
    let left_y = cy + radius * 0.05;
    let mid_x = cx - radius * 0.10;
    let mid_y = cy + radius * 0.45;
    let right_x = cx + radius * 0.62;
    let right_y = cy - radius * 0.42;

    ctx.new_path();
    ctx.move_to(left_x, left_y);
    ctx.line_to(mid_x, mid_y);
    ctx.line_to(right_x, right_y);
    ctx.set_line_width((radius * 0.48).max(1.0));
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);
    let _ = ctx.stroke();
}
