mod thumbnail;

use crate::input::InputState;
use crate::input::state::{
    BoardPickerFocus, BoardPickerLayout, PAGE_NAME_HEIGHT, PAGE_NAME_PADDING,
};
use crate::ui::constants::{
    self, BG_HOVER, DIVIDER_LIGHT, TEXT_HINT, TEXT_SECONDARY, TEXT_TERTIARY,
};
use crate::ui::primitives::draw_rounded_rect;

use thumbnail::{
    PagePreviewArgs, PageThumbnailArgs, render_add_page_card, render_page_preview,
    render_page_thumbnail,
};

pub(super) fn render_page_panel(
    ctx: &cairo::Context,
    input_state: &InputState,
    layout: BoardPickerLayout,
    screen_width: u32,
    screen_height: u32,
) {
    if !layout.page_panel_enabled {
        return;
    }
    let Some(board_index) = layout.page_board_index else {
        return;
    };
    let Some(board) = input_state.boards.board_states().get(board_index) else {
        return;
    };
    let pages = board.pages.pages();
    let page_count = board.pages.page_count();
    let drag = input_state.board_picker_page_drag;
    let is_dragging = drag.is_some_and(|d| d.board_index == board_index);

    // Vertical divider between board list and page panel
    {
        let divider_x = layout.page_panel_x - 8.0;
        let divider_top = layout.origin_y + layout.padding_y;
        let divider_bottom = layout.origin_y + layout.height - layout.padding_y;
        ctx.set_source_rgba(DIVIDER_LIGHT.0, DIVIDER_LIGHT.1, DIVIDER_LIGHT.2, 0.4);
        ctx.set_line_width(1.0);
        ctx.move_to(divider_x, divider_top);
        ctx.line_to(divider_x, divider_bottom);
        let _ = ctx.stroke();
    }

    // Header: show "drag to reorder" hint only during drag
    let label = if is_dragging && page_count > 1 {
        format!("Pages — {}  • drag to reorder", board.spec.name)
    } else {
        format!("Pages — {}", board.spec.name)
    };
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(layout.footer_font_size);
    constants::set_color(ctx, TEXT_TERTIARY);
    let label_y = layout.origin_y + layout.padding_y + layout.title_font_size;
    ctx.move_to(layout.page_panel_x + 2.0, label_y);
    let _ = ctx.show_text(&label);

    let (pointer_x, pointer_y) = input_state.pointer_position();
    let start_x = layout.page_viewport_x;
    let start_y = layout.page_viewport_y;
    let row_stride =
        layout.page_thumb_height + PAGE_NAME_HEIGHT + PAGE_NAME_PADDING + layout.page_thumb_gap;
    let cols = layout.page_cols.max(1);

    // Handle empty state - show "Add your first page" CTA
    if page_count == 0 {
        let add_hover = input_state.board_picker_page_add_card_at(pointer_x, pointer_y);
        render_add_page_card(
            ctx,
            start_x,
            start_y,
            layout.page_thumb_width,
            layout.page_thumb_height,
            add_hover,
            true,
        );
        return;
    }

    let active_page = board.pages.active_index();
    let page_focus_page_index = if input_state.board_picker_focus() == BoardPickerFocus::PagePanel {
        input_state.board_picker_page_focus_page_index()
    } else {
        None
    };
    let hover_index = input_state.board_picker_page_index_at(pointer_x, pointer_y);
    let hover_delete = input_state.board_picker_page_delete_index_at(pointer_x, pointer_y);
    let hover_duplicate = input_state.board_picker_page_duplicate_index_at(pointer_x, pointer_y);
    let hover_rename = input_state.board_picker_page_rename_index_at(pointer_x, pointer_y);
    let first_visible = layout.page_first_visible_index.min(page_count);
    let visible = layout
        .page_visible_count
        .min(page_count.saturating_sub(first_visible));
    let visible_slots = layout.page_visible_slots.max(visible);

    for slot in 0..visible {
        let index = first_visible + slot;
        let Some(page) = pages.get(index) else {
            continue;
        };
        let col = slot % cols;
        let row = slot / cols;
        let thumb_x = start_x + col as f64 * (layout.page_thumb_width + layout.page_thumb_gap);
        let thumb_y = start_y + row as f64 * row_stride;
        let is_active = index == active_page;
        let is_drop_target = drag.is_some_and(|d| {
            d.board_index == board_index
                && d.target_board == Some(board_index)
                && d.current_index == index
        });
        render_page_thumbnail(PageThumbnailArgs {
            ctx,
            frame: page,
            background: &board.spec.background,
            x: thumb_x,
            y: thumb_y,
            width: layout.page_thumb_width,
            height: layout.page_thumb_height,
            screen_width,
            screen_height,
            page_number: index + 1,
            page_name: page.page_name(),
            is_active,
            is_drop_target,
            is_search_match: input_state.board_picker_page_matches_current_search(index),
            is_hovered: hover_index == Some(index),
            is_keyboard_focused: page_focus_page_index == Some(index),
            delete_hovered: hover_delete == Some(index),
            duplicate_hovered: hover_duplicate == Some(index),
            rename_hovered: hover_rename == Some(index),
        });
    }

    if let Some(hover_index) = hover_index
        && hover_index >= first_visible
        && hover_index < first_visible + visible
        && !is_dragging
    {
        let slot = hover_index - first_visible;
        let col = slot % cols;
        let row = slot / cols;
        let thumb_x = start_x + col as f64 * (layout.page_thumb_width + layout.page_thumb_gap);
        let thumb_y = start_y + row as f64 * row_stride;
        let page = &pages[hover_index];
        render_page_preview(PagePreviewArgs {
            ctx,
            frame: page,
            background: &board.spec.background,
            thumb_x,
            thumb_y,
            thumb_w: layout.page_thumb_width,
            thumb_h: layout.page_thumb_height,
            screen_width,
            screen_height,
            page_number: hover_index + 1,
        });
    }

    if let Some((edit_board, edit_page, buffer)) = input_state.board_picker_page_edit_state()
        && edit_board == board_index
        && edit_page >= first_visible
        && edit_page < first_visible + visible
    {
        let slot = edit_page - first_visible;
        let col = slot % cols;
        let row = slot / cols;
        let thumb_x = start_x + col as f64 * (layout.page_thumb_width + layout.page_thumb_gap);
        let thumb_y = start_y + row as f64 * row_stride;
        render_page_rename_overlay(
            ctx,
            thumb_x,
            thumb_y,
            layout.page_thumb_width,
            layout.page_thumb_height,
            layout.footer_font_size,
            buffer,
        );
    }

    // Render "Add page" card at the end of thumbnails when the end of the page list is visible.
    let end_visible = first_visible + visible >= page_count;
    if end_visible && visible < visible_slots {
        let add_card_slot = visible;
        let add_col = add_card_slot % cols;
        let add_row = add_card_slot / cols;
        let add_x = start_x + add_col as f64 * (layout.page_thumb_width + layout.page_thumb_gap);
        let add_y = start_y + add_row as f64 * row_stride;
        let add_hover = point_in_rect(
            pointer_x,
            pointer_y,
            add_x,
            add_y,
            layout.page_thumb_width,
            layout.page_thumb_height,
        );
        render_add_page_card(
            ctx,
            add_x,
            add_y,
            layout.page_thumb_width,
            layout.page_thumb_height,
            add_hover,
            false,
        );
    }

    render_sticky_add_button(ctx, layout, pointer_x, pointer_y);

    if page_count > visible {
        let first_label = first_visible + 1;
        let last_label = first_visible + visible;
        let hint = format!("Pages {first_label}-{last_label} of {page_count}");
        ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        ctx.set_font_size(layout.footer_font_size);
        let overflow_hover = input_state.board_picker_page_overflow_at(pointer_x, pointer_y);
        if overflow_hover {
            constants::set_color(ctx, TEXT_TERTIARY);
        } else {
            constants::set_color(ctx, TEXT_HINT);
        }
        let hint_y = layout.page_add_button_y - 4.0;
        ctx.move_to(start_x, hint_y);
        let _ = ctx.show_text(&hint);
        if overflow_hover && let Ok(extents) = ctx.text_extents(&hint) {
            ctx.set_line_width(1.0);
            ctx.move_to(start_x, hint_y + 2.0);
            ctx.line_to(start_x + extents.width(), hint_y + 2.0);
            let _ = ctx.stroke();
        }
    }
}

fn render_sticky_add_button(
    ctx: &cairo::Context,
    layout: BoardPickerLayout,
    pointer_x: i32,
    pointer_y: i32,
) {
    let hover = point_in_rect(
        pointer_x,
        pointer_y,
        layout.page_add_button_x,
        layout.page_add_button_y,
        layout.page_add_button_width,
        layout.page_add_button_height,
    );
    draw_rounded_rect(
        ctx,
        layout.page_add_button_x,
        layout.page_add_button_y,
        layout.page_add_button_width,
        layout.page_add_button_height,
        5.0,
    );
    if hover {
        constants::set_color(ctx, BG_HOVER);
    } else {
        ctx.set_source_rgba(0.16, 0.20, 0.28, 0.85);
    }
    let _ = ctx.fill_preserve();
    ctx.set_source_rgba(1.0, 1.0, 1.0, if hover { 0.28 } else { 0.16 });
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(layout.footer_font_size);
    constants::set_color(ctx, TEXT_SECONDARY);
    let label = "+ Add page";
    if let Ok(extents) = ctx.text_extents(label) {
        let text_x =
            layout.page_add_button_x + (layout.page_add_button_width - extents.width()) * 0.5;
        let text_y = layout.page_add_button_y
            + (layout.page_add_button_height + extents.height()) * 0.5
            - 1.0;
        ctx.move_to(text_x, text_y);
        let _ = ctx.show_text(label);
    }
}

fn point_in_rect(x: i32, y: i32, rx: f64, ry: f64, rw: f64, rh: f64) -> bool {
    let x = x as f64;
    let y = y as f64;
    x >= rx && x <= rx + rw && y >= ry && y <= ry + rh
}

fn render_page_rename_overlay(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    font_size: f64,
    text: &str,
) {
    let pad = 2.0;
    let max_font = (PAGE_NAME_HEIGHT - 4.0).max(9.0);
    let font_size = font_size.min(max_font);
    let input_h = PAGE_NAME_HEIGHT;
    let input_x = x + pad;
    let input_y = y + height + PAGE_NAME_PADDING;
    let input_w = (width - pad * 2.0).max(24.0);
    draw_rounded_rect(ctx, input_x, input_y, input_w, input_h, 4.0);
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.6);
    let _ = ctx.fill_preserve();
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.25);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(font_size);
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
    let text_x = input_x + 4.0;
    let text_y = input_y + input_h - 4.0;
    let _ = ctx.save();
    ctx.rectangle(input_x + 4.0, input_y, input_w - 8.0, input_h);
    ctx.clip();
    ctx.move_to(text_x, text_y);
    let _ = ctx.show_text(text);
    let _ = ctx.restore();

    if let Ok(extents) = ctx.text_extents(text) {
        let caret_x = (text_x + extents.x_advance()).min(input_x + input_w - 6.0);
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
        ctx.set_line_width(1.0);
        ctx.move_to(caret_x, input_y + 3.0);
        ctx.line_to(caret_x, input_y + input_h - 3.0);
        let _ = ctx.stroke();
    }
}
