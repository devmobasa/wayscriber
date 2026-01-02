use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::draw::{BLACK, BLUE, Color, GREEN, ORANGE, PINK, RED, WHITE, YELLOW};
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
    let rename_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, rename_x, name_row_y, rename_w, name_row_h))
        .unwrap_or(false);
    draw_button(ctx, rename_x, name_row_y, rename_w, name_row_h, true, rename_hover);
    draw_label_center(ctx, rename_x, name_row_y, rename_w, name_row_h, "Rename");
    hits.push(HitRegion {
        rect: (rename_x, name_row_y, rename_w, name_row_h),
        event: ToolbarEvent::RenameBoard,
        kind: HitKind::Click,
        tooltip: Some("Rename board".to_string()),
    });

    let name_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
    let max_label_w = content_width - rename_w - name_gap;
    let name_label = board_label(snapshot);
    let display_label = truncate_label(&name_label, 24);
    draw_label_left(ctx, x, name_row_y, max_label_w, name_row_h, &display_label);

    let picker_y = name_row_y + name_row_h + name_gap;
    let picker_w = content_width;
    let picker_h = ToolbarLayoutSpec::SIDE_COLOR_PICKER_INPUT_HEIGHT;
    draw_color_picker(ctx, x, picker_y, picker_w, picker_h);

    let board_color_enabled = snapshot.board_color.is_some();
    if board_color_enabled {
        hits.push(HitRegion {
            rect: (x, picker_y, picker_w, picker_h),
            event: ToolbarEvent::SetBoardColor(snapshot.board_color.unwrap_or(Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            })),
            kind: HitKind::PickBoardColor {
                x,
                y: picker_y,
                w: picker_w,
                h: picker_h,
            },
            tooltip: Some("Pick board color".to_string()),
        });
    } else {
        ctx.set_source_rgba(0.0, 0.0, 0.0, 0.45);
        ctx.rectangle(x, picker_y, picker_w, picker_h);
        let _ = ctx.fill();
        draw_label_center(ctx, x, picker_y, picker_w, picker_h, "No background");
    }

    let basic_colors: &[(Color, &str)] = &[
        (RED, "Red"),
        (GREEN, "Green"),
        (BLUE, "Blue"),
        (YELLOW, "Yellow"),
        (WHITE, "White"),
        (BLACK, "Black"),
    ];
    let extended_colors: &[(Color, &str)] = &[
        (ORANGE, "Orange"),
        (PINK, "Pink"),
        (
            Color {
                r: 0.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            "Cyan",
        ),
        (
            Color {
                r: 0.6,
                g: 0.4,
                b: 0.8,
                a: 1.0,
            },
            "Purple",
        ),
        (
            Color {
                r: 0.4,
                g: 0.4,
                b: 0.4,
                a: 1.0,
            },
            "Gray",
        ),
    ];

    let swatch = ToolbarLayoutSpec::SIDE_COLOR_SWATCH;
    let swatch_gap = ToolbarLayoutSpec::SIDE_COLOR_SWATCH_GAP;
    let mut row_y = picker_y + picker_h + ToolbarLayoutSpec::SIDE_BOARD_SWATCH_TOP_GAP;
    let mut cx = x;
    for (color, _name) in basic_colors {
        draw_swatch(ctx, cx, row_y, swatch, *color, snapshot.board_color == Some(*color));
        if board_color_enabled {
            hits.push(HitRegion {
                rect: (cx, row_y, swatch, swatch),
                event: ToolbarEvent::SetBoardColor(*color),
                kind: HitKind::Click,
                tooltip: Some("Set board color".to_string()),
            });
        } else {
            ctx.set_source_rgba(0.0, 0.0, 0.0, 0.35);
            ctx.rectangle(cx, row_y, swatch, swatch);
            let _ = ctx.fill();
        }
        cx += swatch + swatch_gap;
    }

    if snapshot.show_more_colors {
        row_y += swatch + swatch_gap;
        cx = x;
        for (color, _name) in extended_colors {
            draw_swatch(ctx, cx, row_y, swatch, *color, snapshot.board_color == Some(*color));
            if board_color_enabled {
                hits.push(HitRegion {
                    rect: (cx, row_y, swatch, swatch),
                    event: ToolbarEvent::SetBoardColor(*color),
                    kind: HitKind::Click,
                    tooltip: Some("Set board color".to_string()),
                });
            } else {
                ctx.set_source_rgba(0.0, 0.0, 0.0, 0.35);
                ctx.rectangle(cx, row_y, swatch, swatch);
                let _ = ctx.fill();
            }
            cx += swatch + swatch_gap;
        }
    }

    *y += board_card_h + section_gap;
}

fn board_label(snapshot: &crate::ui::toolbar::ToolbarSnapshot) -> String {
    let name = snapshot.board_name.trim();
    if snapshot.board_count > 1 {
        if name.is_empty() {
            format!("Board {}/{}", snapshot.board_index + 1, snapshot.board_count)
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
