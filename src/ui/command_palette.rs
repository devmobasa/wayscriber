//! Command palette UI rendering.

use crate::config::action_meta::ActionMeta;
use crate::input::InputState;
use crate::input::state::{
    COMMAND_PALETTE_INPUT_HEIGHT, COMMAND_PALETTE_ITEM_HEIGHT, COMMAND_PALETTE_LIST_GAP,
    COMMAND_PALETTE_MAX_VISIBLE, COMMAND_PALETTE_PADDING, COMMAND_PALETTE_QUERY_PLACEHOLDER,
};
use crate::ui_text::{UiTextStyle, draw_text_baseline};

use super::constants::{
    self, BORDER_COMMAND_PALETTE, EMPTY_COMMAND_PALETTE, EMPTY_COMMAND_SUGGESTIONS, HINT_PRESS_ESC,
    INPUT_BG, INPUT_BORDER_FOCUSED, OVERLAY_DIM_MEDIUM, PANEL_BG_COMMAND_PALETTE, RADIUS_LG,
    RADIUS_STD, SHADOW, TEXT_DESCRIPTION, TEXT_PLACEHOLDER, TEXT_WHITE,
};
use super::primitives::{draw_rounded_rect, text_extents_for};

mod command_palette_row;

use self::command_palette_row::{command_palette_row_styles, render_command_row};

const HINT_BASELINE_BOTTOM_OFFSET: f64 = 12.0;
const ELLIPSIS: &str = "\u{2026}";
const COMMAND_PALETTE_FONT_FAMILY: &str = "Sans";
const COMMAND_PALETTE_LABEL_TEXT_SIZE: f64 = 14.0;
const COMMAND_PALETTE_DESC_TEXT_SIZE: f64 = 12.0;
const COMMAND_PALETTE_SHORTCUT_TEXT_SIZE: f64 = 10.0;
const COMMAND_PALETTE_HINT_TEXT_SIZE: f64 = 11.0;
const COMMAND_PALETTE_SHORTCUT_BADGE_PADDING_X: f64 = 5.0;
const COMMAND_PALETTE_SHORTCUT_BADGE_HEIGHT: f64 = 18.0;
const COMMAND_PALETTE_SHORTCUT_BADGE_GAP: f64 = 12.0;
const COMMAND_PALETTE_SHORTCUT_MIN_DESC_WIDTH: f64 = 48.0;

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
    let geometry =
        input_state.command_palette_geometry(screen_width, screen_height, filtered.len());
    let palette_width = geometry.width;
    let height = geometry.height;

    let x = geometry.x;
    let y = geometry.y;

    draw_command_palette_frame(
        ctx,
        screen_width as f64,
        screen_height as f64,
        x,
        y,
        palette_width,
        height,
    );

    let inner_x = x + COMMAND_PALETTE_PADDING;
    let inner_width = palette_width - COMMAND_PALETTE_PADDING * 2.0;
    let mut cursor_y = y + COMMAND_PALETTE_PADDING;

    cursor_y = draw_command_palette_input(
        ctx,
        inner_x,
        cursor_y,
        inner_width,
        &input_state.command_palette_query,
    );

    render_command_palette_rows(ctx, input_state, &filtered, inner_x, inner_width, cursor_y);

    if filtered.is_empty() && !input_state.command_palette_query.is_empty() {
        draw_command_palette_empty_state(
            ctx,
            inner_x,
            inner_width,
            cursor_y + COMMAND_PALETTE_ITEM_HEIGHT,
        );
    }

    render_command_palette_scroll_indicator(
        ctx,
        x,
        y,
        palette_width,
        cursor_y,
        filtered.len(),
        input_state.command_palette_scroll,
    );

    draw_command_palette_escape_hint(ctx, x, y, palette_width, height);
}

fn command_palette_text_style(
    size: f64,
    weight: cairo::FontWeight,
    slant: cairo::FontSlant,
) -> UiTextStyle<'static> {
    UiTextStyle {
        family: COMMAND_PALETTE_FONT_FAMILY,
        slant,
        weight,
        size,
    }
}

fn draw_command_palette_frame(
    ctx: &cairo::Context,
    screen_width: f64,
    screen_height: f64,
    x: f64,
    y: f64,
    palette_width: f64,
    height: f64,
) {
    ctx.set_source_rgba(0.0, 0.0, 0.0, OVERLAY_DIM_MEDIUM);
    ctx.rectangle(0.0, 0.0, screen_width, screen_height);
    let _ = ctx.fill();

    constants::set_color(ctx, SHADOW);
    draw_rounded_rect(ctx, x + 4.0, y + 4.0, palette_width, height, RADIUS_LG);
    let _ = ctx.fill();

    constants::set_color(ctx, PANEL_BG_COMMAND_PALETTE);
    draw_rounded_rect(ctx, x, y, palette_width, height, RADIUS_LG);
    let _ = ctx.fill();

    constants::set_color(ctx, BORDER_COMMAND_PALETTE);
    draw_rounded_rect(ctx, x, y, palette_width, height, RADIUS_LG);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();
}

fn draw_command_palette_input(
    ctx: &cairo::Context,
    inner_x: f64,
    mut cursor_y: f64,
    inner_width: f64,
    query: &str,
) -> f64 {
    draw_rounded_rect(
        ctx,
        inner_x,
        cursor_y,
        inner_width,
        COMMAND_PALETTE_INPUT_HEIGHT,
        RADIUS_STD,
    );
    constants::set_color(ctx, INPUT_BG);
    let _ = ctx.fill_preserve();
    constants::set_color(ctx, INPUT_BORDER_FOCUSED);
    ctx.set_line_width(1.5);
    let _ = ctx.stroke();

    let input_style = command_palette_text_style(
        COMMAND_PALETTE_LABEL_TEXT_SIZE,
        cairo::FontWeight::Normal,
        cairo::FontSlant::Normal,
    );
    let text_y = cursor_y + COMMAND_PALETTE_INPUT_HEIGHT / 2.0 + input_style.size / 3.0;

    if query.is_empty() {
        constants::set_color(ctx, TEXT_PLACEHOLDER);
        draw_text_baseline(
            ctx,
            input_style,
            COMMAND_PALETTE_QUERY_PLACEHOLDER,
            inner_x + 10.0,
            text_y,
            None,
        );
    } else {
        constants::set_color(ctx, TEXT_WHITE);
        draw_text_baseline(ctx, input_style, query, inner_x + 10.0, text_y, None);
    }

    cursor_y += COMMAND_PALETTE_INPUT_HEIGHT + COMMAND_PALETTE_LIST_GAP;
    cursor_y
}

fn render_command_palette_rows(
    ctx: &cairo::Context,
    input_state: &InputState,
    filtered: &[&'static ActionMeta],
    inner_x: f64,
    inner_width: f64,
    start_y: f64,
) {
    let styles = command_palette_row_styles();

    let scroll = input_state.command_palette_scroll;
    for (visible_idx, cmd) in filtered
        .iter()
        .skip(scroll)
        .take(COMMAND_PALETTE_MAX_VISIBLE)
        .enumerate()
    {
        let actual_idx = scroll + visible_idx;
        let is_selected = actual_idx == input_state.command_palette_selected;
        let item_y = start_y + (visible_idx as f64 * COMMAND_PALETTE_ITEM_HEIGHT);
        render_command_row(
            ctx,
            input_state,
            cmd,
            &styles,
            inner_x,
            inner_width,
            item_y,
            is_selected,
        );
    }
}

fn draw_command_palette_empty_state(
    ctx: &cairo::Context,
    inner_x: f64,
    inner_width: f64,
    empty_y: f64,
) {
    let center_x = inner_x + inner_width / 2.0;

    let empty_style = command_palette_text_style(
        COMMAND_PALETTE_LABEL_TEXT_SIZE,
        cairo::FontWeight::Bold,
        cairo::FontSlant::Normal,
    );
    constants::set_color(ctx, TEXT_DESCRIPTION);
    let msg_extents = text_extents_for(
        ctx,
        COMMAND_PALETTE_FONT_FAMILY,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
        empty_style.size,
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

    let suggest_style = command_palette_text_style(
        COMMAND_PALETTE_HINT_TEXT_SIZE,
        cairo::FontWeight::Normal,
        cairo::FontSlant::Italic,
    );
    ctx.set_source_rgba(
        TEXT_DESCRIPTION.0,
        TEXT_DESCRIPTION.1,
        TEXT_DESCRIPTION.2,
        0.7,
    );
    let suggest_extents = text_extents_for(
        ctx,
        COMMAND_PALETTE_FONT_FAMILY,
        cairo::FontSlant::Italic,
        cairo::FontWeight::Normal,
        suggest_style.size,
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

fn render_command_palette_scroll_indicator(
    ctx: &cairo::Context,
    x: f64,
    _y: f64,
    palette_width: f64,
    start_y: f64,
    total_items: usize,
    scroll: usize,
) {
    if total_items <= COMMAND_PALETTE_MAX_VISIBLE {
        return;
    }

    let scroll_track_x = x + palette_width - 8.0;
    let scroll_track_h = (COMMAND_PALETTE_MAX_VISIBLE as f64) * COMMAND_PALETTE_ITEM_HEIGHT - 4.0;
    let scroll_track_w = 4.0;

    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.1);
    draw_rounded_rect(
        ctx,
        scroll_track_x,
        start_y,
        scroll_track_w,
        scroll_track_h,
        2.0,
    );
    let _ = ctx.fill();

    let thumb_ratio = COMMAND_PALETTE_MAX_VISIBLE as f64 / total_items as f64;
    let thumb_h = (scroll_track_h * thumb_ratio).max(20.0);
    let scroll_range = total_items - COMMAND_PALETTE_MAX_VISIBLE;
    let scroll_progress = if scroll_range > 0 {
        scroll as f64 / scroll_range as f64
    } else {
        0.0
    };
    let thumb_y = start_y + scroll_progress * (scroll_track_h - thumb_h);

    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.35);
    draw_rounded_rect(ctx, scroll_track_x, thumb_y, scroll_track_w, thumb_h, 2.0);
    let _ = ctx.fill();
}

fn draw_command_palette_escape_hint(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    palette_width: f64,
    height: f64,
) {
    let hint_style = command_palette_text_style(
        COMMAND_PALETTE_HINT_TEXT_SIZE,
        cairo::FontWeight::Normal,
        cairo::FontSlant::Normal,
    );
    ctx.set_source_rgba(
        TEXT_DESCRIPTION.0,
        TEXT_DESCRIPTION.1,
        TEXT_DESCRIPTION.2,
        0.6,
    );
    let hint_y = y + height - HINT_BASELINE_BOTTOM_OFFSET;
    let hint_extents = text_extents_for(
        ctx,
        COMMAND_PALETTE_FONT_FAMILY,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        COMMAND_PALETTE_HINT_TEXT_SIZE,
        HINT_PRESS_ESC,
    );
    draw_text_baseline(
        ctx,
        hint_style,
        HINT_PRESS_ESC,
        x + (palette_width - hint_extents.width()) / 2.0,
        hint_y,
        None,
    );
}

fn ellipsize_to_width(
    ctx: &cairo::Context,
    text: &str,
    family: &str,
    slant: cairo::FontSlant,
    weight: cairo::FontWeight,
    size: f64,
    max_width: f64,
) -> String {
    if max_width <= 0.0 {
        return String::new();
    }

    let extents = text_extents_for(ctx, family, slant, weight, size, text);
    if extents.width() <= max_width {
        return text.to_string();
    }

    let ellipsis_extents = text_extents_for(ctx, family, slant, weight, size, ELLIPSIS);
    if ellipsis_extents.width() > max_width {
        return String::new();
    }

    let mut end = text.len();
    while end > 0 {
        if !text.is_char_boundary(end) {
            end -= 1;
            continue;
        }
        let candidate = format!("{}{}", &text[..end], ELLIPSIS);
        let candidate_extents = text_extents_for(ctx, family, slant, weight, size, &candidate);
        if candidate_extents.width() <= max_width {
            return candidate;
        }
        end -= 1;
    }

    ELLIPSIS.to_string()
}
