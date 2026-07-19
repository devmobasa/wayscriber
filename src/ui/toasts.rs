use crate::input::InputState;
use crate::input::state::{PRESET_TOAST_DURATION_MS, PresetFeedbackKind, UiToastKind};
use std::time::Instant;

use super::anim;
use super::constants::{
    self, BLOCKED_FLASH, PROGRESS_TRACK, RADIUS_LG, RADIUS_SM, SPACING_LG, TEXT_WHITE, TOAST_ERROR,
    TOAST_INFO, TOAST_SUCCESS, TOAST_WARNING,
};
use super::primitives::draw_rounded_rect;
use crate::ui_text::{UiTextStyle, measure_text, text_layout};

/// Border width for blocked action feedback edge flash.
const BLOCKED_FEEDBACK_BORDER: f64 = 6.0;

/// Vertical position for UI toasts (percentage of screen height from top)
const UI_TOAST_Y_RATIO: f64 = 0.12;
/// Fixed final fade keeps long-lived UI toasts crisp for nearly their full lifetime.
const UI_TOAST_FADE_SECONDS: f64 = 0.2;
/// Portion of preset-toast lifetime to keep fully opaque before fading.
const PRESET_TOAST_HOLD_RATIO: f64 = 0.75;
/// Vertical position for preset toast (percentage of screen height from top)
const PRESET_TOAST_Y_RATIO: f64 = 0.2;

const UI_TOAST_FONT_SIZE: f64 = 15.0;
const PRESET_TOAST_FONT_SIZE: f64 = 16.0;
const TOAST_PADDING_X: f64 = SPACING_LG;
const TOAST_PADDING_Y: f64 = 9.0;
const TOAST_WARNING_TEXT: (f64, f64, f64) = (0.07, 0.09, 0.15);

fn toast_text_style(size: f64) -> UiTextStyle<'static> {
    UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size,
    }
}

fn preset_toast_fade(progress: f64) -> f64 {
    anim::hold_then_fade_out(progress, PRESET_TOAST_HOLD_RATIO)
}

fn ui_toast_fade(elapsed_secs: f64, duration_secs: f64) -> f64 {
    anim::end_fade(elapsed_secs, duration_secs, UI_TOAST_FADE_SECONDS)
}

/// Box geometry for a toast label centered horizontally at a screen-height ratio.
fn toast_box_geometry(
    label: &str,
    font_size: f64,
    screen_width: u32,
    screen_height: u32,
    y_ratio: f64,
) -> Option<(f64, f64, f64, f64)> {
    let extents = measure_text(toast_text_style(font_size), label, None)?;
    let width = extents.width() + TOAST_PADDING_X * 2.0;
    let height = extents.height() + TOAST_PADDING_Y * 2.0;
    let x = (screen_width as f64 - width) / 2.0;
    let center_y = screen_height as f64 * y_ratio;
    let y = center_y - height / 2.0;
    Some((x, y, width, height))
}

/// The most recent, still-animating preset feedback entry: (slot, kind, progress).
fn latest_preset_feedback(
    input_state: &InputState,
    now: Instant,
) -> Option<(usize, PresetFeedbackKind, f32)> {
    let duration_secs = PRESET_TOAST_DURATION_MS as f32 / 1000.0;
    let mut latest: Option<(usize, PresetFeedbackKind, Instant, f32)> = None;

    for (index, entry) in input_state.preset_feedback.iter().enumerate() {
        let Some(feedback) = entry.as_ref() else {
            continue;
        };
        let elapsed = now.saturating_duration_since(feedback.started);
        let progress = (elapsed.as_secs_f32() / duration_secs).clamp(0.0, 1.0);
        if progress >= 1.0 {
            continue;
        }
        match latest {
            Some((_, _, prev_started, _)) if prev_started >= feedback.started => {}
            _ => {
                latest = Some((index + 1, feedback.kind, feedback.started, progress));
            }
        }
    }

    latest.map(|(slot, kind, _started, progress)| (slot, kind, progress))
}

fn preset_feedback_label(slot: usize, kind: PresetFeedbackKind) -> String {
    match kind {
        PresetFeedbackKind::Apply => format!("Preset {} applied", slot),
        PresetFeedbackKind::Save => format!("Preset {} saved", slot),
        PresetFeedbackKind::Clear => format!("Preset {} cleared", slot),
    }
}

fn ui_toast_full_label(input_state: &InputState) -> Option<String> {
    let toast = input_state.ui_toast.as_ref()?;
    let action_suffix = toast
        .action
        .as_ref()
        .map(|a| format!(" [{}]", a.label))
        .unwrap_or_default();
    Some(format!("{}{}", toast.message, action_suffix))
}

/// On-screen bounds (x, y, width, height) the active UI toast occupies, without
/// rendering it. Used for damage tracking; measurement goes through the same
/// layout cache as rendering, so the two always agree.
pub fn ui_toast_geometry(
    input_state: &InputState,
    screen_width: u32,
    screen_height: u32,
) -> Option<(f64, f64, f64, f64)> {
    let full_label = ui_toast_full_label(input_state)?;
    toast_box_geometry(
        &full_label,
        UI_TOAST_FONT_SIZE,
        screen_width,
        screen_height,
        UI_TOAST_Y_RATIO,
    )
}

/// On-screen bounds (x, y, width, height) of the active preset toast, without
/// rendering it. Returns `None` when no preset toast would be drawn.
pub fn preset_toast_geometry(
    input_state: &InputState,
    screen_width: u32,
    screen_height: u32,
) -> Option<(f64, f64, f64, f64)> {
    if !input_state.show_preset_toasts {
        return None;
    }
    let (slot, kind, _progress) = latest_preset_feedback(input_state, Instant::now())?;
    let label = preset_feedback_label(slot, kind);
    toast_box_geometry(
        &label,
        PRESET_TOAST_FONT_SIZE,
        screen_width,
        screen_height,
        PRESET_TOAST_Y_RATIO,
    )
}

/// The four screen-edge strips flashed by blocked-action feedback.
pub fn blocked_feedback_rects(screen_width: u32, screen_height: u32) -> [(f64, f64, f64, f64); 4] {
    let w = screen_width as f64;
    let h = screen_height as f64;
    let b = BLOCKED_FEEDBACK_BORDER;
    [
        (0.0, 0.0, w, b),
        (0.0, h - b, w, b),
        (0.0, b, b, h - 2.0 * b),
        (w - b, b, b, h - 2.0 * b),
    ]
}

/// Render a transient toast for preset actions (apply/save/clear).
pub fn render_preset_toast(
    ctx: &cairo::Context,
    input_state: &InputState,
    screen_width: u32,
    screen_height: u32,
) {
    if !input_state.show_preset_toasts {
        return;
    }

    let Some((slot, kind, progress)) = latest_preset_feedback(input_state, Instant::now()) else {
        return;
    };

    let label = preset_feedback_label(slot, kind);
    let radius = RADIUS_LG;

    let text_style = toast_text_style(PRESET_TOAST_FONT_SIZE);
    let layout = text_layout(ctx, text_style, &label, None);
    let extents = layout.ink_extents();
    let Some((x, y, width, height)) = toast_box_geometry(
        &label,
        PRESET_TOAST_FONT_SIZE,
        screen_width,
        screen_height,
        PRESET_TOAST_Y_RATIO,
    ) else {
        return;
    };

    let fade = preset_toast_fade(progress as f64);
    let (r, g, b) = match kind {
        PresetFeedbackKind::Apply => TOAST_INFO,
        PresetFeedbackKind::Save => TOAST_SUCCESS,
        PresetFeedbackKind::Clear => TOAST_ERROR,
    };

    constants::set_color_alpha(ctx, (r, g, b), 0.85 * fade);
    draw_rounded_rect(ctx, x, y, width, height, radius);
    let _ = ctx.fill();

    ctx.set_source_rgba(TEXT_WHITE.0, TEXT_WHITE.1, TEXT_WHITE.2, 0.95 * fade);
    let text_x = x + (width - extents.width()) / 2.0 - extents.x_bearing();
    let text_y = y + (height - extents.height()) / 2.0 - extents.y_bearing();
    layout.show_at_baseline(ctx, text_x, text_y);
}

/// Render a transient UI toast (warnings/errors/info).
/// Returns the toast bounds (x, y, width, height) if rendered, for click detection.
pub fn render_ui_toast(
    ctx: &cairo::Context,
    input_state: &InputState,
    screen_width: u32,
    screen_height: u32,
) -> Option<(f64, f64, f64, f64)> {
    let toast = input_state.ui_toast.as_ref()?;

    let now = Instant::now();
    let duration_secs = toast.duration_ms as f32 / 1000.0;
    let elapsed = now.saturating_duration_since(toast.started);
    let progress = (elapsed.as_secs_f32() / duration_secs).clamp(0.0, 1.0);
    if progress >= 1.0 {
        return None;
    }

    let label = toast.message.as_str();
    let padding_x = TOAST_PADDING_X;
    let radius = RADIUS_LG;

    // Calculate label with optional action suffix
    let action_suffix = toast
        .action
        .as_ref()
        .map(|a| format!(" [{}]", a.label))
        .unwrap_or_default();
    let full_label = format!("{}{}", label, action_suffix);

    let text_style = toast_text_style(UI_TOAST_FONT_SIZE);
    let full_layout = text_layout(ctx, text_style, &full_label, None);
    let full_extents = full_layout.ink_extents();
    let (x, y, width, height) = toast_box_geometry(
        &full_label,
        UI_TOAST_FONT_SIZE,
        screen_width,
        screen_height,
        UI_TOAST_Y_RATIO,
    )?;

    let fade = ui_toast_fade(elapsed.as_secs_f64(), duration_secs as f64);
    let (r, g, b) = match toast.kind {
        UiToastKind::Info => TOAST_INFO,
        UiToastKind::Warning => TOAST_WARNING,
        UiToastKind::Error => TOAST_ERROR,
    };

    let background_alpha = if toast.kind == UiToastKind::Warning {
        1.0
    } else {
        0.92
    };
    constants::set_color_alpha(ctx, (r, g, b), background_alpha * fade);
    draw_rounded_rect(ctx, x, y, width, height, radius);
    let _ = ctx.fill();

    // Keep the outline on the contrasting side of the semantic fill.
    let outline = if toast.kind == UiToastKind::Warning {
        TOAST_WARNING_TEXT
    } else {
        (TEXT_WHITE.0, TEXT_WHITE.1, TEXT_WHITE.2)
    };
    ctx.set_source_rgba(outline.0, outline.1, outline.2, 0.28 * fade);
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

    // Draw countdown progress bar for confirmation toasts
    if toast.action.is_some() {
        let progress_height = 3.0;
        let progress_y = y + height - progress_height - 2.0;
        let progress_width = width - padding_x * 2.0;
        let remaining_width = progress_width * (1.0 - progress as f64);

        // Track background
        constants::set_color(ctx, PROGRESS_TRACK);
        draw_rounded_rect(
            ctx,
            x + padding_x,
            progress_y,
            progress_width,
            progress_height,
            1.5,
        );
        let _ = ctx.fill();

        // Remaining time indicator (shrinks as time runs out)
        if remaining_width > 0.0 {
            let progress_color = if toast.kind == UiToastKind::Warning {
                TOAST_WARNING_TEXT
            } else {
                (TEXT_WHITE.0, TEXT_WHITE.1, TEXT_WHITE.2)
            };
            ctx.set_source_rgba(
                progress_color.0,
                progress_color.1,
                progress_color.2,
                0.8 * fade,
            );
            draw_rounded_rect(
                ctx,
                x + padding_x,
                progress_y,
                remaining_width,
                progress_height,
                1.5,
            );
            let _ = ctx.fill();
        }
    }

    // Draw main label
    let label_layout = text_layout(ctx, text_style, label, None);
    let label_extents = label_layout.ink_extents();
    let text_x = x + (width - full_extents.width()) / 2.0 - full_extents.x_bearing();
    let text_y = y + (height - full_extents.height()) / 2.0 - full_extents.y_bearing();

    let text_color = if toast.kind == UiToastKind::Warning {
        TOAST_WARNING_TEXT
    } else {
        (TEXT_WHITE.0, TEXT_WHITE.1, TEXT_WHITE.2)
    };
    ctx.set_source_rgba(text_color.0, text_color.1, text_color.2, fade);
    label_layout.show_at_baseline(ctx, text_x, text_y);

    // Draw action suffix with button-style background for better visibility
    if toast.action.is_some() {
        let suffix_layout = text_layout(ctx, text_style, &action_suffix, None);
        let suffix_extents = suffix_layout.ink_extents();
        let suffix_x = text_x + label_extents.width() + label_extents.x_bearing();

        // Button-style background for action
        let btn_padding = 4.0;
        let btn_x = suffix_x - btn_padding + suffix_extents.x_bearing();
        let btn_y = text_y - suffix_extents.height() - btn_padding + suffix_extents.y_bearing();
        let btn_w = suffix_extents.width() + btn_padding * 2.0;
        let btn_h = suffix_extents.height() + btn_padding * 2.0;

        ctx.set_source_rgba(text_color.0, text_color.1, text_color.2, 0.16 * fade);
        draw_rounded_rect(ctx, btn_x, btn_y, btn_w, btn_h, RADIUS_SM);
        let _ = ctx.fill();

        // Action text
        ctx.set_source_rgba(text_color.0, text_color.1, text_color.2, 0.95 * fade);
        suffix_layout.show_at_baseline(ctx, suffix_x, text_y);
    }

    Some((x, y, width, height))
}

/// Render blocked action feedback - a brief red flash on screen edges.
pub fn render_blocked_feedback(
    ctx: &cairo::Context,
    input_state: &InputState,
    screen_width: u32,
    screen_height: u32,
) {
    let Some(progress) = input_state.blocked_feedback_progress() else {
        return;
    };

    // Quick fade in, hold at peak, then fade out
    let alpha = anim::flash(progress, 0.15, 0.4, 0.22);

    // Red tint on all four screen edges
    constants::set_color_alpha(ctx, BLOCKED_FLASH, alpha);
    for (x, y, w, h) in blocked_feedback_rects(screen_width, screen_height) {
        ctx.rectangle(x, y, w, h);
        let _ = ctx.fill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_toast_uses_a_short_fixed_end_fade() {
        assert_eq!(ui_toast_fade(0.0, 5.0), 1.0);
        assert_eq!(ui_toast_fade(4.8, 5.0), 1.0);
        assert!((ui_toast_fade(4.9, 5.0) - 0.5).abs() < 1e-9);
        assert_eq!(ui_toast_fade(5.0, 5.0), 0.0);
    }

    #[test]
    fn preset_toast_keeps_its_proportional_fade() {
        assert_eq!(preset_toast_fade(0.0), 1.0);
        assert_eq!(preset_toast_fade(PRESET_TOAST_HOLD_RATIO), 1.0);
        assert_eq!(preset_toast_fade(0.875), 0.5);
        assert_eq!(preset_toast_fade(1.0), 0.0);
    }
}
