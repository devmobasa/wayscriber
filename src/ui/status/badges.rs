use super::super::primitives::draw_rounded_rect;
use crate::ui_text::{UiTextStyle, text_layout};

/// Render a small badge indicating frozen mode (visible even when status bar is hidden).
pub fn render_frozen_badge(ctx: &cairo::Context, screen_width: u32, _screen_height: u32) {
    let label = "FROZEN";
    let padding = 12.0;
    let radius = 8.0;
    let font_size = 16.0;
    let layout = text_layout(
        ctx,
        UiTextStyle {
            family: "Sans",
            slant: cairo::FontSlant::Normal,
            weight: cairo::FontWeight::Bold,
            size: font_size,
        },
        label,
        None,
    );
    let extents = layout.ink_extents();

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
    layout.show_at_baseline(ctx, x + (padding * 0.7), y - (padding * 0.35));
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
    let layout = text_layout(
        ctx,
        UiTextStyle {
            family: "Sans",
            slant: cairo::FontSlant::Normal,
            weight: cairo::FontWeight::Bold,
            size: font_size,
        },
        &label,
        None,
    );
    let extents = layout.ink_extents();

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
    layout.show_at_baseline(ctx, x + (padding * 0.7), y - (padding * 0.35));
}

/// Render a small badge indicating the current page (visible even when status bar is hidden).
#[allow(clippy::too_many_arguments)]
pub fn render_page_badge(
    ctx: &cairo::Context,
    _screen_width: u32,
    _screen_height: u32,
    board_index: usize,
    board_count: usize,
    board_name: &str,
    page_index: usize,
    page_count: usize,
) {
    let truncated_name = crate::util::truncate_with_ellipsis(board_name, 20);
    let board_label = if !truncated_name.trim().is_empty() {
        if board_count > 1 {
            Some(format!(
                "Board {}/{}: {}",
                board_index + 1,
                board_count.max(1),
                truncated_name
            ))
        } else {
            Some(format!("Board: {}", truncated_name))
        }
    } else if board_count > 1 {
        Some(format!("Board {}/{}", board_index + 1, board_count.max(1)))
    } else {
        None
    };
    let page_label = if page_count > 1 {
        Some(format!("Page {}/{}", page_index + 1, page_count.max(1)))
    } else {
        None
    };
    let label = match (board_label, page_label) {
        (Some(board), Some(page)) => format!("{board} | {page}"),
        (Some(board), None) => board,
        (None, Some(page)) => page,
        (None, None) => return,
    };
    let padding = 12.0;
    let edge_padding = 4.0;
    let radius = 8.0;
    let font_size = 15.0;
    let layout = text_layout(
        ctx,
        UiTextStyle {
            family: "Sans",
            slant: cairo::FontSlant::Normal,
            weight: cairo::FontWeight::Bold,
            size: font_size,
        },
        &label,
        None,
    );
    let extents = layout.ink_extents();

    let width = extents.width() + padding * 1.4;
    let height = extents.height() + padding;

    let x = padding;
    let y = edge_padding + height;

    // Background with a neutral cool tone.
    ctx.set_source_rgba(0.2, 0.32, 0.45, 0.92);
    draw_rounded_rect(ctx, x, y - height, width, height, radius);
    let _ = ctx.fill();

    // Text
    ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    layout.show_at_baseline(ctx, x + (padding * 0.7), y - (padding * 0.5));
}
