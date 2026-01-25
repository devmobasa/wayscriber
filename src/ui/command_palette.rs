//! Command palette UI rendering.

use crate::input::InputState;
use crate::ui_text::{UiTextStyle, draw_text_baseline};

use super::constants::{
    self, BG_INPUT_SELECTION, BORDER_COMMAND_PALETTE, EMPTY_COMMAND_PALETTE,
    EMPTY_COMMAND_SUGGESTIONS, HINT_PRESS_ESC, INPUT_BG, INPUT_BORDER_FOCUSED, OVERLAY_DIM_MEDIUM,
    PANEL_BG_COMMAND_PALETTE, RADIUS_LG, RADIUS_SM, RADIUS_STD, SHADOW, SPACING_MD,
    TEXT_DESCRIPTION, TEXT_PLACEHOLDER, TEXT_WHITE,
};
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

    // Dimmed background overlay
    ctx.set_source_rgba(0.0, 0.0, 0.0, OVERLAY_DIM_MEDIUM);
    ctx.rectangle(0.0, 0.0, screen_width as f64, screen_height as f64);
    let _ = ctx.fill();

    // Drop shadow
    constants::set_color(ctx, SHADOW);
    draw_rounded_rect(ctx, x + 4.0, y + 4.0, PALETTE_WIDTH, height, RADIUS_LG);
    let _ = ctx.fill();

    // Main background
    constants::set_color(ctx, PANEL_BG_COMMAND_PALETTE);
    draw_rounded_rect(ctx, x, y, PALETTE_WIDTH, height, RADIUS_LG);
    let _ = ctx.fill();

    // Border
    constants::set_color(ctx, BORDER_COMMAND_PALETTE);
    draw_rounded_rect(ctx, x, y, PALETTE_WIDTH, height, RADIUS_LG);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    let inner_x = x + PADDING;
    let inner_width = PALETTE_WIDTH - PADDING * 2.0;
    let mut cursor_y = y + PADDING;

    // Input field
    draw_rounded_rect(
        ctx,
        inner_x,
        cursor_y,
        inner_width,
        INPUT_HEIGHT,
        RADIUS_STD,
    );
    constants::set_color(ctx, INPUT_BG);
    let _ = ctx.fill_preserve();
    constants::set_color(ctx, INPUT_BORDER_FOCUSED);
    ctx.set_line_width(1.5);
    let _ = ctx.stroke();

    // Input text
    let font_size = 14.0;
    let input_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: font_size,
    };
    let desc_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 12.0,
    };

    let text_y = cursor_y + INPUT_HEIGHT / 2.0 + font_size / 3.0;
    if input_state.command_palette_query.is_empty() {
        constants::set_color(ctx, TEXT_PLACEHOLDER);
        draw_text_baseline(
            ctx,
            input_style,
            "Type to search commands...",
            inner_x + 10.0,
            text_y,
            None,
        );
    } else {
        constants::set_color(ctx, TEXT_WHITE);
        draw_text_baseline(
            ctx,
            input_style,
            &input_state.command_palette_query,
            inner_x + 10.0,
            text_y,
            None,
        );
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
            draw_rounded_rect(
                ctx,
                inner_x,
                item_y,
                inner_width,
                ITEM_HEIGHT - 2.0,
                RADIUS_SM,
            );
            constants::set_color(ctx, BG_INPUT_SELECTION);
            let _ = ctx.fill();
        }

        // Command label
        let label_y = item_y + ITEM_HEIGHT / 2.0 + font_size / 3.0;
        let text_alpha = if is_selected { 1.0 } else { 0.85 };
        ctx.set_source_rgba(TEXT_WHITE.0, TEXT_WHITE.1, TEXT_WHITE.2, text_alpha);
        draw_text_baseline(ctx, input_style, cmd.label, inner_x + 10.0, label_y, None);

        // Keyboard shortcut badge (right-aligned)
        let shortcut_labels = input_state.action_binding_labels(cmd.action);
        let shortcut_x_end = inner_x + inner_width - 8.0;
        if let Some(shortcut) = shortcut_labels.first() {
            let shortcut_style = UiTextStyle {
                family: "Sans",
                slant: cairo::FontSlant::Normal,
                weight: cairo::FontWeight::Normal,
                size: 10.0,
            };
            let shortcut_extents = text_extents_for(
                ctx,
                "Sans",
                cairo::FontSlant::Normal,
                cairo::FontWeight::Normal,
                10.0,
                shortcut,
            );
            let badge_w = shortcut_extents.width() + 10.0;
            let badge_h = 18.0;
            let badge_x = shortcut_x_end - badge_w;
            let badge_y = item_y + (ITEM_HEIGHT - badge_h) / 2.0 - 1.0;

            // Badge background
            let badge_alpha = if is_selected { 0.25 } else { 0.15 };
            ctx.set_source_rgba(1.0, 1.0, 1.0, badge_alpha);
            draw_rounded_rect(ctx, badge_x, badge_y, badge_w, badge_h, 3.0);
            let _ = ctx.fill();

            // Badge text
            let shortcut_alpha = if is_selected { 0.9 } else { 0.7 };
            ctx.set_source_rgba(1.0, 1.0, 1.0, shortcut_alpha);
            draw_text_baseline(
                ctx,
                shortcut_style,
                shortcut,
                badge_x + 5.0,
                badge_y + badge_h / 2.0 + 3.0,
                None,
            );
        }

        // Description (dimmer but improved contrast)
        let label_extents = text_extents_for(
            ctx,
            "Sans",
            cairo::FontSlant::Normal,
            cairo::FontWeight::Normal,
            font_size,
            cmd.label,
        );
        // Limit description width to avoid overlapping with shortcut badge
        let max_desc_width = if !shortcut_labels.is_empty() {
            inner_width - label_extents.width() - 100.0
        } else {
            inner_width - label_extents.width() - 30.0
        };
        let desc_x = inner_x + 10.0 + label_extents.width() + 12.0;
        let desc_alpha = if is_selected { 0.9 } else { 0.75 };
        ctx.set_source_rgba(
            TEXT_DESCRIPTION.0,
            TEXT_DESCRIPTION.1,
            TEXT_DESCRIPTION.2,
            desc_alpha,
        );
        draw_text_baseline(
            ctx,
            desc_style,
            cmd.description,
            desc_x,
            label_y,
            Some(max_desc_width.max(50.0)),
        );
    }

    // Enhanced empty state
    if filtered.is_empty() && !input_state.command_palette_query.is_empty() {
        let empty_y = cursor_y + ITEM_HEIGHT;
        let center_x = inner_x + inner_width / 2.0;

        // Main message - larger and centered
        let empty_style = UiTextStyle {
            family: "Sans",
            slant: cairo::FontSlant::Normal,
            weight: cairo::FontWeight::Bold,
            size: font_size,
        };
        constants::set_color(ctx, TEXT_DESCRIPTION);
        let msg_extents = text_extents_for(
            ctx,
            "Sans",
            cairo::FontSlant::Normal,
            cairo::FontWeight::Bold,
            font_size,
            EMPTY_COMMAND_PALETTE,
        );
        draw_text_baseline(
            ctx,
            empty_style,
            EMPTY_COMMAND_PALETTE,
            center_x - msg_extents.width() / 2.0,
            empty_y,
            None,
        );

        // Suggestions
        let suggest_style = UiTextStyle {
            family: "Sans",
            slant: cairo::FontSlant::Italic,
            weight: cairo::FontWeight::Normal,
            size: 11.0,
        };
        ctx.set_source_rgba(
            TEXT_DESCRIPTION.0,
            TEXT_DESCRIPTION.1,
            TEXT_DESCRIPTION.2,
            0.7,
        );
        let suggest_extents = text_extents_for(
            ctx,
            "Sans",
            cairo::FontSlant::Italic,
            cairo::FontWeight::Normal,
            11.0,
            EMPTY_COMMAND_SUGGESTIONS,
        );
        draw_text_baseline(
            ctx,
            suggest_style,
            EMPTY_COMMAND_SUGGESTIONS,
            center_x - suggest_extents.width() / 2.0,
            empty_y + 20.0,
            None,
        );
    }

    // Escape hint at bottom
    let hint_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 11.0,
    };
    ctx.set_source_rgba(
        TEXT_DESCRIPTION.0,
        TEXT_DESCRIPTION.1,
        TEXT_DESCRIPTION.2,
        0.6,
    );
    let hint_y = y + height - SPACING_MD;
    let hint_extents = text_extents_for(
        ctx,
        "Sans",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        11.0,
        HINT_PRESS_ESC,
    );
    draw_text_baseline(
        ctx,
        hint_style,
        HINT_PRESS_ESC,
        x + (PALETTE_WIDTH - hint_extents.width()) / 2.0,
        hint_y,
        None,
    );
}
