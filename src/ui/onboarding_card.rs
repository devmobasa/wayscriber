use crate::ui_text::{UiTextStyle, draw_text_baseline};

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
const CARD_PADDING: f64 = 16.0;
const CARD_RADIUS: f64 = 12.0;
const ITEM_DOT_SIZE: f64 = 10.0;
const ITEM_GAP_Y: f64 = 24.0;
const TEXT_OFFSET_Y: f64 = 5.0;
const BASE_CONTENT_HEIGHT: f64 = 126.0;
const CARD_SCALE: f64 = 1.3;
const ELLIPSIS: &str = "...";

pub fn render_onboarding_card(
    ctx: &cairo::Context,
    width: u32,
    height: u32,
    card: &OnboardingCard,
) {
    if card.items.is_empty() {
        return;
    }

    let margin = CARD_MARGIN * CARD_SCALE;
    let card_max_width = CARD_MAX_WIDTH * CARD_SCALE;
    let card_min_width = CARD_MIN_WIDTH * CARD_SCALE;
    let card_padding = CARD_PADDING * CARD_SCALE;
    let card_radius = CARD_RADIUS * CARD_SCALE;
    let item_dot_size = ITEM_DOT_SIZE * CARD_SCALE;
    let item_gap_y = ITEM_GAP_Y * CARD_SCALE;
    let text_offset_y = TEXT_OFFSET_Y * CARD_SCALE;

    let card_width = (width as f64 - margin * 2.0).clamp(card_min_width, card_max_width);
    let x = (width as f64 - card_width - margin).max(margin);
    let min_y = margin;
    let max_y = (height as f64 - margin).max(min_y);
    let y = (height as f64 * 0.06).clamp(min_y, max_y);
    let content_height = BASE_CONTENT_HEIGHT * CARD_SCALE + card.items.len() as f64 * item_gap_y;
    let card_height = content_height + card_padding * 2.0;

    rounded_rect(ctx, x, y, card_width, card_height, card_radius);
    ctx.set_source_rgba(0.07, 0.09, 0.12, 0.94);
    let _ = ctx.fill_preserve();
    ctx.set_source_rgba(0.36, 0.46, 0.58, 0.8);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    let mut cursor_y = y + card_padding;
    let content_x = x + card_padding;
    let content_w = card_width - card_padding * 2.0;

    let eyebrow_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 12.0 * CARD_SCALE,
    };
    let title_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: 20.0 * CARD_SCALE,
    };
    let body_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 13.0 * CARD_SCALE,
    };
    let item_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 13.0 * CARD_SCALE,
    };
    let footer_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 11.0 * CARD_SCALE,
    };

    ctx.set_source_rgba(0.65, 0.74, 0.88, 1.0);
    draw_text_baseline(
        ctx,
        eyebrow_style,
        &fit_text(ctx, &card.eyebrow, eyebrow_style, content_w),
        content_x,
        cursor_y + 12.0 * CARD_SCALE,
        None,
    );
    cursor_y += 20.0 * CARD_SCALE;

    ctx.set_source_rgba(0.96, 0.98, 1.0, 1.0);
    draw_text_baseline(
        ctx,
        title_style,
        &fit_text(ctx, &card.title, title_style, content_w),
        content_x,
        cursor_y + 20.0 * CARD_SCALE,
        None,
    );
    cursor_y += 30.0 * CARD_SCALE;

    ctx.set_source_rgba(0.78, 0.84, 0.92, 1.0);
    draw_text_baseline(
        ctx,
        body_style,
        &fit_text(ctx, &card.body, body_style, content_w),
        content_x,
        cursor_y + 13.0 * CARD_SCALE,
        None,
    );
    cursor_y += 26.0 * CARD_SCALE;

    for item in &card.items {
        let dot_x = content_x + item_dot_size * 0.5;
        let dot_y = cursor_y + item_dot_size * 0.5 + 1.0 * CARD_SCALE;
        ctx.arc(
            dot_x,
            dot_y,
            item_dot_size * 0.5,
            0.0,
            std::f64::consts::TAU,
        );
        if item.done {
            ctx.set_source_rgba(0.30, 0.82, 0.52, 1.0);
        } else {
            ctx.set_source_rgba(0.44, 0.52, 0.62, 1.0);
        }
        let _ = ctx.fill();

        if item.done {
            ctx.set_source_rgba(0.96, 1.0, 0.97, 1.0);
            draw_text_baseline(
                ctx,
                item_style,
                "x",
                content_x + 1.8 * CARD_SCALE,
                cursor_y + 10.0 * CARD_SCALE,
                None,
            );
        }

        ctx.set_source_rgba(0.86, 0.90, 0.96, 1.0);
        let item_x = content_x + item_dot_size + 8.0 * CARD_SCALE;
        let item_w = content_w - item_dot_size - 8.0 * CARD_SCALE;
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

    ctx.set_source_rgba(0.60, 0.68, 0.78, 1.0);
    draw_text_baseline(
        ctx,
        footer_style,
        &fit_text(ctx, &card.footer, footer_style, content_w),
        content_x,
        y + card_height - card_padding + 2.0 * CARD_SCALE,
        None,
    );
}

fn fit_text(ctx: &cairo::Context, text: &str, style: UiTextStyle<'_>, max_width: f64) -> String {
    if text.is_empty() || max_width <= 0.0 {
        return String::new();
    }
    ctx.select_font_face(style.family, style.slant, style.weight);
    ctx.set_font_size(style.size);
    let Ok(extents) = ctx.text_extents(text) else {
        return text.to_string();
    };
    if extents.width() <= max_width {
        return text.to_string();
    }

    let mut current = text.to_string();
    while !current.is_empty() {
        current.pop();
        let candidate = format!("{current}{ELLIPSIS}");
        let Ok(candidate_extents) = ctx.text_extents(&candidate) else {
            break;
        };
        if candidate_extents.width() <= max_width {
            return candidate;
        }
    }
    ELLIPSIS.to_string()
}

fn rounded_rect(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0);
    ctx.new_sub_path();
    ctx.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    ctx.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    ctx.arc(
        x + r,
        y + h - r,
        r,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    ctx.arc(
        x + r,
        y + r,
        r,
        std::f64::consts::PI,
        3.0 * std::f64::consts::FRAC_PI_2,
    );
    ctx.close_path();
}
