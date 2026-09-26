//! The chrome island's layout-preset menu.
//!
//! The layout button opens this small popover instead of cycling presets.
//! A cycling button moved out from under the pointer, because each preset
//! gives the strip a different width; a menu lets the user pick the preset
//! they want in one click. Rows come from the shared
//! `model::layout_menu_entries`, so both frontends list the same presets
//! with the same wording.

use crate::ui::toolbar::{ToolbarSnapshot, model};

use super::super::node::{ButtonStyle, Interaction, LabelSpec, WidgetKind, WidgetNode};
use super::super::popover;
use super::super::tree::WidgetTree;
use super::{ChromeMetrics, TopStripPlan, base_bar_height};

const NAME_FONT: f64 = 13.0;
const DESCRIPTION_FONT: f64 = 11.5;
const TEXT_INSET: f64 = 10.0;
const NAME_H: f64 = 18.0;
const DESCRIPTION_H: f64 = 14.0;
const ANCHOR_GAP: f64 = 6.0;
const BOTTOM_MARGIN: f64 = 4.0;
const SURFACE_MARGIN: f64 = 4.0;

fn panel_size() -> (f64, f64) {
    let rows = model::layout_menu_entries(crate::config::ToolbarLayoutMode::Regular).len() as f64;
    let width = model::LAYOUT_MENU_ROW_W + model::LAYOUT_MENU_PAD * 2.0;
    let height = model::LAYOUT_MENU_PAD * 2.0
        + rows * model::LAYOUT_MENU_ROW_H
        + (rows - 1.0).max(0.0) * model::LAYOUT_MENU_ROW_GAP;
    (width, height)
}

/// How far the open menu reaches below the base bar, for sizing the surface.
pub(super) fn layout_menu_height_planned(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> f64 {
    if !snapshot.layout_menu_open {
        return 0.0;
    }

    let metrics = ChromeMetrics::for_plan(plan);
    let base_height = base_bar_height(snapshot);
    let anchor_bottom = (base_height - metrics.size) / 2.0 + metrics.size;
    let (_, panel_h) = panel_size();
    (anchor_bottom + ANCHOR_GAP + panel_h + BOTTOM_MARGIN - base_height).max(0.0)
}

/// Push the open menu anchored below the layout button's `anchor` rect.
pub(super) fn push_layout_menu(
    tree: &mut WidgetTree,
    snapshot: &ToolbarSnapshot,
    anchor: Option<(f64, f64, f64, f64)>,
    bounds: (f64, f64),
) {
    let Some(anchor) = anchor.filter(|_| snapshot.layout_menu_open) else {
        return;
    };

    let placement = popover::place_popover(popover::PopoverSpec {
        anchor,
        content: panel_size(),
        bounds,
        gap: ANCHOR_GAP,
        margin: SURFACE_MARGIN,
    });
    let (px, py, _, _) = placement.rect;
    // The menu covers the style pill; nothing under it may take a click.
    tree.suppress_interactions_covered_by(placement.rect);
    tree.push(WidgetNode::decor(
        "top.layout.panel",
        placement.rect,
        WidgetKind::Popover {
            caret_x: placement.caret_x,
            caret_up: placement.side == popover::PopoverSide::Below,
        },
    ));

    let row_x = px + model::LAYOUT_MENU_PAD;
    let mut row_y = py + model::LAYOUT_MENU_PAD;
    for entry in model::layout_menu_entries(snapshot.layout_mode) {
        let key = model::layout_mode_label(entry.mode).to_ascii_lowercase();
        let row = (
            row_x,
            row_y,
            model::LAYOUT_MENU_ROW_W,
            model::LAYOUT_MENU_ROW_H,
        );
        // The current preset reads as the selected value.
        tree.push(WidgetNode::new(
            format!("top.layout.{key}"),
            row,
            WidgetKind::TextButton {
                label: LabelSpec::new("", NAME_FONT, true),
                style: ButtonStyle::active(entry.current),
            },
            Some(Interaction::click(
                entry.event.clone(),
                Some(format!("Switch to the {} layout", entry.label)),
            )),
        ));
        let text_x = row_x + TEXT_INSET;
        let text_w = model::LAYOUT_MENU_ROW_W - TEXT_INSET * 2.0;
        let name = if entry.current {
            format!("\u{2713} {}", entry.label)
        } else {
            entry.label.to_string()
        };
        tree.push(WidgetNode::decor(
            format!("top.layout.{key}.name"),
            (text_x, row_y + 3.0, text_w, NAME_H),
            WidgetKind::Label(LabelSpec::new(name, NAME_FONT, true)),
        ));
        tree.push(WidgetNode::decor(
            format!("top.layout.{key}.description"),
            (text_x, row_y + 3.0 + NAME_H, text_w, DESCRIPTION_H),
            WidgetKind::Label(LabelSpec::new(entry.description, DESCRIPTION_FONT, false)),
        ));
        row_y += model::LAYOUT_MENU_ROW_H + model::LAYOUT_MENU_ROW_GAP;
    }
}

#[cfg(test)]
mod tests {
    use crate::backend::wayland::toolbar::layout::top_size;
    use crate::backend::wayland::toolbar::view::{WidgetKind, WidgetTree};
    use crate::config::ToolbarLayoutMode;
    use crate::input::state::test_support::make_test_input_state;
    use crate::ui::toolbar::{ToolbarBindingHints, ToolbarEvent, ToolbarSnapshot, model};
    use crate::ui_text::UiTextEngine;

    use super::super::{build_top_view, top_extra_height, top_input_rects, top_natural_width};

    fn snapshot(mode: ToolbarLayoutMode, open: bool) -> ToolbarSnapshot {
        let state = make_test_input_state();
        let mut snapshot =
            ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default());
        snapshot.layout_mode = mode;
        snapshot.layout_menu_open = open;
        snapshot
    }

    fn tree_for(snapshot: &ToolbarSnapshot) -> WidgetTree {
        let engine = UiTextEngine::default();
        let (w, h) = top_size(&engine, snapshot);
        build_top_view(&engine, snapshot, w as f64, h as f64)
    }

    #[test]
    fn open_menu_lists_every_preset_and_marks_the_current_one() {
        for mode in ToolbarLayoutMode::ALL {
            let tree = tree_for(&snapshot(mode, true));
            assert!(tree.node_by_id(&"top.layout.panel".into()).is_some());

            for other in ToolbarLayoutMode::ALL {
                let key = model::layout_mode_label(other).to_ascii_lowercase();
                let row = tree
                    .node_by_id(&format!("top.layout.{key}").into())
                    .unwrap_or_else(|| panic!("{other:?} row"));
                assert_eq!(
                    row.interact.as_ref().map(|interaction| &interaction.event),
                    Some(&ToolbarEvent::SetToolbarLayoutMode(other))
                );
                let WidgetKind::TextButton { style, .. } = &row.kind else {
                    panic!("row is a button");
                };
                assert_eq!(style.active, other == mode, "{other:?} under {mode:?}");
                assert!(
                    tree.node_by_id(&format!("top.layout.{key}.description").into())
                        .is_some()
                );
            }
        }
    }

    #[test]
    fn closed_menu_renders_nothing() {
        let tree = tree_for(&snapshot(ToolbarLayoutMode::Regular, false));

        assert!(
            tree.nodes()
                .iter()
                .all(|node| !node.id.as_str().starts_with("top.layout."))
        );
    }

    /// The open menu grows the surface and its input region, but never the
    /// strip's width, so the layout button stays put while the menu opens.
    #[test]
    fn open_menu_grows_height_and_input_but_not_width() {
        let engine = UiTextEngine::default();
        let closed = snapshot(ToolbarLayoutMode::Simple, false);
        let open = snapshot(ToolbarLayoutMode::Simple, true);

        assert_eq!(
            top_natural_width(&engine, &open, 58.0),
            top_natural_width(&engine, &closed, 58.0)
        );
        assert!(top_extra_height(&engine, &open) >= top_extra_height(&engine, &closed));

        let (w, h) = top_size(&engine, &open);
        let tree = build_top_view(&engine, &open, w as f64, h as f64);
        let panel = tree
            .node_by_id(&"top.layout.panel".into())
            .expect("panel")
            .rect;
        assert!(panel.1 + panel.3 <= h as f64, "the panel fits the surface");
        let rects = top_input_rects(&engine, &open, w as f64, h as f64).expect("rects");
        assert!(
            rects.iter().any(|rect| rect.0 <= panel.0
                && rect.1 <= panel.1
                && rect.0 + rect.2 >= panel.0 + panel.2
                && rect.1 + rect.3 >= panel.1 + panel.3),
            "the whole panel accepts input: {panel:?} in {rects:?}"
        );
    }
}
