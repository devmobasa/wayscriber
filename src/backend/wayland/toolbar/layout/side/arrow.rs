use super::{SideLayoutContext, ToolbarLayoutSpec};
use crate::ui::toolbar::ToolContext;

// Arrow hit regions are built in render to avoid duplicate registrations.
pub(super) fn advance_arrow_section(ctx: &SideLayoutContext<'_>, y: f64) -> f64 {
    if !ToolContext::from_snapshot(ctx.snapshot).show_arrow_labels {
        return y;
    }

    let card_h = if ctx.snapshot.arrow_label_enabled {
        ToolbarLayoutSpec::SIDE_TOGGLE_CARD_HEIGHT_WITH_RESET
    } else {
        ToolbarLayoutSpec::SIDE_TOGGLE_CARD_HEIGHT
    };
    y + card_h + ctx.section_gap
}
