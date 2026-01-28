use super::{
    HitKind, HitRegion, SideLayoutContext, ToolbarEvent, ToolbarLayoutMode, ToolbarLayoutSpec,
};

pub(super) fn push_header_hits(ctx: &SideLayoutContext<'_>, hits: &mut Vec<HitRegion>) {
    let btn_size = ToolbarLayoutSpec::SIDE_HEADER_BUTTON_SIZE;
    let btn_gap = ToolbarLayoutSpec::SIDE_HEADER_BUTTON_GAP;
    let btn_margin = ToolbarLayoutSpec::SIDE_HEADER_BUTTON_MARGIN_RIGHT;

    // ========== ROW 1: Drag + Ico/Txt (center) + Pin/Close ==========
    let row1_y = ToolbarLayoutSpec::SIDE_TOP_PADDING;
    let row1_h = ToolbarLayoutSpec::SIDE_HEADER_ROW1_HEIGHT;

    // Drag handle
    let drag_size = ToolbarLayoutSpec::SIDE_HEADER_DRAG_SIZE;
    let drag_y = row1_y + (row1_h - drag_size) / 2.0;
    hits.push(HitRegion {
        rect: (ctx.x, drag_y, drag_size, drag_size),
        event: ToolbarEvent::MoveSideToolbar { x: 0.0, y: 0.0 },
        kind: HitKind::DragMoveSide,
        tooltip: Some("Drag toolbar".to_string()),
    });

    // Utility buttons: Pin, Close
    let btn_y = row1_y + (row1_h - btn_size) / 2.0;
    let close_x = ctx.width - btn_margin - btn_size;
    let pin_x = close_x - btn_size - btn_gap;

    let segment_h = ToolbarLayoutSpec::SIDE_SEGMENT_HEIGHT;
    let icons_w = ToolbarLayoutSpec::SIDE_MODE_ICONS_WIDTH;
    let center_start = ctx.x + drag_size + 8.0;
    let center_end = pin_x - 8.0;
    let icons_x = center_start + (center_end - center_start - icons_w) / 2.0;
    let icons_y = row1_y + (row1_h - segment_h) / 2.0;

    hits.push(HitRegion {
        rect: (icons_x, icons_y, icons_w / 2.0, segment_h),
        event: ToolbarEvent::ToggleIconMode(true),
        kind: HitKind::Click,
        tooltip: Some("Icons mode".to_string()),
    });
    hits.push(HitRegion {
        rect: (icons_x + icons_w / 2.0, icons_y, icons_w / 2.0, segment_h),
        event: ToolbarEvent::ToggleIconMode(false),
        kind: HitKind::Click,
        tooltip: Some("Text mode".to_string()),
    });

    hits.push(HitRegion {
        rect: (pin_x, btn_y, btn_size, btn_size),
        event: ToolbarEvent::PinSideToolbar(!ctx.snapshot.side_pinned),
        kind: HitKind::Click,
        tooltip: Some(if ctx.snapshot.side_pinned {
            "Unpin".to_string()
        } else {
            "Pin".to_string()
        }),
    });

    hits.push(HitRegion {
        rect: (close_x, btn_y, btn_size, btn_size),
        event: ToolbarEvent::CloseSideToolbar,
        kind: HitKind::Click,
        tooltip: Some("Close".to_string()),
    });

    // ========== ROW 2: Simple/Full + More ==========
    let row2_y = ctx.spec.side_header_row2_y();
    let row2_h = ToolbarLayoutSpec::SIDE_HEADER_ROW2_HEIGHT;

    let segment_h = ToolbarLayoutSpec::SIDE_SEGMENT_HEIGHT;
    let segment_y = row2_y + (row2_h - segment_h) / 2.0;
    let layout_w = ToolbarLayoutSpec::SIDE_MODE_LAYOUT_WIDTH;
    let layout_x = ctx.x;
    let more_x = ctx.x + ctx.content_width - btn_size;
    let more_y = row2_y + (row2_h - btn_size) / 2.0;

    let full_mode = if ctx.snapshot.layout_mode == ToolbarLayoutMode::Advanced {
        ToolbarLayoutMode::Advanced
    } else {
        ToolbarLayoutMode::Regular
    };
    hits.push(HitRegion {
        rect: (layout_x, segment_y, layout_w / 2.0, segment_h),
        event: ToolbarEvent::SetToolbarLayoutMode(ToolbarLayoutMode::Simple),
        kind: HitKind::Click,
        tooltip: Some("Simple mode".to_string()),
    });
    hits.push(HitRegion {
        rect: (
            layout_x + layout_w / 2.0,
            segment_y,
            layout_w / 2.0,
            segment_h,
        ),
        event: ToolbarEvent::SetToolbarLayoutMode(full_mode),
        kind: HitKind::Click,
        tooltip: Some("Full mode".to_string()),
    });
    hits.push(HitRegion {
        rect: (more_x, more_y, btn_size, btn_size),
        event: ToolbarEvent::ToggleDrawer(!ctx.snapshot.drawer_open),
        kind: HitKind::Click,
        tooltip: Some("More options".to_string()),
    });

    // ========== ROW 3: Board chip ==========
    let row3_y = ctx.spec.side_header_row3_y();
    let row3_h = ToolbarLayoutSpec::SIDE_HEADER_ROW3_HEIGHT;
    let chip_h = ToolbarLayoutSpec::SIDE_BOARD_CHIP_HEIGHT;
    let chip_y = row3_y + (row3_h - chip_h) / 2.0;
    hits.push(HitRegion {
        rect: (ctx.x, chip_y, ctx.content_width, chip_h),
        event: ToolbarEvent::ToggleBoardPicker,
        kind: HitKind::Click,
        tooltip: Some("Boards".to_string()),
    });
}
