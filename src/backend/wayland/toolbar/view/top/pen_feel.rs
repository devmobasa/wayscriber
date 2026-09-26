//! The style pill's Pen feel chip and the panel it opens
//! (`stroke_controls = "panel"`, the default).
//!
//! The chip is a fixed slot in the pill, budgeted by the width planner like
//! the font button. The panel hangs below the pill under the chip, built like
//! the chrome island's layout menu: its sections, wording, and geometry come
//! from the shared `model::pen_feel_*`, so the GTK panel shows the same thing.
//! Its meters are the inline meters' bars laid across the panel's column,
//! with the same events, so a click or a wheel step there works exactly like
//! one on the pill.

use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::ui::theme::toolbar::{FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL, FONT_SIZE_SMALL};
use crate::ui::toolbar::{ToolbarSnapshot, model};
use crate::ui_text::{UiTextEngine, UiTextStyle};

use super::super::node::{ButtonStyle, Interaction, LabelSpec, WidgetKind, WidgetNode};
use super::super::popover;
use super::super::tree::WidgetTree;
use super::meter::meter_bar_nodes;
use super::{CAPTION_FONT_SIZE, TOP_LABEL_FONT_SIZE, TopStripPlan, bar_band_height};

const ANCHOR_GAP: f64 = 6.0;
const BOTTOM_MARGIN: f64 = 4.0;
const SURFACE_MARGIN: f64 = 4.0;

/// The chip in its pill slot. It reads as pressed while the panel is open.
pub(super) fn pen_feel_chip_node(
    control: model::StylePillControl,
    snapshot: &ToolbarSnapshot,
    rect: (f64, f64, f64, f64),
) -> WidgetNode {
    WidgetNode::new(
        control.id().into_owned(),
        rect,
        WidgetKind::TextButton {
            label: LabelSpec::new(control.label(snapshot), TOP_LABEL_FONT_SIZE, true),
            style: ButtonStyle::active(control.active(snapshot)),
        },
        Some(Interaction::click(
            control.click_event(snapshot),
            control.tooltip(snapshot),
        )),
    )
}

/// The rect the panel hangs from: the chip's column across the pill's full
/// height, so the open panel clears the pill instead of covering it.
pub(super) fn pen_feel_anchor(chip_x: f64, pill_y: f64) -> (f64, f64, f64, f64) {
    (
        chip_x,
        pill_y,
        ToolbarLayoutSpec::TOP_STYLE_PEN_FEEL_W,
        ToolbarLayoutSpec::TOP_STYLE_PILL_H,
    )
}

/// How far the open panel reaches below the base bar, for sizing the
/// surface. Zero unless the panel is open and the pill carries its chip.
pub(super) fn pen_feel_height_planned(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> f64 {
    if !snapshot.pen_feel_open
        || !model::StylePillSpec::build(snapshot, plan)
            .controls()
            .contains(&model::StylePillControl::PenFeelChip)
    {
        return 0.0;
    }

    let pill_bottom = bar_band_height(snapshot, plan)
        + ToolbarLayoutSpec::TOP_STYLE_PILL_GAP
        + ToolbarLayoutSpec::TOP_STYLE_PILL_H;
    let (_, panel_h) = model::pen_feel_panel_size(snapshot);
    (pill_bottom + ANCHOR_GAP + panel_h + BOTTOM_MARGIN - super::base_bar_height(snapshot)).max(0.0)
}

/// Push the open panel under the chip's `anchor`.
pub(super) fn push_pen_feel_panel(
    engine: &UiTextEngine,
    tree: &mut WidgetTree,
    snapshot: &ToolbarSnapshot,
    anchor: Option<(f64, f64, f64, f64)>,
    bounds: (f64, f64),
) {
    let Some(anchor) = anchor.filter(|_| snapshot.pen_feel_open) else {
        return;
    };

    let placement = popover::place_popover(popover::PopoverSpec {
        anchor,
        content: model::pen_feel_panel_size(snapshot),
        bounds,
        gap: ANCHOR_GAP,
        margin: SURFACE_MARGIN,
    });
    // Nothing under the panel may take a click meant for it.
    tree.suppress_interactions_covered_by(placement.rect);
    tree.push(WidgetNode::decor(
        "top.feel.panel",
        placement.rect,
        WidgetKind::Popover {
            caret_x: placement.caret_x,
            caret_up: placement.side == popover::PopoverSide::Below,
        },
    ));

    let (px, py, _, _) = placement.rect;
    let x = px + model::PEN_FEEL_PAD;
    let w = model::PEN_FEEL_CONTENT_W;
    let mut y = py + model::PEN_FEEL_PAD;
    tree.push(WidgetNode::decor(
        "top.feel.title",
        (x, y, w, model::PEN_FEEL_TITLE_H),
        WidgetKind::Label(LabelSpec::new(model::PEN_FEEL_TITLE, FONT_SIZE_LABEL, true)),
    ));
    y += model::PEN_FEEL_TITLE_H;

    for section in model::pen_feel_sections(snapshot) {
        y += model::PEN_FEEL_SECTION_GAP;
        y = push_section(engine, tree, &section, (x, y, w));
    }
}

/// One setting's rows starting at `(x, y)` across width `w`: the header
/// (name left, level right), the meter, the preview where there is one, and
/// the level's hint. Returns the y below the section.
fn push_section(
    engine: &UiTextEngine,
    tree: &mut WidgetTree,
    section: &model::PenFeelSection,
    (x, mut y, w): (f64, f64, f64),
) -> f64 {
    let header_h = model::PEN_FEEL_HEADER_H;
    tree.push(WidgetNode::decor(
        section.id("label"),
        (x, y, w, header_h),
        WidgetKind::Label(
            LabelSpec::new(section.setting.name(), CAPTION_FONT_SIZE, false).caption(),
        ),
    ));
    let value_w = ink_right_edge(engine, section.level_name).min(w);
    tree.push(WidgetNode::decor(
        section.id("value"),
        (x + w - value_w, y, value_w, header_h),
        WidgetKind::Label(LabelSpec::new(section.level_name, CAPTION_FONT_SIZE, true)),
    ));
    y += header_h + model::PEN_FEEL_ROW_GAP;

    for node in meter_bar_nodes(
        section.meter.clone(),
        true,
        (x, y, w, model::PEN_FEEL_BARS_H),
    ) {
        tree.push(node);
    }
    y += model::PEN_FEEL_BARS_H + model::PEN_FEEL_ROW_GAP;

    if section.has_preview() {
        tree.push(WidgetNode::decor(
            section.id("preview"),
            (x, y, w, model::PEN_FEEL_PREVIEW_H),
            WidgetKind::SmoothingPreview {
                level: section.level,
            },
        ));
        y += model::PEN_FEEL_PREVIEW_H + model::PEN_FEEL_ROW_GAP;
    }

    tree.push(WidgetNode::decor(
        section.id("hint"),
        (x, y, w, model::PEN_FEEL_HINT_H),
        WidgetKind::Label(LabelSpec::new(section.hint, FONT_SIZE_SMALL, false).caption()),
    ));
    y + model::PEN_FEEL_HINT_H
}

/// Where a bold header label's ink ends when drawn from x = 0, so the level
/// name can sit flush with the right edge of the column.
fn ink_right_edge(engine: &UiTextEngine, text: &str) -> f64 {
    let style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: CAPTION_FONT_SIZE,
    };
    engine
        .measure(style, text, None)
        .map_or(0.0, |extents| extents.x_bearing() + extents.width())
}

#[cfg(test)]
mod tests;
