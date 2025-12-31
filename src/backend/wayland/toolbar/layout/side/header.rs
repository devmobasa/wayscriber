use super::{HitKind, HitRegion, SideLayoutContext, ToolbarEvent, ToolbarLayoutMode, ToolbarLayoutSpec};

pub(super) fn push_header_hits(ctx: &SideLayoutContext<'_>, hits: &mut Vec<HitRegion>) {
    let (pin_x, close_x, header_y) = ctx.spec.side_header_button_positions(ctx.width);
    let header_btn = ToolbarLayoutSpec::SIDE_HEADER_BUTTON_SIZE;
    let icons_w = ToolbarLayoutSpec::SIDE_HEADER_TOGGLE_WIDTH;
    hits.push(HitRegion {
        rect: (ctx.x, header_y, icons_w, header_btn),
        event: ToolbarEvent::ToggleIconMode(!ctx.snapshot.use_icons),
        kind: HitKind::Click,
        tooltip: None,
    });

    let mode_w = ToolbarLayoutSpec::SIDE_HEADER_MODE_WIDTH;
    let mode_x = ctx.x + icons_w + ToolbarLayoutSpec::SIDE_HEADER_MODE_GAP;
    let mode_tooltip = format!(
        "Mode: S/R/A = {}/{}/{}",
        ToolbarLayoutMode::Simple.label(),
        ToolbarLayoutMode::Regular.label(),
        ToolbarLayoutMode::Advanced.label(),
    );
    hits.push(HitRegion {
        rect: (mode_x, header_y, mode_w, header_btn),
        event: ToolbarEvent::SetToolbarLayoutMode(ctx.snapshot.layout_mode.next()),
        kind: HitKind::Click,
        tooltip: Some(mode_tooltip),
    });

    hits.push(HitRegion {
        rect: (close_x, header_y, header_btn, header_btn),
        event: ToolbarEvent::CloseSideToolbar,
        kind: HitKind::Click,
        tooltip: Some("Close".to_string()),
    });

    hits.push(HitRegion {
        rect: (pin_x, header_y, header_btn, header_btn),
        event: ToolbarEvent::PinSideToolbar(!ctx.snapshot.side_pinned),
        kind: HitKind::Click,
        tooltip: Some(if ctx.snapshot.side_pinned {
            "Unpin".to_string()
        } else {
            "Pin".to_string()
        }),
    });
}
