//! Style-pill level meters: a caption naming the setting, then one bar per
//! level, laid out abutting in a fixed-width row. The Pen feel panel lays the
//! same bars out across its wider content column.

use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::ui::toolbar::{ToolbarSnapshot, model};

use super::super::node::{Interaction, LabelSpec, WidgetKind, WidgetNode};
use super::CAPTION_FONT_SIZE;

/// Width one meter occupies, caption included, without the trailing gap.
///
/// The width planner walks the built tree, so this is what it budgets; the
/// GTK row asks for the same two slots.
pub(super) fn style_meter_width() -> f64 {
    ToolbarLayoutSpec::TOP_STYLE_CAPTION_W + ToolbarLayoutSpec::TOP_STYLE_METER_W
}

/// Push one meter's nodes starting at `x` on the row whose top is `y`,
/// returning the advance it consumed (see [`style_meter_width`]).
pub(super) fn push_style_meter(
    nodes: &mut Vec<WidgetNode>,
    control: model::StylePillControl,
    snapshot: &ToolbarSnapshot,
    x: f64,
    y: f64,
) -> f64 {
    let row_h = ToolbarLayoutSpec::TOP_STYLE_ROW_H;
    let caption_w = ToolbarLayoutSpec::TOP_STYLE_CAPTION_W;
    let id = control.id();

    if let Some(caption) = control.caption() {
        nodes.push(WidgetNode::decor(
            format!("{id}.caption"),
            (x, y, caption_w, row_h),
            WidgetKind::Label(LabelSpec::new(caption, CAPTION_FONT_SIZE, false).caption()),
        ));
    }
    nodes.extend(meter_bar_nodes(
        control.required_meter(snapshot),
        control.enabled(snapshot),
        (
            x + caption_w,
            y,
            ToolbarLayoutSpec::TOP_STYLE_METER_W,
            row_h,
        ),
    ));

    style_meter_width()
}

/// One interactive node per bar of `meter`, dividing `row` evenly.
///
/// Each bar's hit slot takes an equal share of the row, so the bars sit edge
/// to edge and a click between two bars still lands on one.
pub(super) fn meter_bar_nodes(
    meter: model::StylePillMeter,
    enabled: bool,
    row: (f64, f64, f64, f64),
) -> impl Iterator<Item = WidgetNode> {
    let (x, y, w, h) = row;
    let bar_w = w / meter.segments.len().max(1) as f64;

    meter
        .segments
        .into_iter()
        .enumerate()
        .map(move |(index, segment)| {
            WidgetNode::new(
                segment.id,
                (x + index as f64 * bar_w, y, bar_w, h),
                WidgetKind::MeterBar {
                    filled: segment.filled,
                    enabled,
                },
                enabled.then(|| Interaction::click(segment.event, Some(segment.tooltip))),
            )
        })
}
