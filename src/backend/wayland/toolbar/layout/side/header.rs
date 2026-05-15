use super::{HitKind, HitRegion, SideLayoutContext, ToolbarEvent, ToolbarLayoutSpec};
use crate::ui::toolbar::model::{
    SideHeaderModel, ToolbarControl, ToolbarControlKind, ToolbarSegmentedControl,
};

pub(super) fn push_header_hits(ctx: &SideLayoutContext<'_>, hits: &mut Vec<HitRegion>) {
    let header_model = SideHeaderModel::from_snapshot(ctx.snapshot);
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
        event: single_control_event(&header_model.drag),
        kind: HitKind::DragMoveSide,
        tooltip: header_model.drag.presentation.tooltip.as_string(),
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
        rect: (pin_x, btn_y, btn_size, btn_size),
        event: single_control_event(&header_model.pin),
        kind: HitKind::Click,
        tooltip: header_model.pin.presentation.tooltip.as_string(),
    });

    hits.push(HitRegion {
        rect: (close_x, btn_y, btn_size, btn_size),
        event: single_control_event(&header_model.close),
        kind: HitKind::Click,
        tooltip: header_model.close.presentation.tooltip.as_string(),
    });

    push_segment_hits(
        hits,
        &header_model.icon_mode,
        icons_x,
        icons_y,
        icons_w / 2.0,
        segment_h,
    );

    // ========== ROW 2: Simple/Full + More ==========
    let row2_y = ctx.spec.side_header_row2_y();
    let row2_h = ToolbarLayoutSpec::SIDE_HEADER_ROW2_HEIGHT;

    let segment_h = ToolbarLayoutSpec::SIDE_SEGMENT_HEIGHT;
    let segment_y = row2_y + (row2_h - segment_h) / 2.0;
    let layout_w = ToolbarLayoutSpec::SIDE_MODE_LAYOUT_WIDTH;
    let layout_x = ctx.x;
    let more_x = ctx.x + ctx.content_width - btn_size;
    let more_y = row2_y + (row2_h - btn_size) / 2.0;

    push_segment_hits(
        hits,
        &header_model.layout_mode,
        layout_x,
        segment_y,
        layout_w / 2.0,
        segment_h,
    );
    hits.push(HitRegion {
        rect: (more_x, more_y, btn_size, btn_size),
        event: single_control_event(&header_model.drawer_more),
        kind: HitKind::Click,
        tooltip: header_model.drawer_more.presentation.tooltip.as_string(),
    });

    // ========== ROW 3: Board chip ==========
    let row3_y = ctx.spec.side_header_row3_y();
    let row3_h = ToolbarLayoutSpec::SIDE_HEADER_ROW3_HEIGHT;
    let chip_h = ToolbarLayoutSpec::SIDE_BOARD_CHIP_HEIGHT;
    let chip_y = row3_y + (row3_h - chip_h) / 2.0;
    hits.push(HitRegion {
        rect: (ctx.x, chip_y, ctx.content_width, chip_h),
        event: single_control_event(&header_model.board_chip),
        kind: HitKind::Click,
        tooltip: header_model.board_chip.presentation.tooltip.as_string(),
    });
}

fn single_control_event(control: &ToolbarControl) -> ToolbarEvent {
    let ToolbarControlKind::Single(single) = &control.kind else {
        return ToolbarEvent::CloseSideToolbar;
    };
    single.activation.compatibility_event()
}

fn segmented_control(control: &ToolbarControl) -> Option<&ToolbarSegmentedControl> {
    match &control.kind {
        ToolbarControlKind::Segmented(segmented) => Some(segmented),
        ToolbarControlKind::Single(_) => None,
    }
}

fn push_segment_hits(
    hits: &mut Vec<HitRegion>,
    control: &ToolbarControl,
    x: f64,
    y: f64,
    segment_w: f64,
    segment_h: f64,
) {
    let Some(segmented) = segmented_control(control) else {
        return;
    };
    for (index, segment) in segmented.segments().iter().enumerate() {
        hits.push(HitRegion {
            rect: (x + segment_w * index as f64, y, segment_w, segment_h),
            event: segment.activation.compatibility_event(),
            kind: HitKind::Click,
            tooltip: segment.tooltip.as_string(),
        });
    }
}
