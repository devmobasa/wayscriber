//! Toast text measurement, ellipsis and action geometry.

use super::*;

#[derive(Debug)]
pub(super) struct UiToastLayout {
    pub(super) bounds: ToastBounds,
    pub(super) action_bounds: [Option<ToastBounds>; 2],
    pub(super) message: String,
}

pub(super) fn toast_box_geometry(
    engine: &UiTextEngine,
    label: &str,
    font_size: f64,
    screen_width: u32,
    screen_height: u32,
    y_ratio: f64,
) -> Option<(f64, f64, f64, f64)> {
    let extents = engine.measure(toast_text_style(font_size), label, None)?;
    let width = extents.width() + TOAST_PADDING_X * 2.0;
    let height = extents.height() + TOAST_PADDING_Y * 2.0;
    let x = (screen_width as f64 - width) / 2.0;
    let center_y = screen_height as f64 * y_ratio;
    let y = center_y - height / 2.0;
    Some((x, y, width, height))
}

fn measured_width(engine: &UiTextEngine, text: &str) -> Option<f64> {
    Some(
        engine
            .measure(toast_text_style(UI_TOAST_FONT_SIZE), text, None)?
            .width(),
    )
}

fn ellipsize_to_width(engine: &UiTextEngine, text: &str, max_width: f64) -> Option<String> {
    if measured_width(engine, text)? <= max_width {
        return Some(text.to_string());
    }
    const ELLIPSIS: &str = "…";
    if measured_width(engine, ELLIPSIS)? > max_width {
        return Some(String::new());
    }
    for (end, _) in text.char_indices().rev() {
        let candidate = format!("{}{}", text[..end].trim_end(), ELLIPSIS);
        if measured_width(engine, &candidate)? <= max_width {
            return Some(candidate);
        }
    }
    Some(ELLIPSIS.to_string())
}

pub(super) fn ui_toast_layout(
    engine: &UiTextEngine,
    input_state: &InputState,
    screen_width: u32,
    screen_height: u32,
) -> Option<UiToastLayout> {
    let toast = input_state.active_toast()?;
    let actions = [toast.action.as_ref(), toast.secondary_action.as_ref()];
    let action_sizes = actions.map(|action| {
        action.and_then(|action| {
            let extents =
                engine.measure(toast_text_style(UI_TOAST_FONT_SIZE), &action.label, None)?;
            Some((
                extents.width() + TOAST_ACTION_PADDING_X * 2.0,
                extents.height() + TOAST_ACTION_PADDING_Y * 2.0,
            ))
        })
    });
    let action_count = action_sizes.iter().flatten().count();
    let action_width = action_sizes
        .iter()
        .flatten()
        .map(|size| size.0)
        .sum::<f64>()
        + TOAST_ACTION_GAP * action_count.saturating_sub(1) as f64;
    let message_action_gap = if action_count > 0 {
        TOAST_ACTION_GAP
    } else {
        0.0
    };
    let max_box_width = (screen_width as f64 - TOAST_SCREEN_MARGIN * 2.0).max(1.0);
    let max_message_width =
        (max_box_width - TOAST_PADDING_X * 2.0 - action_width - message_action_gap).max(0.0);
    let message = ellipsize_to_width(engine, &toast.message, max_message_width)?;
    let message_extents = engine.measure(toast_text_style(UI_TOAST_FONT_SIZE), &message, None)?;
    let content_width = message_extents.width() + message_action_gap + action_width;
    let content_height = action_sizes
        .iter()
        .flatten()
        .map(|size| size.1)
        .fold(message_extents.height(), f64::max);
    let width = (content_width + TOAST_PADDING_X * 2.0).min(max_box_width);
    let height = content_height + TOAST_PADDING_Y * 2.0;
    let x = (screen_width as f64 - width) / 2.0;
    let y = screen_height as f64 * UI_TOAST_Y_RATIO - height / 2.0;

    let mut action_x = x + TOAST_PADDING_X + message_extents.width() + message_action_gap;
    let mut action_bounds = [None, None];
    for (index, size) in action_sizes.into_iter().enumerate() {
        let Some((action_width, action_height)) = size else {
            continue;
        };
        action_bounds[index] = Some((
            action_x,
            y + (height - action_height) / 2.0,
            action_width,
            action_height,
        ));
        action_x += action_width + TOAST_ACTION_GAP;
    }

    Some(UiToastLayout {
        bounds: (x, y, width, height),
        action_bounds,
        message,
    })
}
