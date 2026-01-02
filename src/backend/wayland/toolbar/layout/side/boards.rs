use super::{HitKind, HitRegion, SideLayoutContext, ToolbarEvent, ToolbarLayoutSpec};
use crate::draw::{BLACK, BLUE, Color, GREEN, ORANGE, PINK, RED, WHITE, YELLOW};

pub(super) fn push_board_hits(ctx: &SideLayoutContext, y: f64, hits: &mut Vec<HitRegion>) -> f64 {
    let snapshot = ctx.snapshot;
    let x = ctx.x;
    let content_width = ctx.content_width;

    let name_row_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let name_row_h = ToolbarLayoutSpec::SIDE_BOARD_NAME_ROW_HEIGHT;
    let rename_w = ToolbarLayoutSpec::SIDE_BOARD_NAME_BUTTON_WIDTH;
    let rename_x = x + content_width - rename_w;
    hits.push(HitRegion {
        rect: (rename_x, name_row_y, rename_w, name_row_h),
        event: ToolbarEvent::RenameBoard,
        kind: HitKind::Click,
        tooltip: Some("Rename board".to_string()),
    });

    let name_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
    let picker_y = name_row_y + name_row_h + name_gap;
    let picker_w = content_width;
    let picker_h = ToolbarLayoutSpec::SIDE_COLOR_PICKER_INPUT_HEIGHT;
    if snapshot.board_color.is_some() {
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
    if snapshot.board_color.is_some() {
        for (color, _name) in basic_colors {
            hits.push(HitRegion {
                rect: (cx, row_y, swatch, swatch),
                event: ToolbarEvent::SetBoardColor(*color),
                kind: HitKind::Click,
                tooltip: Some("Set board color".to_string()),
            });
            cx += swatch + swatch_gap;
        }
        if snapshot.show_more_colors {
            row_y += swatch + swatch_gap;
            cx = x;
            for (color, _name) in extended_colors {
                hits.push(HitRegion {
                    rect: (cx, row_y, swatch, swatch),
                    event: ToolbarEvent::SetBoardColor(*color),
                    kind: HitKind::Click,
                    tooltip: Some("Set board color".to_string()),
                });
                cx += swatch + swatch_gap;
            }
        }
    }

    y + ctx.spec.side_board_height(snapshot) + ctx.section_gap
}
