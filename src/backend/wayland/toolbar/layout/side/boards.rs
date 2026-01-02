use super::{HitKind, HitRegion, SideLayoutContext, ToolbarEvent, ToolbarLayoutSpec};

pub(super) fn push_board_hits(ctx: &SideLayoutContext, y: f64, hits: &mut Vec<HitRegion>) -> f64 {
    let snapshot = ctx.snapshot;
    let x = ctx.x;
    let content_width = ctx.content_width;

    let name_row_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let name_row_h = ToolbarLayoutSpec::SIDE_BOARD_NAME_ROW_HEIGHT;
    let rename_w = ToolbarLayoutSpec::SIDE_BOARD_NAME_BUTTON_WIDTH;
    let rename_x = x + content_width - rename_w;
    let dot_size = ToolbarLayoutSpec::SIDE_BOARD_COLOR_DOT_SIZE;
    let dot_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
    let dot_x = rename_x - dot_gap - dot_size;
    let dot_y = name_row_y + (name_row_h - dot_size) * 0.5;
    hits.push(HitRegion {
        rect: (rename_x, name_row_y, rename_w, name_row_h),
        event: ToolbarEvent::RenameBoard,
        kind: HitKind::Click,
        tooltip: Some("Rename board".to_string()),
    });

    if snapshot.board_color.is_some() {
        hits.push(HitRegion {
            rect: (dot_x, dot_y, dot_size, dot_size),
            event: ToolbarEvent::EditBoardColor,
            kind: HitKind::Click,
            tooltip: Some("Edit board color".to_string()),
        });
    }

    y + ctx.spec.side_board_height(snapshot) + ctx.section_gap
}
