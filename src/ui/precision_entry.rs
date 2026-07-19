//! Precise numeric entry popup rendering.
//!
//! A small overlay popup anchored under the top strip's style pill: a
//! title, an input field showing the typed buffer with its unit, and a
//! commit/cancel hint. Keyboard capture mirrors the color popup's hex
//! field (see `InputState::handle_precision_entry_key`); the popup itself
//! is keyboard-only, so it renders no buttons.

use crate::input::InputState;
use crate::ui::primitives::{draw_rounded_rect, text_extents_for};
use crate::ui_text::{UiTextStyle, draw_text_baseline};

use super::theme::overlay::{
    BG_INPUT_SELECTION, BORDER_MODAL, INPUT_BG, INPUT_BORDER_FOCUSED, INPUT_CARET, PANEL_BG_MODAL,
    RADIUS_MD, RADIUS_STD, TEXT_HINT_DIM, TEXT_PRIMARY,
};

const POPUP_W: f64 = 180.0;
const POPUP_H: f64 = 96.0;
const PAD: f64 = 12.0;
const FIELD_H: f64 = 30.0;

const VALUE_STYLE: UiTextStyle<'static> = UiTextStyle {
    family: "Monospace",
    slant: cairo::FontSlant::Normal,
    weight: cairo::FontWeight::Normal,
    size: 14.0,
};

fn set_rgba(ctx: &cairo::Context, color: super::theme::Rgba) {
    ctx.set_source_rgba(color.0, color.1, color.2, color.3);
}

/// Render the precise-entry popup near `anchor` (the top-left point under
/// the strip's style pill), clamped to the screen.
pub fn render_precision_entry_popup(
    ctx: &cairo::Context,
    input_state: &InputState,
    screen_width: u32,
    screen_height: u32,
    anchor: (f64, f64),
) {
    let Some(entry) = input_state.precision_entry() else {
        return;
    };

    let x = anchor
        .0
        .clamp(8.0, (screen_width as f64 - POPUP_W - 8.0).max(8.0));
    let y = anchor
        .1
        .clamp(8.0, (screen_height as f64 - POPUP_H - 8.0).max(8.0));

    // Panel card.
    draw_rounded_rect(ctx, x, y, POPUP_W, POPUP_H, RADIUS_STD);
    set_rgba(ctx, PANEL_BG_MODAL);
    let _ = ctx.fill_preserve();
    set_rgba(ctx, BORDER_MODAL);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    // Title: the target label.
    let title_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: 13.0,
    };
    set_rgba(ctx, TEXT_PRIMARY);
    let _ = draw_text_baseline(
        ctx,
        title_style,
        entry.target.label(),
        x + PAD,
        y + PAD + 11.0,
        None,
    );

    // Input field (always focused: the popup exists to type into).
    let field_y = y + PAD + 20.0;
    let field_w = POPUP_W - PAD * 2.0;
    draw_rounded_rect(ctx, x + PAD, field_y, field_w, FIELD_H, RADIUS_MD);
    set_rgba(ctx, INPUT_BG);
    let _ = ctx.fill_preserve();
    set_rgba(ctx, INPUT_BORDER_FOCUSED);
    ctx.set_line_width(1.5);
    let _ = ctx.stroke();

    let text = format!("{}{}", entry.buffer, entry.target.unit());
    let text_x = x + PAD + 10.0;
    let baseline = field_y + FIELD_H / 2.0 + 5.0;
    let buffer_advance = |text: &str| {
        text_extents_for(
            ctx,
            VALUE_STYLE.family,
            VALUE_STYLE.slant,
            VALUE_STYLE.weight,
            VALUE_STYLE.size,
            text,
        )
        .x_advance()
    };
    if entry.selected && !entry.buffer.is_empty() {
        // Replace-on-type selection highlight behind the prefilled value.
        let advance = buffer_advance(&entry.buffer);
        set_rgba(ctx, BG_INPUT_SELECTION);
        ctx.rectangle(text_x - 2.0, field_y + 5.0, advance + 4.0, FIELD_H - 10.0);
        let _ = ctx.fill();
    }
    set_rgba(ctx, TEXT_PRIMARY);
    let _ = draw_text_baseline(ctx, VALUE_STYLE, &text, text_x, baseline, None);

    // Caret after the buffer (before the unit suffix).
    if !entry.selected {
        let caret_x = text_x + buffer_advance(&entry.buffer) + 1.0;
        set_rgba(ctx, INPUT_CARET);
        ctx.rectangle(caret_x, field_y + 6.0, 1.5, FIELD_H - 12.0);
        let _ = ctx.fill();
    }

    // Hint line.
    set_rgba(ctx, TEXT_HINT_DIM);
    let _ = draw_text_baseline(
        ctx,
        UiTextStyle {
            family: "Sans",
            slant: cairo::FontSlant::Normal,
            weight: cairo::FontWeight::Normal,
            size: 11.0,
        },
        "Enter apply \u{00b7} Esc cancel",
        x + PAD,
        y + POPUP_H - 10.0,
        None,
    );
}
