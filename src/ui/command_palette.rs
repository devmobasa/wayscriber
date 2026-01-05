//! Command palette UI rendering.

use crate::input::InputState;

use super::primitives::{draw_rounded_rect, text_extents_for};

const PALETTE_WIDTH: f64 = 400.0;
const PALETTE_MAX_HEIGHT: f64 = 420.0;
const ITEM_HEIGHT: f64 = 32.0;
const PADDING: f64 = 12.0;
const PADDING_BOTTOM: f64 = 24.0;
const INPUT_HEIGHT: f64 = 36.0;
const MAX_VISIBLE_ITEMS: usize = 10;

/// Render the command palette if open.
pub fn render_command_palette(
    ctx: &cairo::Context,
    input_state: &InputState,
    screen_width: u32,
    screen_height: u32,
) {
    if !input_state.command_palette_open {
        return;
    }

    let filtered = input_state.filtered_commands();
    let visible_count = filtered.len().min(MAX_VISIBLE_ITEMS);
    let content_height =
        INPUT_HEIGHT + (visible_count as f64 * ITEM_HEIGHT) + PADDING + PADDING_BOTTOM;
    let height = content_height.min(PALETTE_MAX_HEIGHT);

    let x = (screen_width as f64 - PALETTE_WIDTH) / 2.0;
    let y = screen_height as f64 * 0.2;

    // Background with shadow
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.3);
    draw_rounded_rect(ctx, x + 4.0, y + 4.0, PALETTE_WIDTH, height, 10.0);
    let _ = ctx.fill();

    // Main background
    ctx.set_source_rgba(0.15, 0.15, 0.18, 0.98);
    draw_rounded_rect(ctx, x, y, PALETTE_WIDTH, height, 10.0);
    let _ = ctx.fill();

    // Border
    ctx.set_source_rgba(0.4, 0.4, 0.45, 0.5);
    draw_rounded_rect(ctx, x, y, PALETTE_WIDTH, height, 10.0);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    let inner_x = x + PADDING;
    let inner_width = PALETTE_WIDTH - PADDING * 2.0;
    let mut cursor_y = y + PADDING;

    // Input field
    draw_rounded_rect(ctx, inner_x, cursor_y, inner_width, INPUT_HEIGHT, 6.0);
    ctx.set_source_rgba(0.1, 0.1, 0.12, 1.0);
    let _ = ctx.fill_preserve();
    ctx.set_source_rgba(0.3, 0.5, 0.8, 0.6);
    ctx.set_line_width(1.5);
    let _ = ctx.stroke();

    // Input text
    let font_size = 14.0;
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(font_size);

    let text_y = cursor_y + INPUT_HEIGHT / 2.0 + font_size / 3.0;
    if input_state.command_palette_query.is_empty() {
        ctx.set_source_rgba(0.5, 0.5, 0.55, 0.7);
        ctx.move_to(inner_x + 10.0, text_y);
        let _ = ctx.show_text("Type to search commands...");
    } else {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
        ctx.move_to(inner_x + 10.0, text_y);
        let _ = ctx.show_text(&input_state.command_palette_query);
    }

    cursor_y += INPUT_HEIGHT + 8.0;

    // Command list (with scroll offset)
    let scroll = input_state.command_palette_scroll;
    for (visible_idx, cmd) in filtered
        .iter()
        .skip(scroll)
        .take(MAX_VISIBLE_ITEMS)
        .enumerate()
    {
        let actual_idx = scroll + visible_idx;
        let is_selected = actual_idx == input_state.command_palette_selected;
        let item_y = cursor_y + (visible_idx as f64 * ITEM_HEIGHT);

        // Selection highlight
        if is_selected {
            draw_rounded_rect(ctx, inner_x, item_y, inner_width, ITEM_HEIGHT - 2.0, 4.0);
            ctx.set_source_rgba(0.3, 0.5, 0.8, 0.4);
            let _ = ctx.fill();
        }

        // Command label
        let label_y = item_y + ITEM_HEIGHT / 2.0 + font_size / 3.0;
        ctx.set_source_rgba(1.0, 1.0, 1.0, if is_selected { 1.0 } else { 0.85 });
        ctx.move_to(inner_x + 10.0, label_y);
        let _ = ctx.show_text(cmd.label);

        // Description (dimmer)
        let label_extents = text_extents_for(
            ctx,
            "Sans",
            cairo::FontSlant::Normal,
            cairo::FontWeight::Normal,
            font_size,
            cmd.label,
        );
        let desc_x = inner_x + 10.0 + label_extents.width() + 12.0;
        ctx.set_source_rgba(0.6, 0.6, 0.65, if is_selected { 0.9 } else { 0.6 });
        ctx.set_font_size(12.0);
        ctx.move_to(desc_x, label_y);
        let _ = ctx.show_text(cmd.description);
        ctx.set_font_size(font_size);
    }

    // Show "no results" if empty
    if filtered.is_empty() && !input_state.command_palette_query.is_empty() {
        ctx.set_source_rgba(0.6, 0.6, 0.65, 0.8);
        ctx.move_to(
            inner_x + 10.0,
            cursor_y + ITEM_HEIGHT / 2.0 + font_size / 3.0,
        );
        let _ = ctx.show_text("No matching commands");
    }
}
