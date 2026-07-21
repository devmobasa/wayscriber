//! Command palette UI rendering.

use crate::config::action_label;
use crate::input::InputState;
use crate::input::state::{
    COMMAND_PALETTE_INPUT_HEIGHT, COMMAND_PALETTE_ITEM_HEIGHT, COMMAND_PALETTE_LIST_GAP,
    COMMAND_PALETTE_MAX_VISIBLE, COMMAND_PALETTE_PADDING, COMMAND_PALETTE_QUERY_PLACEHOLDER,
    COMMAND_PALETTE_TOP_RATIO, CommandPaletteListRow,
};
use crate::ui_text::{UiTextStyle, draw_text_baseline, measure_text};

use super::constants::{
    self, BORDER_COMMAND_PALETTE, EMPTY_COMMAND_PALETTE, EMPTY_COMMAND_SUGGESTIONS, INPUT_BG,
    INPUT_BORDER_FOCUSED, OVERLAY_DIM_MEDIUM, PANEL_BG_COMMAND_PALETTE, RADIUS_LG, RADIUS_STD,
    SHADOW, TEXT_DESCRIPTION, TEXT_PLACEHOLDER, TEXT_WHITE,
};
use super::primitives::{draw_rounded_rect, text_extents_for};
use super::theme::Rgba;

mod command_palette_row;

use self::command_palette_row::{command_palette_row_styles, render_command_row};

const HINT_BASELINE_BOTTOM_OFFSET: f64 = 12.0;
const ELLIPSIS: &str = "\u{2026}";
const COMMAND_PALETTE_FONT_FAMILY: &str = "Sans";
const COMMAND_PALETTE_LABEL_TEXT_SIZE: f64 = 14.0;
const COMMAND_PALETTE_HEADER_TEXT_SIZE: f64 = 10.0;
const COMMAND_PALETTE_DESC_TEXT_SIZE: f64 = 12.0;
const COMMAND_PALETTE_SHORTCUT_TEXT_SIZE: f64 = 10.0;
const COMMAND_PALETTE_HINT_TEXT_SIZE: f64 = 11.0;
const COMMAND_PALETTE_SHORTCUT_BADGE_PADDING_X: f64 = 5.0;
const COMMAND_PALETTE_SHORTCUT_BADGE_HEIGHT: f64 = 18.0;
const COMMAND_PALETTE_SHORTCUT_BADGE_GAP: f64 = 12.0;
const COMMAND_PALETTE_SHORTCUT_BADGE_RADIUS: f64 = 3.0;
const COMMAND_PALETTE_SHORTCUT_MIN_DESC_WIDTH: f64 = 48.0;
const COMMAND_PALETTE_INPUT_HINT: &str =
    "Enter run • Ctrl+E edit • Ctrl+Delete unbind • Ctrl+R reset • Esc close";
/// Offset of the frame drop shadow below/right of the palette.
const FRAME_SHADOW_OFFSET: f64 = 4.0;
const TOOLTIP_PADDING_X: f64 = 8.0;
const TOOLTIP_PADDING_Y: f64 = 5.0;
const TOOLTIP_POINTER_OFFSET: f64 = 12.0;
/// Action tooltip surface: darker than PANEL_BG_COMMAND_PALETTE so the
/// tooltip reads above the palette (no matching theme token; kept).
const TOOLTIP_BG: Rgba = (0.04, 0.05, 0.07, 0.98);
/// Scrollbar track/thumb white-alpha ladder.
/// TODO(theme-consolidation): thumb duplicates
/// `theme::toolbar::COLOR_SCROLLBAR_SLIDER`; track has no token.
const SCROLL_TRACK: Rgba = (1.0, 1.0, 1.0, 0.1);
const SCROLL_THUMB: Rgba = (1.0, 1.0, 1.0, 0.35);

/// Render the command palette if open.
pub fn render_command_palette(
    ctx: &cairo::Context,
    input_state: &InputState,
    screen_width: u32,
    screen_height: u32,
) {
    if !input_state.command_palette_is_engaged() {
        return;
    }

    if let Some(action) = input_state.keybinding_capture_action {
        render_keybinding_capture(ctx, input_state, action, screen_width, screen_height);
        return;
    }

    let rows = input_state.command_palette_rows();
    let geometry =
        input_state.command_palette_geometry_for_rows(screen_width, screen_height, &rows);
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

    render_command_palette_rows(ctx, input_state, &rows, inner_x, inner_width, cursor_y);

    if rows.is_empty() && !input_state.command_palette_query.is_empty() {
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
        rows.len(),
        input_state.command_palette_scroll,
    );

    if let Some((tooltip, pointer_x, pointer_y)) =
        input_state.command_palette_action_tooltip_for_layout(&rows, geometry)
    {
        draw_command_palette_action_tooltip(
            ctx,
            tooltip,
            pointer_x as f64,
            pointer_y as f64,
            screen_width as f64,
            screen_height as f64,
        );
    }

    draw_command_palette_escape_hint(ctx, x, y, palette_width, height);
}

/// Bounds of every pixel the command palette may change this frame, excluding
/// the full-screen dimmer whose content is stable between open and close.
/// Open/close already force full damage; query, selection, scroll, and tooltip
/// updates can therefore redraw only this compact footprint.
pub fn command_palette_visual_geometry(
    input_state: &InputState,
    screen_width: u32,
    screen_height: u32,
) -> Option<(f64, f64, f64, f64)> {
    if !input_state.command_palette_is_engaged() {
        return None;
    }

    if input_state.keybinding_capture_action.is_some() {
        let width = 520.0_f64.min(screen_width as f64 - 24.0);
        let height = 170.0;
        let x = (screen_width as f64 - width) / 2.0;
        let y = screen_height as f64 * COMMAND_PALETTE_TOP_RATIO;
        return Some((
            x,
            y,
            width + FRAME_SHADOW_OFFSET,
            height + FRAME_SHADOW_OFFSET,
        ));
    }

    let rows = input_state.command_palette_rows();
    let geometry =
        input_state.command_palette_geometry_for_rows(screen_width, screen_height, &rows);
    let mut bounds = (
        geometry.x,
        geometry.y,
        geometry.width + FRAME_SHADOW_OFFSET,
        geometry.height + FRAME_SHADOW_OFFSET,
    );

    if let Some((tooltip, pointer_x, pointer_y)) =
        input_state.command_palette_action_tooltip_for_layout(&rows, geometry)
        && let Some(tooltip_bounds) = command_palette_action_tooltip_geometry(
            tooltip,
            pointer_x as f64,
            pointer_y as f64,
            screen_width as f64,
            screen_height as f64,
        )
    {
        bounds = union_bounds(bounds, tooltip_bounds);
    }

    Some(bounds)
}

fn union_bounds(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> (f64, f64, f64, f64) {
    let min_x = a.0.min(b.0);
    let min_y = a.1.min(b.1);
    let max_x = (a.0 + a.2).max(b.0 + b.2);
    let max_y = (a.1 + a.3).max(b.1 + b.3);
    (min_x, min_y, max_x - min_x, max_y - min_y)
}

fn command_palette_action_tooltip_geometry(
    text: &str,
    pointer_x: f64,
    pointer_y: f64,
    screen_width: f64,
    screen_height: f64,
) -> Option<(f64, f64, f64, f64)> {
    let style = command_palette_text_style(
        COMMAND_PALETTE_SHORTCUT_TEXT_SIZE,
        cairo::FontWeight::Normal,
        cairo::FontSlant::Normal,
    );
    let extents = measure_text(style, text, None)?;
    let width = extents.width() + TOOLTIP_PADDING_X * 2.0;
    let height = style.size + TOOLTIP_PADDING_Y * 2.0;
    let x = (pointer_x + TOOLTIP_POINTER_OFFSET)
        .min((screen_width - width - FRAME_SHADOW_OFFSET).max(FRAME_SHADOW_OFFSET));
    let y = (pointer_y + TOOLTIP_POINTER_OFFSET)
        .min((screen_height - height - FRAME_SHADOW_OFFSET).max(FRAME_SHADOW_OFFSET));
    Some((x, y, width, height))
}

fn draw_command_palette_action_tooltip(
    ctx: &cairo::Context,
    text: &str,
    pointer_x: f64,
    pointer_y: f64,
    screen_width: f64,
    screen_height: f64,
) {
    let style = command_palette_text_style(
        COMMAND_PALETTE_SHORTCUT_TEXT_SIZE,
        cairo::FontWeight::Normal,
        cairo::FontSlant::Normal,
    );
    let Some((x, y, width, height)) = command_palette_action_tooltip_geometry(
        text,
        pointer_x,
        pointer_y,
        screen_width,
        screen_height,
    ) else {
        return;
    };

    constants::set_color(ctx, TOOLTIP_BG);
    draw_rounded_rect(ctx, x, y, width, height, 5.0);
    let _ = ctx.fill();
    constants::set_color(ctx, TEXT_WHITE);
    draw_text_baseline(
        ctx,
        style,
        text,
        x + TOOLTIP_PADDING_X,
        y + TOOLTIP_PADDING_Y + style.size,
        None,
    );
}

fn render_keybinding_capture(
    ctx: &cairo::Context,
    input_state: &InputState,
    action: crate::config::Action,
    screen_width: u32,
    screen_height: u32,
) {
    let width = 520.0_f64.min(screen_width as f64 - 24.0);
    let height = 170.0;
    let x = (screen_width as f64 - width) / 2.0;
    let y = screen_height as f64 * COMMAND_PALETTE_TOP_RATIO;
    draw_command_palette_frame(
        ctx,
        screen_width as f64,
        screen_height as f64,
        x,
        y,
        width,
        height,
    );

    let title_style =
        command_palette_text_style(18.0, cairo::FontWeight::Bold, cairo::FontSlant::Normal);
    let body_style =
        command_palette_text_style(13.0, cairo::FontWeight::Normal, cairo::FontSlant::Normal);
    constants::set_color(ctx, TEXT_WHITE);
    draw_text_baseline(
        ctx,
        title_style,
        &format!("Rebind {}", action_label(action)),
        x + 22.0,
        y + 38.0,
        None,
    );
    let current = input_state.action_binding_labels(action);
    constants::set_color(ctx, TEXT_DESCRIPTION);
    draw_text_baseline(
        ctx,
        body_style,
        &format!(
            "Current: {}",
            if current.is_empty() {
                "Not bound".to_string()
            } else {
                current.join(", ")
            }
        ),
        x + 22.0,
        y + 70.0,
        None,
    );
    constants::set_color(ctx, TEXT_WHITE);
    draw_text_baseline(
        ctx,
        body_style,
        "Press the new shortcut now",
        x + 22.0,
        y + 108.0,
        None,
    );
    constants::set_color(ctx, TEXT_DESCRIPTION);
    draw_text_baseline(
        ctx,
        body_style,
        "Escape cancels • conflicts are rejected without changing your config",
        x + 22.0,
        y + 140.0,
        None,
    );
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
    draw_rounded_rect(
        ctx,
        x + FRAME_SHADOW_OFFSET,
        y + FRAME_SHADOW_OFFSET,
        palette_width,
        height,
        RADIUS_LG,
    );
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
    rows: &[CommandPaletteListRow],
    inner_x: f64,
    inner_width: f64,
    start_y: f64,
) {
    let styles = command_palette_row_styles();

    let scroll = input_state.command_palette_scroll;
    for (visible_idx, row) in rows
        .iter()
        .skip(scroll)
        .take(COMMAND_PALETTE_MAX_VISIBLE)
        .enumerate()
    {
        let item_y = start_y + (visible_idx as f64 * COMMAND_PALETTE_ITEM_HEIGHT);
        match row {
            CommandPaletteListRow::Header(label) => {
                render_command_group_header(ctx, label, inner_x, inner_width, item_y);
            }
            CommandPaletteListRow::Command {
                command,
                command_index,
            } => {
                let is_selected = *command_index == input_state.command_palette_selected;
                render_command_row(
                    ctx,
                    input_state,
                    command,
                    &styles,
                    inner_x,
                    inner_width,
                    item_y,
                    is_selected,
                );
            }
        }
    }
}

/// Group header row: small uppercase label with a hairline rule filling the
/// remaining width. Occupies a full item row so hit-testing stays uniform.
fn render_command_group_header(
    ctx: &cairo::Context,
    label: &str,
    inner_x: f64,
    inner_width: f64,
    item_y: f64,
) {
    let style = command_palette_text_style(
        COMMAND_PALETTE_HEADER_TEXT_SIZE,
        cairo::FontWeight::Bold,
        cairo::FontSlant::Normal,
    );
    let text = label.to_uppercase();
    let baseline = item_y + COMMAND_PALETTE_ITEM_HEIGHT / 2.0 + style.size / 3.0;
    constants::set_color(ctx, constants::TEXT_HINT);
    let extents = draw_text_baseline(ctx, style, &text, inner_x + 10.0, baseline, None);

    let rule_start = inner_x + 10.0 + extents.width() + 10.0;
    let rule_end = inner_x + inner_width - 8.0;
    if rule_end > rule_start {
        constants::set_color(ctx, constants::DIVIDER_LIGHT);
        ctx.set_line_width(1.0);
        ctx.move_to(rule_start, baseline - style.size / 3.0);
        ctx.line_to(rule_end, baseline - style.size / 3.0);
        let _ = ctx.stroke();
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
    constants::set_color(ctx, constants::with_alpha(TEXT_DESCRIPTION, 0.7));
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

    constants::set_color(ctx, SCROLL_TRACK);
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

    constants::set_color(ctx, SCROLL_THUMB);
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
    constants::set_color(ctx, constants::with_alpha(TEXT_DESCRIPTION, 0.6));
    let hint_y = y + height - HINT_BASELINE_BOTTOM_OFFSET;
    let hint_extents = text_extents_for(
        ctx,
        COMMAND_PALETTE_FONT_FAMILY,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        COMMAND_PALETTE_HINT_TEXT_SIZE,
        COMMAND_PALETTE_INPUT_HINT,
    );
    draw_text_baseline(
        ctx,
        hint_style,
        COMMAND_PALETTE_INPUT_HINT,
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

    let boundaries: Vec<usize> = text
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(text.len()))
        .collect();
    let mut low = 0;
    let mut high = boundaries.len() - 1;
    while low < high {
        let mid = (low + high).div_ceil(2);
        let candidate = format!("{}{}", &text[..boundaries[mid]], ELLIPSIS);
        let candidate_extents = text_extents_for(ctx, family, slant, weight, size, &candidate);
        if candidate_extents.width() <= max_width {
            low = mid;
        } else {
            high = mid - 1;
        }
    }

    format!("{}{}", &text[..boundaries[low]], ELLIPSIS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context() -> cairo::Context {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 320, 80).expect("surface");
        cairo::Context::new(&surface).expect("context")
    }

    fn text_width(ctx: &cairo::Context, text: &str) -> f64 {
        text_extents_for(
            ctx,
            COMMAND_PALETTE_FONT_FAMILY,
            cairo::FontSlant::Normal,
            cairo::FontWeight::Normal,
            COMMAND_PALETTE_DESC_TEXT_SIZE,
            text,
        )
        .width()
    }

    #[test]
    fn ellipsize_keeps_full_text_when_it_fits() {
        let ctx = test_context();
        let text = "Short label";
        assert_eq!(
            ellipsize_to_width(
                &ctx,
                text,
                COMMAND_PALETTE_FONT_FAMILY,
                cairo::FontSlant::Normal,
                cairo::FontWeight::Normal,
                COMMAND_PALETTE_DESC_TEXT_SIZE,
                text_width(&ctx, text),
            ),
            text
        );
    }

    #[test]
    fn ellipsize_binary_search_respects_width_and_unicode_boundaries() {
        let ctx = test_context();
        let text = "Capture 🖌️ annotation history safely";
        let max_width = text_width(&ctx, "Capture 🖌️…");
        let result = ellipsize_to_width(
            &ctx,
            text,
            COMMAND_PALETTE_FONT_FAMILY,
            cairo::FontSlant::Normal,
            cairo::FontWeight::Normal,
            COMMAND_PALETTE_DESC_TEXT_SIZE,
            max_width,
        );

        assert!(result.ends_with(ELLIPSIS));
        assert!(text_width(&ctx, &result) <= max_width);
        assert!(result.is_char_boundary(result.len()));
    }
}
