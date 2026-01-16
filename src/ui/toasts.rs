use crate::input::InputState;
use crate::input::state::{PRESET_TOAST_DURATION_MS, PresetFeedbackKind, UiToastKind};
use std::time::Instant;

use super::primitives::draw_rounded_rect;
use crate::ui_text::{UiTextStyle, text_layout};

/// Border width for blocked action feedback edge flash.
const BLOCKED_FEEDBACK_BORDER: f64 = 6.0;

/// Vertical position for UI toasts (percentage of screen height from top)
const UI_TOAST_Y_RATIO: f64 = 0.12;
/// Portion of toast lifetime to keep fully opaque before fading
const UI_TOAST_HOLD_RATIO: f64 = 0.75;
/// Vertical position for preset toast (percentage of screen height from top)
const PRESET_TOAST_Y_RATIO: f64 = 0.2;

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

    let now = Instant::now();
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

    let Some((slot, kind, _started, progress)) = latest else {
        return;
    };

    let label = match kind {
        PresetFeedbackKind::Apply => format!("Preset {} applied", slot),
        PresetFeedbackKind::Save => format!("Preset {} saved", slot),
        PresetFeedbackKind::Clear => format!("Preset {} cleared", slot),
    };

    let font_size = 16.0;
    let padding_x = 16.0;
    let padding_y = 9.0;
    let radius = 10.0;

    let text_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: font_size,
    };
    let layout = text_layout(ctx, text_style, &label, None);
    let extents = layout.ink_extents();
    let width = extents.width() + padding_x * 2.0;
    let height = extents.height() + padding_y * 2.0;
    let x = (screen_width as f64 - width) / 2.0;
    let center_y = screen_height as f64 * PRESET_TOAST_Y_RATIO;
    let y = center_y - height / 2.0;

    let fade = if (progress as f64) <= UI_TOAST_HOLD_RATIO {
        1.0
    } else {
        let t = ((progress as f64) - UI_TOAST_HOLD_RATIO) / (1.0 - UI_TOAST_HOLD_RATIO);
        (1.0 - t).clamp(0.0, 1.0)
    };
    let (r, g, b) = match kind {
        PresetFeedbackKind::Apply => (0.22, 0.5, 0.9),
        PresetFeedbackKind::Save => (0.2, 0.7, 0.4),
        PresetFeedbackKind::Clear => (0.88, 0.3, 0.3),
    };

    ctx.set_source_rgba(r, g, b, 0.85 * fade);
    draw_rounded_rect(ctx, x, y, width, height, radius);
    let _ = ctx.fill();

    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95 * fade);
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
    let font_size = 15.0;
    let padding_x = 16.0;
    let padding_y = 9.0;
    let radius = 10.0;

    // Calculate label with optional action suffix
    let action_suffix = toast
        .action
        .as_ref()
        .map(|a| format!(" [{}]", a.label))
        .unwrap_or_default();
    let full_label = format!("{}{}", label, action_suffix);

    let text_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: font_size,
    };
    let full_layout = text_layout(ctx, text_style, &full_label, None);
    let full_extents = full_layout.ink_extents();
    let width = full_extents.width() + padding_x * 2.0;
    let height = full_extents.height() + padding_y * 2.0;
    let x = (screen_width as f64 - width) / 2.0;
    let center_y = screen_height as f64 * UI_TOAST_Y_RATIO;
    let y = center_y - height / 2.0;

    let fade = (1.0 - progress as f64).clamp(0.0, 1.0);
    let (r, g, b) = match toast.kind {
        UiToastKind::Info => (0.25, 0.7, 0.9),
        UiToastKind::Warning => (0.92, 0.62, 0.18),
        UiToastKind::Error => (0.9, 0.3, 0.3),
    };

    ctx.set_source_rgba(r, g, b, 0.92 * fade);
    draw_rounded_rect(ctx, x, y, width, height, radius);
    let _ = ctx.fill();

    // Draw main label
    let label_layout = text_layout(ctx, text_style, label, None);
    let label_extents = label_layout.ink_extents();
    let text_x = x + (width - full_extents.width()) / 2.0 - full_extents.x_bearing();
    let text_y = y + (height - full_extents.height()) / 2.0 - full_extents.y_bearing();

    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.55 * fade);
    label_layout.show_at_baseline(ctx, text_x + 1.0, text_y + 1.0);
    ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0 * fade);
    label_layout.show_at_baseline(ctx, text_x, text_y);

    // Draw action suffix in slightly dimmer color if present
    if toast.action.is_some() {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.7 * fade);
        let suffix_layout = text_layout(ctx, text_style, &action_suffix, None);
        suffix_layout.show_at_baseline(
            ctx,
            text_x + label_extents.width() + label_extents.x_bearing(),
            text_y,
        );
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

    // Quick fade in, then fade out
    let alpha = if progress < 0.2 {
        // Fade in during first 20%
        (progress / 0.2) * 0.18
    } else {
        // Fade out during remaining 80%
        0.18 * (1.0 - (progress - 0.2) / 0.8)
    };

    let w = screen_width as f64;
    let h = screen_height as f64;
    let b = BLOCKED_FEEDBACK_BORDER;

    // Red tint on all four screen edges
    ctx.set_source_rgba(0.9, 0.2, 0.2, alpha);

    // Top edge
    ctx.rectangle(0.0, 0.0, w, b);
    let _ = ctx.fill();

    // Bottom edge
    ctx.rectangle(0.0, h - b, w, b);
    let _ = ctx.fill();

    // Left edge (between top and bottom)
    ctx.rectangle(0.0, b, b, h - 2.0 * b);
    let _ = ctx.fill();

    // Right edge (between top and bottom)
    ctx.rectangle(w - b, b, b, h - 2.0 * b);
    let _ = ctx.fill();
}
