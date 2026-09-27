//! The onboarding card's action buttons: layout, hit rectangles, and paint.

use super::{OnboardingCardAction, OnboardingCardButton, OnboardingCardButtonHit, fit_text};
use crate::ui::primitives::draw_rounded_rect;
use crate::ui::theme::{self, Rgba, overlay};
use crate::ui_text::{UiTextEngine, UiTextStyle};

// Metrics at 1x; the card multiplies them by its type scale.
const BUTTON_HEIGHT: f64 = 28.0;
const BUTTON_PADDING_X: f64 = 12.0;
const BUTTON_GAP: f64 = 8.0;
const BUTTON_RADIUS: f64 = 6.0;
const LABEL_SIZE: f64 = 12.5;
const KEY_HINT_SIZE: f64 = 11.0;
const KEY_HINT_GAP: f64 = 7.0;
/// Space between the checklist (or body) and the first button row.
pub(super) const BUTTON_TOP_GAP: f64 = 4.0;

/// Secondary button fill and border: quiet so the primary action leads.
const SECONDARY_BG: Rgba = (1.0, 1.0, 1.0, 0.06);
const SECONDARY_BG_HOVER: Rgba = (1.0, 1.0, 1.0, 0.14);
const SECONDARY_BORDER: Rgba = (0.36, 0.46, 0.58, 0.8);
/// Key hint on the accent fill.
const PRIMARY_KEY_HINT: Rgba = (1.0, 1.0, 1.0, 0.78);

fn label_style(scale: f64) -> UiTextStyle<'static> {
    UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: LABEL_SIZE * scale,
    }
}

fn key_hint_style(scale: f64) -> UiTextStyle<'static> {
    UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: KEY_HINT_SIZE * scale,
    }
}

/// Places the buttons left to right from `(left, top)`, wrapping onto a new
/// row when the next one would cross `max_width`. Returns their rectangles
/// and the height of the block.
pub(super) fn layout_buttons(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    buttons: &[OnboardingCardButton],
    (left, top): (f64, f64),
    max_width: f64,
    scale: f64,
) -> (Vec<OnboardingCardButtonHit>, f64) {
    if buttons.is_empty() {
        return (Vec::new(), 0.0);
    }
    let height = BUTTON_HEIGHT * scale;
    let gap = BUTTON_GAP * scale;
    let text_width =
        |style, text: &str| engine.layout(ctx, style, text, None).ink_extents().width();

    let mut hits = Vec::with_capacity(buttons.len());
    let (mut x, mut y) = (left, top);
    for button in buttons {
        let mut width =
            BUTTON_PADDING_X * 2.0 * scale + text_width(label_style(scale), &button.label);
        if let Some(hint) = &button.key_hint {
            width += KEY_HINT_GAP * scale + text_width(key_hint_style(scale), hint);
        }
        let width = width.min(max_width);
        if x > left && x + width > left + max_width {
            x = left;
            y += height + gap;
        }
        hits.push(OnboardingCardButtonHit {
            x,
            y,
            width,
            height,
            action: button.action,
        });
        x += width + gap;
    }

    (hits, y + height - top)
}

pub(super) fn paint_buttons(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    buttons: &[OnboardingCardButton],
    hits: &[OnboardingCardButtonHit],
    hovered: Option<OnboardingCardAction>,
    scale: f64,
) {
    let label_style = label_style(scale);
    let hint_style = key_hint_style(scale);
    let padding = BUTTON_PADDING_X * scale;

    for (button, hit) in buttons.iter().zip(hits) {
        let hover = hovered == Some(button.action);
        draw_rounded_rect(
            ctx,
            hit.x,
            hit.y,
            hit.width,
            hit.height,
            BUTTON_RADIUS * scale,
        );
        if button.primary {
            let fill = if hover {
                overlay::ACCENT_BRIGHT
            } else {
                overlay::ACCENT_PRIMARY
            };
            theme::set_color(ctx, fill);
            let _ = ctx.fill();
        } else {
            let fill = if hover {
                SECONDARY_BG_HOVER
            } else {
                SECONDARY_BG
            };
            theme::set_color(ctx, fill);
            let _ = ctx.fill_preserve();
            theme::set_color(ctx, SECONDARY_BORDER);
            ctx.set_line_width(1.0);
            let _ = ctx.stroke();
        }

        let baseline = hit.y + hit.height * 0.5 + label_style.size * 0.36;
        let inner_width = (hit.width - padding * 2.0).max(0.0);
        let label = fit_text(engine, ctx, &button.label, label_style, inner_width);
        let label_color = if button.primary {
            overlay::TEXT_WHITE
        } else {
            overlay::TEXT_SECONDARY
        };
        theme::set_color(ctx, label_color);
        let label_extents =
            engine.draw_baseline(ctx, label_style, &label, hit.x + padding, baseline, None);

        let Some(hint) = &button.key_hint else {
            continue;
        };
        let hint_x = hit.x + padding + label_extents.width() + KEY_HINT_GAP * scale;
        let hint_room = hit.x + hit.width - padding - hint_x;
        if hint_room <= 0.0 {
            continue;
        }
        let hint_color = if button.primary {
            PRIMARY_KEY_HINT
        } else {
            overlay::TEXT_HINT
        };
        theme::set_color(ctx, hint_color);
        engine.draw_baseline(
            ctx,
            hint_style,
            &fit_text(engine, ctx, hint, hint_style, hint_room),
            hint_x,
            baseline,
            None,
        );
    }
}
