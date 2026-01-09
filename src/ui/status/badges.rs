use super::super::primitives::{draw_rounded_rect, fallback_text_extents, text_extents_for};
use crate::input::InputState;

/// Render a small badge indicating frozen mode (visible even when status bar is hidden).
pub fn render_frozen_badge(ctx: &cairo::Context, screen_width: u32, _screen_height: u32) {
    let label = "FROZEN";
    let padding = 12.0;
    let radius = 8.0;
    let font_size = 16.0;

    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(font_size);

    let extents = ctx
        .text_extents(label)
        .unwrap_or_else(|_| fallback_text_extents(font_size, label));

    let width = extents.width() + padding * 1.4;
    let height = extents.height() + padding;

    let x = screen_width as f64 - width - padding;
    let y = padding + height;

    // Background with warning tint
    ctx.set_source_rgba(0.82, 0.22, 0.2, 0.9);
    draw_rounded_rect(ctx, x, y - height, width, height, radius);
    let _ = ctx.fill();

    // Text
    ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    ctx.move_to(x + (padding * 0.7), y - (padding * 0.35));
    let _ = ctx.show_text(label);
}

/// Render a small badge indicating zoom mode (visible even when status bar is hidden).
pub fn render_zoom_badge(
    ctx: &cairo::Context,
    screen_width: u32,
    _screen_height: u32,
    zoom_scale: f64,
    locked: bool,
) {
    let zoom_pct = (zoom_scale * 100.0).round() as i32;
    let label = if locked {
        format!("ZOOM {}% LOCKED", zoom_pct)
    } else {
        format!("ZOOM {}%", zoom_pct)
    };
    let padding = 12.0;
    let radius = 8.0;
    let font_size = 15.0;

    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(font_size);

    let extents = ctx
        .text_extents(&label)
        .unwrap_or_else(|_| fallback_text_extents(font_size, &label));

    let width = extents.width() + padding * 1.4;
    let height = extents.height() + padding;

    let x = screen_width as f64 - width - padding;
    let y = padding + height;

    // Background with teal tint
    ctx.set_source_rgba(0.2, 0.52, 0.7, 0.9);
    draw_rounded_rect(ctx, x, y - height, width, height, radius);
    let _ = ctx.fill();

    // Text
    ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    ctx.move_to(x + (padding * 0.7), y - (padding * 0.35));
    let _ = ctx.show_text(&label);
}

/// Render a small badge indicating the current page (visible even when status bar is hidden).
pub fn render_page_badge(
    ctx: &cairo::Context,
    _screen_width: u32,
    _screen_height: u32,
    page_index: usize,
    page_count: usize,
) {
    let label = format!("Page {}/{}", page_index + 1, page_count.max(1));
    let padding = 12.0;
    let radius = 8.0;
    let font_size = 15.0;

    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(font_size);

    let extents = ctx
        .text_extents(&label)
        .unwrap_or_else(|_| fallback_text_extents(font_size, &label));

    let width = extents.width() + padding * 1.4;
    let height = extents.height() + padding;

    let x = padding;
    let y = padding + height;

    // Background with a neutral cool tone.
    ctx.set_source_rgba(0.2, 0.32, 0.45, 0.92);
    draw_rounded_rect(ctx, x, y - height, width, height, radius);
    let _ = ctx.fill();

    // Text
    ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    ctx.move_to(x + (padding * 0.7), y - (padding * 0.35));
    let _ = ctx.show_text(&label);
}

/// Render the click-through escape hatch indicator.
pub fn render_clickthrough_hotspot(ctx: &cairo::Context, input_state: &InputState) {
    if !input_state.clickthrough_active() {
        return;
    }
    let Some(rect) = input_state.clickthrough_hotspot_rect() else {
        return;
    };

    let x = rect.x as f64;
    let y = rect.y as f64;
    let width = rect.width as f64;
    let height = rect.height as f64;
    let radius = (width.min(height) * 0.25).max(4.0);

    ctx.set_source_rgba(0.05, 0.05, 0.05, 0.6);
    draw_rounded_rect(ctx, x, y, width, height, radius);
    let _ = ctx.fill();

    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.85);
    ctx.set_line_width(1.5);
    let inner_radius = (radius - 1.0).max(0.0);
    draw_rounded_rect(
        ctx,
        x + 1.0,
        y + 1.0,
        (width - 2.0).max(1.0),
        (height - 2.0).max(1.0),
        inner_radius,
    );
    let _ = ctx.stroke();

    let label = "CT";
    let font_size = (height * 0.45).clamp(10.0, 14.0);
    let extents = text_extents_for(
        ctx,
        "Sans",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
        font_size,
        label,
    );
    let text_x = x + (width - extents.width()) / 2.0 - extents.x_bearing();
    let text_y = y + (height - extents.height()) / 2.0 - extents.y_bearing();
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    ctx.move_to(text_x, text_y);
    let _ = ctx.show_text(label);
}
