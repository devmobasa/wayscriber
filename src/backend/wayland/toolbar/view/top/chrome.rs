//! The right-aligned chrome island: layout menu, About, pin, minimize, and
//! exit.

use crate::ui::toolbar::{ToolbarSnapshot, model};

use super::super::node::{Interaction, WidgetKind, WidgetNode};
use super::super::tree::WidgetTree;
use super::build::control_button_node;
use super::{
    ChromeMetrics, TopStripPlan, bar_band_height, base_bar_height, planned_island_metrics,
};

/// Push the chrome island pill and its buttons against the surface's right
/// edge. Returns the layout button's rect, which anchors the layout menu.
pub(super) fn push_chrome_island(
    tree: &mut WidgetTree,
    snapshot: &ToolbarSnapshot,
    spec: &model::TopToolbarSpec,
    plan: &TopStripPlan,
    width: f64,
) -> Option<(f64, f64, f64, f64)> {
    let metrics = ChromeMetrics::for_plan(plan);
    let (_, island_pad) = planned_island_metrics(plan);
    let chrome_y = (base_bar_height(snapshot) - metrics.size) / 2.0;
    let chrome_count = spec.chrome().len();
    let mut chrome_x = width - metrics.margin_right - metrics.block_width(chrome_count);
    if chrome_count > 0 {
        let pill_left = chrome_x - island_pad;
        tree.push(WidgetNode::decor(
            "top.island.chrome",
            (
                pill_left,
                0.0,
                width - pill_left,
                bar_band_height(snapshot, plan),
            ),
            WidgetKind::Panel,
        ));
    }

    let mut layout_anchor = None;
    for control in spec.chrome().iter().copied() {
        let rect = (chrome_x, chrome_y, metrics.size, metrics.size);
        chrome_x += metrics.size + metrics.gap;
        if control == model::TopToolbarControl::LayoutMode {
            layout_anchor = Some(rect);
        }
        tree.push(chrome_node(snapshot, control, rect));
    }
    layout_anchor
}

fn chrome_node(
    snapshot: &ToolbarSnapshot,
    control: model::TopToolbarControl,
    rect: (f64, f64, f64, f64),
) -> WidgetNode {
    let kind = match control {
        // About, Exit, and the layout menu are ordinary icon buttons in chrome
        // styling with a neutral hover; pin and minimize keep their bespoke
        // glyph widgets.
        model::TopToolbarControl::About
        | model::TopToolbarControl::Exit
        | model::TopToolbarControl::LayoutMode => {
            return control_button_node(
                snapshot,
                control,
                control.id().render_id().into_owned(),
                rect,
                true,
            );
        }
        model::TopToolbarControl::Pin => WidgetKind::PinButton {
            pinned: control.active(snapshot),
        },
        model::TopToolbarControl::Minimize => WidgetKind::MinimizeButton,
        _ => unreachable!("non-chrome control in chrome specification"),
    };
    WidgetNode::new(
        control.id().render_id().into_owned(),
        rect,
        kind,
        Some(Interaction::click(
            control.event(snapshot),
            Some(control.tooltip(snapshot)),
        )),
    )
}
