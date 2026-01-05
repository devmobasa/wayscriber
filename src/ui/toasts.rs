use crate::input::InputState;
use crate::input::state::{PRESET_TOAST_DURATION_MS, PresetFeedbackKind, UiToastKind};
use std::time::Instant;

use super::primitives::{draw_rounded_rect, text_extents_for};

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

    let extents = text_extents_for(
        ctx,
        "Sans",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
        font_size,
        &label,
    );
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
    ctx.move_to(text_x, text_y);
    let _ = ctx.show_text(&label);
}

/// Render a transient UI toast (warnings/errors/info).
pub fn render_ui_toast(
    ctx: &cairo::Context,
    input_state: &InputState,
    screen_width: u32,
    screen_height: u32,
) {
    let Some(toast) = input_state.ui_toast.as_ref() else {
        return;
    };

    let now = Instant::now();
    let duration_secs = toast.duration_ms as f32 / 1000.0;
    let elapsed = now.saturating_duration_since(toast.started);
    let progress = (elapsed.as_secs_f32() / duration_secs).clamp(0.0, 1.0);
    if progress >= 1.0 {
        return;
    }

    let label = toast.message.as_str();
    let font_size = 15.0;
    let padding_x = 16.0;
    let padding_y = 9.0;
    let radius = 10.0;

    let extents = text_extents_for(
        ctx,
        "Sans",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
        font_size,
        label,
    );
    let width = extents.width() + padding_x * 2.0;
    let height = extents.height() + padding_y * 2.0;
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

    let text_x = x + (width - extents.width()) / 2.0 - extents.x_bearing();
    let text_y = y + (height - extents.height()) / 2.0 - extents.y_bearing();
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.55 * fade);
    ctx.move_to(text_x + 1.0, text_y + 1.0);
    let _ = ctx.show_text(label);
    ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0 * fade);
    ctx.move_to(text_x, text_y);
    let _ = ctx.show_text(label);
}
