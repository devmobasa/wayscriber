//! The style pill's arrow style chip and the menu it opens.
//!
//! The chip shows the next arrow's style drawn and named; the menu hangs
//! below the pill under the chip, built like the chrome island's layout
//! menu, one row per style with its drawn preview. Rows and geometry come
//! from the shared `model::arrow_style_*`, so the GTK menu lists the same
//! styles the same way.

use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::ui::toolbar::{ToolbarSnapshot, model};

use super::super::node::{ButtonStyle, Interaction, LabelSpec, WidgetKind, WidgetNode};
use super::super::popover;
use super::super::tree::WidgetTree;
use super::{TOP_LABEL_FONT_SIZE, TopStripPlan, bar_band_height};

const ANCHOR_GAP: f64 = 6.0;
const BOTTOM_MARGIN: f64 = 4.0;
const SURFACE_MARGIN: f64 = 4.0;
/// Space between a preview and the name after it.
const PREVIEW_GAP: f64 = 8.0;
const ROW_FONT_SIZE: f64 = 13.0;

/// The chip in its pill slot: a button body with the style's glyph and name
/// laid over it. It reads as pressed while the menu is open.
pub(super) fn arrow_style_chip_nodes(
    control: model::StylePillControl,
    snapshot: &ToolbarSnapshot,
    rect: (f64, f64, f64, f64),
) -> Vec<WidgetNode> {
    let (x, y, w, h) = rect;
    let id = control.id();
    let glyph_x = x + model::ARROW_STYLE_MENU_INSET;
    let label_x = glyph_x + model::ARROW_STYLE_CHIP_GLYPH_W + PREVIEW_GAP / 2.0;
    vec![
        WidgetNode::new(
            id.clone().into_owned(),
            rect,
            WidgetKind::TextButton {
                label: LabelSpec::new("", TOP_LABEL_FONT_SIZE, true),
                style: ButtonStyle::active(control.active(snapshot)),
            },
            Some(Interaction::click(
                control.click_event(snapshot),
                control.tooltip(snapshot),
            )),
        ),
        WidgetNode::decor(
            format!("{id}.glyph"),
            (
                glyph_x,
                y + (h - model::ARROW_STYLE_CHIP_GLYPH_H) / 2.0,
                model::ARROW_STYLE_CHIP_GLYPH_W,
                model::ARROW_STYLE_CHIP_GLYPH_H,
            ),
            WidgetKind::ArrowStylePreview {
                style: snapshot.arrow_style,
            },
        ),
        WidgetNode::decor(
            format!("{id}.label"),
            (label_x, y, x + w - label_x, h),
            WidgetKind::Label(LabelSpec::new(
                model::arrow_style_chip_label(snapshot.arrow_style),
                TOP_LABEL_FONT_SIZE,
                true,
            )),
        ),
    ]
}

/// The rect the menu hangs from: the chip's column across the pill's full
/// height, so the open menu clears the pill instead of covering it.
pub(super) fn arrow_style_anchor(chip_x: f64, pill_y: f64) -> (f64, f64, f64, f64) {
    (
        chip_x,
        pill_y,
        model::ARROW_STYLE_CHIP_W,
        ToolbarLayoutSpec::TOP_STYLE_PILL_H,
    )
}

/// How far the open menu reaches below the base bar, for sizing the surface.
/// Zero unless the menu is open and the pill carries its chip.
pub(super) fn arrow_style_menu_height_planned(
    snapshot: &ToolbarSnapshot,
    plan: &TopStripPlan,
) -> f64 {
    if !snapshot.arrow_style_menu_open
        || !model::StylePillSpec::build(snapshot, plan)
            .controls()
            .contains(&model::StylePillControl::ArrowStyleChip)
    {
        return 0.0;
    }

    let pill_bottom = bar_band_height(snapshot, plan)
        + ToolbarLayoutSpec::TOP_STYLE_PILL_GAP
        + ToolbarLayoutSpec::TOP_STYLE_PILL_H;
    let (_, menu_h) = model::arrow_style_menu_size();
    (pill_bottom + ANCHOR_GAP + menu_h + BOTTOM_MARGIN - super::base_bar_height(snapshot)).max(0.0)
}

/// Push the open menu under the chip's `anchor`.
pub(super) fn push_arrow_style_menu(
    tree: &mut WidgetTree,
    snapshot: &ToolbarSnapshot,
    anchor: Option<(f64, f64, f64, f64)>,
    bounds: (f64, f64),
) {
    let Some(anchor) = anchor.filter(|_| snapshot.arrow_style_menu_open) else {
        return;
    };

    let placement = popover::place_popover(popover::PopoverSpec {
        anchor,
        content: model::arrow_style_menu_size(),
        bounds,
        gap: ANCHOR_GAP,
        margin: SURFACE_MARGIN,
    });
    // Nothing under the menu may take a click meant for it.
    tree.suppress_interactions_covered_by(placement.rect);
    tree.push(WidgetNode::decor(
        "top.arrow-style.panel",
        placement.rect,
        WidgetKind::Popover {
            caret_x: placement.caret_x,
            caret_up: placement.side == popover::PopoverSide::Below,
        },
    ));

    let (px, py, _, _) = placement.rect;
    let row_x = px + model::ARROW_STYLE_MENU_PAD;
    let mut row_y = py + model::ARROW_STYLE_MENU_PAD;
    let row_w = model::ARROW_STYLE_MENU_ROW_W;
    let row_h = model::ARROW_STYLE_MENU_ROW_H;
    for entry in model::arrow_style_menu_entries(snapshot.arrow_style) {
        let id = entry.id();
        // The current style reads as the selected value.
        tree.push(WidgetNode::new(
            id.clone(),
            (row_x, row_y, row_w, row_h),
            WidgetKind::TextButton {
                label: LabelSpec::new("", ROW_FONT_SIZE, true),
                style: ButtonStyle::active(entry.current),
            },
            Some(Interaction::click(
                entry.event.clone(),
                Some(entry.tooltip()),
            )),
        ));
        let preview_x = row_x + model::ARROW_STYLE_MENU_INSET;
        tree.push(WidgetNode::decor(
            format!("{id}.preview"),
            (
                preview_x,
                row_y + (row_h - model::ARROW_STYLE_MENU_PREVIEW_H) / 2.0,
                model::ARROW_STYLE_MENU_PREVIEW_W,
                model::ARROW_STYLE_MENU_PREVIEW_H,
            ),
            WidgetKind::ArrowStylePreview { style: entry.style },
        ));
        let name_x = preview_x + model::ARROW_STYLE_MENU_PREVIEW_W + PREVIEW_GAP;
        tree.push(WidgetNode::decor(
            format!("{id}.name"),
            (name_x, row_y, row_x + row_w - name_x, row_h),
            WidgetKind::Label(LabelSpec::new(entry.label, ROW_FONT_SIZE, true)),
        ));
        row_y += row_h + model::ARROW_STYLE_MENU_ROW_GAP;
    }
}

#[cfg(test)]
mod tests;
