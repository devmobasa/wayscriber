use super::constants::{COLOR_TEXT_PRIMARY, set_color};
use crate::ui_text::{UiTextEngine, UiTextStyle};

#[allow(clippy::too_many_arguments)]
pub(in crate::backend::wayland::toolbar::render) fn draw_label_center(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    style: UiTextStyle<'_>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    text: &str,
) {
    let layout = engine.layout(ctx, style, text, None);
    let ext = layout.ink_extents();
    let tx = x + (w - ext.width()) / 2.0 - ext.x_bearing();
    let ty = y + (h - ext.height()) / 2.0 - ext.y_bearing();
    set_color(ctx, COLOR_TEXT_PRIMARY);
    layout.show_at_baseline(ctx, tx, ty);
}

#[allow(clippy::too_many_arguments)]
pub(in crate::backend::wayland::toolbar::render) fn draw_label_center_color(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    style: UiTextStyle<'_>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    text: &str,
    color: (f64, f64, f64, f64),
) {
    let layout = engine.layout(ctx, style, text, None);
    let ext = layout.ink_extents();
    let tx = x + (w - ext.width()) / 2.0 - ext.x_bearing();
    let ty = y + (h - ext.height()) / 2.0 - ext.y_bearing();
    set_color(ctx, color);
    layout.show_at_baseline(ctx, tx, ty);
}

/// Truncate `text` with a trailing ellipsis so it fits `max_width`.
pub(in crate::backend::wayland::toolbar::render) fn ellipsize_to_width(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    style: UiTextStyle<'_>,
    text: &str,
    max_width: f64,
) -> String {
    if max_width <= 0.0 {
        return String::new();
    }
    if engine.layout(ctx, style, text, None).ink_extents().width() <= max_width {
        return text.to_string();
    }
    let mut chars: Vec<char> = text.chars().collect();
    while chars.len() > 3 {
        chars.pop();
        let candidate: String = chars.iter().collect();
        let candidate = format!("{candidate}...");
        if engine
            .layout(ctx, style, &candidate, None)
            .ink_extents()
            .width()
            <= max_width
        {
            return candidate;
        }
    }
    "...".to_string()
}

#[allow(clippy::too_many_arguments)]
pub(in crate::backend::wayland::toolbar::render) fn draw_label_left(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    style: UiTextStyle<'_>,
    x: f64,
    y: f64,
    _w: f64,
    h: f64,
    text: &str,
) {
    let layout = engine.layout(ctx, style, text, None);
    let ext = layout.ink_extents();
    let ty = y + (h - ext.height()) / 2.0 - ext.y_bearing();
    set_color(ctx, COLOR_TEXT_PRIMARY);
    layout.show_at_baseline(ctx, x, ty);
}

#[allow(clippy::too_many_arguments)]
pub(in crate::backend::wayland::toolbar::render) fn draw_label_left_wrapped(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    style: UiTextStyle<'_>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    text: &str,
) {
    let layout = engine.layout(ctx, style, text, Some(w));
    let ext = layout.ink_extents();
    let ty = y + (h - ext.height()) / 2.0 - ext.y_bearing();
    set_color(ctx, COLOR_TEXT_PRIMARY);
    layout.show_at_baseline(ctx, x, ty);
}
