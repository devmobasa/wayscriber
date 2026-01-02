use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::ui::toolbar::ToolbarEvent;

use super::super::widgets::*;

pub(super) fn draw_board_section(layout: &mut SidePaletteLayout, y: &mut f64) {
    let ctx = layout.ctx;
    let snapshot = layout.snapshot;
    let hits = &mut layout.hits;
    let hover = layout.hover;
    let x = layout.x;
    let card_x = layout.card_x;
    let card_w = layout.card_w;
    let content_width = layout.content_width;
    let section_gap = layout.section_gap;

    let board_card_h = layout.spec.side_board_height(snapshot);
    draw_group_card(ctx, card_x, *y, card_w, board_card_h);
    draw_section_label(
        ctx,
        x,
        *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_Y,
        "Board",
    );

    let name_row_y = *y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let name_row_h = ToolbarLayoutSpec::SIDE_BOARD_NAME_ROW_HEIGHT;
    let rename_w = ToolbarLayoutSpec::SIDE_BOARD_NAME_BUTTON_WIDTH;
    let rename_x = x + content_width - rename_w;
    let dot_size = ToolbarLayoutSpec::SIDE_BOARD_COLOR_DOT_SIZE;
    let dot_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
    let dot_x = rename_x - dot_gap - dot_size;
    let dot_y = name_row_y + (name_row_h - dot_size) * 0.5;
    let rename_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, rename_x, name_row_y, rename_w, name_row_h))
        .unwrap_or(false);
    draw_button(
        ctx,
        rename_x,
        name_row_y,
        rename_w,
        name_row_h,
        true,
        rename_hover,
    );
    draw_label_center(ctx, rename_x, name_row_y, rename_w, name_row_h, "Rename");
    hits.push(HitRegion {
        rect: (rename_x, name_row_y, rename_w, name_row_h),
        event: ToolbarEvent::RenameBoard,
        kind: HitKind::Click,
        tooltip: Some("Rename board".to_string()),
    });

    let name_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
    let max_label_w = content_width - rename_w - dot_size - name_gap * 2.0;
    let name_label = board_label(snapshot);
    let display_label = truncate_label(&name_label, 24);
    draw_label_left(ctx, x, name_row_y, max_label_w, name_row_h, &display_label);

    if let Some(color) = snapshot.board_color {
        draw_swatch(ctx, dot_x, dot_y, dot_size, color, false);
        hits.push(HitRegion {
            rect: (dot_x, dot_y, dot_size, dot_size),
            event: ToolbarEvent::EditBoardColor,
            kind: HitKind::Click,
            tooltip: Some("Edit board color".to_string()),
        });
    } else {
        ctx.set_source_rgba(0.62, 0.68, 0.76, 0.7);
        draw_round_rect(ctx, dot_x, dot_y, dot_size, dot_size, 3.0);
        let _ = ctx.stroke();
        ctx.move_to(dot_x, dot_y);
        ctx.line_to(dot_x + dot_size, dot_y + dot_size);
        ctx.move_to(dot_x + dot_size, dot_y);
        ctx.line_to(dot_x, dot_y + dot_size);
        let _ = ctx.stroke();
    }

    *y += board_card_h + section_gap;
}

fn board_label(snapshot: &crate::ui::toolbar::ToolbarSnapshot) -> String {
    let name = snapshot.board_name.trim();
    if snapshot.board_count > 1 {
        if name.is_empty() {
            format!(
                "Board {}/{}",
                snapshot.board_index + 1,
                snapshot.board_count
            )
        } else {
            format!(
                "Board {}/{}: {}",
                snapshot.board_index + 1,
                snapshot.board_count,
                name
            )
        }
    } else if name.is_empty() {
        "Board".to_string()
    } else {
        format!("Board: {}", name)
    }
}

fn truncate_label(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value.to_string()
    } else {
        let mut truncated = value
            .chars()
            .take(max_chars.saturating_sub(3))
            .collect::<String>();
        truncated.push_str("...");
        truncated
    }
}
