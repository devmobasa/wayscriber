//! Palette shortcut-capture modal and action tooltips.

use super::*;

pub(super) fn command_palette_action_tooltip_geometry(
    engine: &UiTextEngine,
    text: &str,
    pointer_x: f64,
    pointer_y: f64,
    screen_width: f64,
    screen_height: f64,
) -> Option<(f64, f64, f64, f64)> {
    let style = command_palette_text_style(
        COMMAND_PALETTE_SHORTCUT_TEXT_SIZE,
        cairo::FontWeight::Normal,
        cairo::FontSlant::Normal,
    );
    let extents = engine.measure(style, text, None)?;
    let width = extents.width() + TOOLTIP_PADDING_X * 2.0;
    let height = style.size + TOOLTIP_PADDING_Y * 2.0;
    let x = (pointer_x + TOOLTIP_POINTER_OFFSET)
        .min((screen_width - width - FRAME_SHADOW_OFFSET).max(FRAME_SHADOW_OFFSET));
    let y = (pointer_y + TOOLTIP_POINTER_OFFSET)
        .min((screen_height - height - FRAME_SHADOW_OFFSET).max(FRAME_SHADOW_OFFSET));
    Some((x, y, width, height))
}

pub(super) fn draw_command_palette_action_tooltip(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    text: &str,
    pointer_x: f64,
    pointer_y: f64,
    screen_width: f64,
    screen_height: f64,
) {
    let style = command_palette_text_style(
        COMMAND_PALETTE_SHORTCUT_TEXT_SIZE,
        cairo::FontWeight::Normal,
        cairo::FontSlant::Normal,
    );
    let Some((x, y, width, height)) = command_palette_action_tooltip_geometry(
        engine,
        text,
        pointer_x,
        pointer_y,
        screen_width,
        screen_height,
    ) else {
        return;
    };

    constants::set_color(ctx, TOOLTIP_BG);
    draw_rounded_rect(ctx, x, y, width, height, 5.0);
    let _ = ctx.fill();
    constants::set_color(ctx, TEXT_WHITE);
    engine.draw_baseline(
        ctx,
        style,
        text,
        x + TOOLTIP_PADDING_X,
        y + TOOLTIP_PADDING_Y + style.size,
        None,
    );
}

/// Frame of the shortcut-capture modal, shared by rendering and damage.
pub(super) fn keybinding_capture_geometry(
    screen_width: u32,
    screen_height: u32,
) -> (f64, f64, f64, f64) {
    let width = 520.0_f64.min(screen_width as f64 - 24.0);
    let height = 170.0;
    let x = (screen_width as f64 - width) / 2.0;
    let y = screen_height as f64 * COMMAND_PALETTE_TOP_RATIO;
    (x, y, width, height)
}

pub(super) fn render_keybinding_capture(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    current: &[String],
    action: crate::config::Action,
    screen_width: u32,
    screen_height: u32,
) {
    let (x, y, width, height) = keybinding_capture_geometry(screen_width, screen_height);
    draw_command_palette_frame(
        ctx,
        screen_width as f64,
        screen_height as f64,
        x,
        y,
        width,
        height,
    );

    let title_style =
        command_palette_text_style(18.0, cairo::FontWeight::Bold, cairo::FontSlant::Normal);
    let body_style =
        command_palette_text_style(13.0, cairo::FontWeight::Normal, cairo::FontSlant::Normal);
    constants::set_color(ctx, TEXT_WHITE);
    engine.draw_baseline(
        ctx,
        title_style,
        &format!("Rebind {}", action_label(action)),
        x + 22.0,
        y + 38.0,
        None,
    );
    constants::set_color(ctx, TEXT_DESCRIPTION);
    engine.draw_baseline(
        ctx,
        body_style,
        &format!(
            "Current: {}",
            if current.is_empty() {
                "Not bound".to_string()
            } else {
                current.join(", ")
            }
        ),
        x + 22.0,
        y + 70.0,
        None,
    );
    constants::set_color(ctx, TEXT_WHITE);
    engine.draw_baseline(
        ctx,
        body_style,
        "Press the new shortcut now",
        x + 22.0,
        y + 108.0,
        None,
    );
    constants::set_color(ctx, TEXT_DESCRIPTION);
    engine.draw_baseline(
        ctx,
        body_style,
        KEYBINDING_CAPTURE_SCOPE_NOTE,
        x + 22.0,
        y + 140.0,
        None,
    );
}

/// Says what a captured chord costs and what refuses it, at the interaction
/// point. The edit is durable, so the line names the two things that are not
/// obvious: backing out, and what happens to a chord that is already taken.
const KEYBINDING_CAPTURE_SCOPE_NOTE: &str =
    "Escape cancels • a shortcut already in use is rejected";
