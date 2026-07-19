//! Top-strip tree construction and expanded-surface geometry.
//!
//! This module owns the implementation behind `top`'s stable planning,
//! building, sizing, and input-region interface.

use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::config::{Action, action_label, action_short_label, toolbar_item_ids as ids};
use crate::input::Tool;
use crate::toolbar_icons;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot, model};

use super::super::node::{
    ButtonStyle, IconFn, Interaction, LabelSpec, ShortcutBadgePlacement, WidgetKind, WidgetNode,
};
use super::super::tree::WidgetTree;
use super::{
    MINI_LABEL_FONT_SIZE, TOP_CHIP_SIZE, TOP_COMPACT_CHROME, TOP_COMPACT_GAP,
    TOP_COMPACT_MARGIN_RIGHT, TOP_DIVIDER_SPAN, TOP_LABEL_FONT_SIZE, TOP_SWATCH_GAP,
    TOP_SWATCH_SIZE, TopStripPlan, bar_band_height, base_bar_height, plan_top_strip,
    planned_button_size, planned_gap, planned_island_metrics, planned_use_icons,
};

pub(super) fn build_top_view_planned(
    snapshot: &ToolbarSnapshot,
    plan: &TopStripPlan,
    width: f64,
    height: f64,
) -> WidgetTree {
    let spec = model::TopToolbarSpec::build(snapshot, plan);
    if snapshot.top_minimized {
        return build_top_minimized_tab(snapshot, &spec, width, height);
    }
    if snapshot.top_micro_active() {
        return build_top_micro_chip(snapshot, &spec, width, height);
    }
    let gap = planned_gap(plan);
    let (island_gap, island_pad) = planned_island_metrics(plan);
    let band_h = bar_band_height(snapshot, plan);
    let mut tree = WidgetTree::new((width, height));

    let mut x = if plan.compact {
        4.0
    } else {
        ToolbarLayoutSpec::TOP_START_X
    };
    let is_simple = snapshot.layout_mode == crate::config::ToolbarLayoutMode::Simple;
    let use_icons = planned_use_icons(snapshot, plan);
    let (btn_w, btn_h) = planned_button_size(snapshot, plan);
    let base_height = base_bar_height(snapshot);
    let y = (base_height - btn_h) / 2.0;

    // Strip content is collected first so the island pill cards — whose
    // extents are only known after the walk — can be pushed underneath it.
    let mut strip_nodes: Vec<WidgetNode> = Vec::new();
    // (island, pill left edge, content right edge — filled when it closes).
    let mut islands: Vec<(model::TopToolbarIsland, f64, f64)> = Vec::new();

    let push_divider = |nodes: &mut Vec<WidgetNode>, x: &mut f64, key: &str| {
        let divider_span = if plan.compact { 3.0 } else { TOP_DIVIDER_SPAN };
        nodes.push(WidgetNode::decor(
            format!("top.divider.{key}"),
            (*x + (divider_span - 1.0) / 2.0, y + 6.0, 1.0, btn_h - 12.0),
            WidgetKind::Divider { vertical: true },
        ));
        *x += divider_span + gap;
    };

    let mut picker_anchor: Option<(f64, f64, f64, f64)> = None;
    let mut overflow_anchor: Option<(f64, f64, f64, f64)> = None;
    let contextual_ring = spec.contextual().first().copied();
    let swatches_have_badges = spec.strip().iter().any(|node| {
        matches!(
            node,
            model::TopToolbarNode::Control(model::TopToolbarControl::QuickColor(index))
                if snapshot.binding_hints.quick_color_badge(*index).is_some()
        )
    });
    let swatch_y = if swatches_have_badges {
        y + (btn_h - (10.0 + 1.0 + TOP_SWATCH_SIZE)) / 2.0 + 11.0
    } else {
        y + (btn_h - TOP_SWATCH_SIZE) / 2.0
    };

    for node in spec.strip() {
        // Island transitions close the current pill (its content right edge
        // is now known) and jump `x` across the inter-island gap plus the
        // next pill's leading padding.
        let island = node.island();
        if islands.last().map(|entry| entry.0) != Some(island) {
            if let Some(last) = islands.last_mut() {
                last.2 = x - gap;
                let pill_right = last.2 + island_pad;
                x = pill_right + island_gap + island_pad;
                islands.push((island, pill_right + island_gap, 0.0));
            } else {
                islands.push((island, 0.0, 0.0));
            }
        }
        match *node {
            model::TopToolbarNode::Divider(divider) => {
                let key = divider.id().trim_start_matches("top.divider.");
                push_divider(&mut strip_nodes, &mut x, key);
            }
            model::TopToolbarNode::Control(control) => match control {
                model::TopToolbarControl::DragHandle => {
                    let size = ToolbarLayoutSpec::TOP_HANDLE_SIZE;
                    strip_nodes.push(WidgetNode::new(
                        control.id().render_id().into_owned(),
                        (x, ToolbarLayoutSpec::TOP_HANDLE_Y, size, size),
                        WidgetKind::DragHandle,
                        Some(Interaction {
                            event: control.event(snapshot),
                            kind: HitKind::DragMoveTop,
                            tooltip: Some(control.tooltip(snapshot)),
                        }),
                    ));
                    x += size + gap;
                }
                model::TopToolbarControl::Tool(_)
                | model::TopToolbarControl::Utility(_)
                | model::TopToolbarControl::ShapePicker
                | model::TopToolbarControl::Undo
                | model::TopToolbarControl::Redo => {
                    let rect = (x, y, btn_w, btn_h);
                    strip_nodes.push(control_button_node(
                        snapshot,
                        control,
                        control.id().render_id().into_owned(),
                        rect,
                        use_icons,
                    ));
                    if control == model::TopToolbarControl::ShapePicker {
                        picker_anchor = Some(rect);
                    }
                    if matches!(
                        control,
                        model::TopToolbarControl::Utility(model::TopToolbarUtility::Highlight)
                    ) && let Some(ring) = contextual_ring
                    {
                        let ring_y = y + btn_h + ToolbarLayoutSpec::TOP_ICON_FILL_OFFSET;
                        strip_nodes.push(WidgetNode::new(
                            ring.id().render_id().into_owned(),
                            (x, ring_y, btn_w, ToolbarLayoutSpec::TOP_ICON_FILL_HEIGHT),
                            WidgetKind::MiniCheckbox {
                                checked: ring.active(snapshot),
                                label: LabelSpec::new(
                                    ring.label(snapshot),
                                    MINI_LABEL_FONT_SIZE,
                                    false,
                                ),
                            },
                            Some(Interaction::click(
                                ring.event(snapshot),
                                Some(ring.tooltip(snapshot)),
                            )),
                        ));
                    }
                    x += btn_w + gap;
                }
                model::TopToolbarControl::Overflow => {
                    // The ⋯ toggle anchors the overflow menu from the
                    // history island; its glyph stays an icon even in
                    // text-label mode.
                    let rect = (x, y, btn_w, btn_h);
                    overflow_anchor = Some(rect);
                    strip_nodes.push(control_button_node(
                        snapshot,
                        control,
                        control.id().render_id().into_owned(),
                        rect,
                        true,
                    ));
                    x += btn_w + gap;
                }
                model::TopToolbarControl::QuickColor(index) => {
                    let entry = &snapshot.quick_colors.rendered_entries()[index];
                    let badge = control.shortcut_badge(snapshot);
                    strip_nodes.push(
                        WidgetNode::new(
                            control.id().render_id().into_owned(),
                            (x, swatch_y, TOP_SWATCH_SIZE, TOP_SWATCH_SIZE),
                            WidgetKind::Swatch {
                                color: (entry.color.r, entry.color.g, entry.color.b, entry.color.a),
                                selected: control.active(snapshot),
                            },
                            Some(Interaction::click(
                                control.event(snapshot),
                                Some(control.tooltip(snapshot)),
                            )),
                        )
                        .with_shortcut_badge(badge.as_deref(), ShortcutBadgePlacement::Above),
                    );
                    x += TOP_SWATCH_SIZE + TOP_SWATCH_GAP;
                }
                model::TopToolbarControl::CurrentColor => {
                    let chip_y = y + (btn_h - TOP_CHIP_SIZE) / 2.0;
                    x += gap - TOP_SWATCH_GAP;
                    strip_nodes.push(WidgetNode::new(
                        control.id().render_id().into_owned(),
                        (x, chip_y, TOP_CHIP_SIZE, TOP_CHIP_SIZE),
                        WidgetKind::Swatch {
                            color: (
                                snapshot.color.r,
                                snapshot.color.g,
                                snapshot.color.b,
                                snapshot.color.a,
                            ),
                            selected: control.active(snapshot),
                        },
                        Some(Interaction::click(
                            control.event(snapshot),
                            Some(control.tooltip(snapshot)),
                        )),
                    ));
                    x += TOP_CHIP_SIZE + gap;
                }
                model::TopToolbarControl::Restore
                | model::TopToolbarControl::MicroChip
                | model::TopToolbarControl::Pin
                | model::TopToolbarControl::Minimize
                | model::TopToolbarControl::ClearCanvas
                | model::TopToolbarControl::HighlightRing => {
                    unreachable!("control belongs outside the main strip")
                }
            },
        }
    }
    if let Some(last) = islands.last_mut() {
        last.2 = x - gap;
    }

    // The pill cards paint under their content: panel background, hairline,
    // panel radius. They are group-paintable decor so a later phase can wrap
    // an island in push_group/paint_with_alpha without re-clustering.
    for (island, pill_left, content_right) in &islands {
        tree.push(WidgetNode::decor(
            format!("top.island.{}", island.key()),
            (
                *pill_left,
                0.0,
                content_right + island_pad - pill_left,
                band_h,
            ),
            WidgetKind::Panel,
        ));
    }
    for node in strip_nodes {
        tree.push(node);
    }

    // --- Shapes popover: the grid plus per-tool options ------------------------
    if snapshot.shape_picker_open
        && let Some(anchor) = picker_anchor
    {
        let rows = model::visible_shape_picker_rows(snapshot, is_simple);
        let option_rows = shape_option_rows(snapshot);
        let max_row_len = rows.iter().map(Vec::len).max().unwrap_or(0);
        if max_row_len > 0 || !option_rows.is_empty() {
            let pad = ToolbarLayoutSpec::TOP_POPOVER_PAD;
            let option_h = ToolbarLayoutSpec::TOP_OPTION_ROW_H;
            let grid_w = max_row_len as f64 * (btn_w + gap) - gap;
            let content_w = grid_w.max(160.0) + pad * 2.0;
            let grid_h = rows.len() as f64 * (btn_h + gap) - gap;
            let content_h = pad * 2.0
                + grid_h.max(0.0)
                + option_rows.len() as f64 * (option_h + gap)
                + if rows.is_empty() { -gap } else { 0.0 };
            let popover_anchor = popover_anchor_below_ring(anchor, snapshot, plan, y, btn_h);
            let placement =
                super::super::popover::place_popover(super::super::popover::PopoverSpec {
                    anchor: popover_anchor,
                    content: (content_w, content_h),
                    bounds: (width, height),
                    gap: ToolbarLayoutSpec::TOP_SHAPE_ROW_GAP,
                    margin: 4.0,
                });
            let (px, py, pw, _ph) = placement.rect;
            tree.push(WidgetNode::decor(
                "top.shapes.panel",
                placement.rect,
                WidgetKind::Popover {
                    caret_x: placement.caret_x,
                    caret_up: placement.side == super::super::popover::PopoverSide::Below,
                },
            ));
            let mut row_y = py + pad;
            for row in rows {
                let mut shape_x = px + pad;
                for tool in row {
                    if !model::tool_visible(snapshot, tool) {
                        continue;
                    }
                    tree.push(tool_button_node(
                        snapshot,
                        tool,
                        format!(
                            "top.picker.{}",
                            model::toolbar_item_id_for_tool(tool).as_str()
                        ),
                        (shape_x, row_y, btn_w, btn_h),
                        use_icons,
                    ));
                    shape_x += btn_w + gap;
                }
                row_y += btn_h + gap;
            }
            for option_row in option_rows {
                push_option_row(
                    &mut tree,
                    snapshot,
                    option_row,
                    (px + pad, row_y, pw - pad * 2.0, option_h),
                );
                row_y += option_h + gap;
            }
        }
    }

    // --- Right-aligned chrome island --------------------------------------------
    let chrome_size = if plan.compact {
        TOP_COMPACT_CHROME
    } else {
        ToolbarLayoutSpec::TOP_PIN_BUTTON_SIZE
    };
    let chrome_y = (base_height - chrome_size) / 2.0;
    let chrome_gap = if plan.compact {
        TOP_COMPACT_GAP
    } else {
        ToolbarLayoutSpec::TOP_PIN_BUTTON_GAP
    };
    let chrome_margin_right = if plan.compact {
        TOP_COMPACT_MARGIN_RIGHT
    } else {
        ToolbarLayoutSpec::TOP_PIN_BUTTON_MARGIN_RIGHT
    };
    let chrome_count = spec.chrome().len();
    let chrome_width =
        chrome_size * chrome_count as f64 + chrome_gap * chrome_count.saturating_sub(1) as f64;
    let mut chrome_x = width - chrome_margin_right - chrome_width;
    if chrome_count > 0 {
        let pill_left = chrome_x - island_pad;
        tree.push(WidgetNode::decor(
            "top.island.chrome",
            (pill_left, 0.0, width - pill_left, band_h),
            WidgetKind::Panel,
        ));
    }
    for control in spec.chrome().iter().copied() {
        let rect = (chrome_x, chrome_y, chrome_size, chrome_size);
        let kind = match control {
            model::TopToolbarControl::Pin => WidgetKind::PinButton {
                pinned: control.active(snapshot),
            },
            model::TopToolbarControl::Minimize => WidgetKind::MinimizeButton,
            _ => unreachable!("non-chrome control in chrome specification"),
        };
        tree.push(WidgetNode::new(
            control.id().render_id().into_owned(),
            rect,
            kind,
            Some(Interaction::click(
                control.event(snapshot),
                Some(control.tooltip(snapshot)),
            )),
        ));
        chrome_x += chrome_size + chrome_gap;
    }

    // --- Overflow popover: the width-dropped items ---------------------------
    if snapshot.top_overflow_open
        && let Some(anchor) = overflow_anchor
    {
        let dropped_count = spec.overflow().len();
        if dropped_count > 0 {
            let cols = dropped_count.min(5);
            let rows = dropped_count.div_ceil(cols);
            let pad = 8.0;
            let content_w = cols as f64 * btn_w + (cols as f64 - 1.0) * gap + pad * 2.0;
            let content_h = rows as f64 * btn_h + (rows as f64 - 1.0) * gap + pad * 2.0;
            let popover_anchor = popover_anchor_below_ring(anchor, snapshot, plan, y, btn_h);
            let placement =
                super::super::popover::place_popover(super::super::popover::PopoverSpec {
                    anchor: popover_anchor,
                    content: (content_w, content_h),
                    bounds: (width, height),
                    gap: 6.0,
                    margin: 4.0,
                });
            let (px, py, _pw, _ph) = placement.rect;
            tree.push(WidgetNode::decor(
                "top.overflow.panel",
                placement.rect,
                WidgetKind::Popover {
                    caret_x: placement.caret_x,
                    caret_up: placement.side == super::super::popover::PopoverSide::Below,
                },
            ));
            let item_rect = |index: usize| {
                let col = index % cols;
                let row = index / cols;
                (
                    px + pad + col as f64 * (btn_w + gap),
                    py + pad + row as f64 * (btn_h + gap),
                    btn_w,
                    btn_h,
                )
            };
            for (index, control) in spec.overflow().iter().copied().enumerate() {
                let id = format!("top.overflow.{}", control.id().render_id());
                tree.push(overflow_control_button_node(
                    snapshot,
                    control,
                    id,
                    item_rect(index),
                    use_icons,
                ));
            }
        }
    }

    tree
}

/// Minimized top strip: the whole tab is one restore button. It is not an
/// item id on purpose — the way back must not be hideable.
fn build_top_minimized_tab(
    snapshot: &ToolbarSnapshot,
    spec: &model::TopToolbarSpec,
    width: f64,
    height: f64,
) -> WidgetTree {
    let control = match spec.strip() {
        [model::TopToolbarNode::Control(control)] => *control,
        _ => unreachable!("minimized specification contains one restore control"),
    };
    let mut tree = WidgetTree::new((width, height));
    tree.push(WidgetNode::decor(
        "top.panel",
        (0.0, 0.0, width, height),
        WidgetKind::Panel,
    ));
    tree.push(WidgetNode::new(
        control.id().render_id().into_owned(),
        (0.0, 0.0, width, height),
        WidgetKind::IconButton {
            glyph: IconFn(toolbar_icons::top_toolbar_icon_painter(
                control.icon(snapshot).expect("restore icon"),
            )),
            icon_size: (height * 0.75).min(18.0),
            style: ButtonStyle::plain(),
        },
        Some(Interaction::click(
            control.event(snapshot),
            Some(control.tooltip(snapshot)),
        )),
    ));
    tree
}

/// Micro-mode top strip: the whole surface is one 44px round chip showing
/// the active tool inside a ring in the current color. Like the restore
/// tab, it is not an item id — the way back must not be hideable.
fn build_top_micro_chip(
    snapshot: &ToolbarSnapshot,
    spec: &model::TopToolbarSpec,
    width: f64,
    height: f64,
) -> WidgetTree {
    let control = match spec.strip() {
        [model::TopToolbarNode::Control(control)] => *control,
        _ => unreachable!("micro specification contains one chip control"),
    };
    let mut tree = WidgetTree::new((width, height));
    tree.push(WidgetNode::new(
        control.id().render_id().into_owned(),
        (0.0, 0.0, width, height),
        WidgetKind::MicroChip {
            glyph: IconFn(toolbar_icons::top_toolbar_icon_painter(
                control.icon(snapshot).expect("micro chip tool icon"),
            )),
            ring_color: (
                snapshot.color.r,
                snapshot.color.g,
                snapshot.color.b,
                snapshot.color.a,
            ),
            ring_width: model::micro_ring_width(snapshot.thickness),
        },
        Some(Interaction::click(
            control.event(snapshot),
            Some(control.tooltip(snapshot)),
        )),
    ));
    tree
}

/// Option rows shown at the bottom of the shapes popover: the controls that
/// used to hang under the bar as mini-checkboxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShapeOptionRow {
    Fill,
    PolygonSides,
}

fn shape_option_rows(snapshot: &ToolbarSnapshot) -> Vec<ShapeOptionRow> {
    let mut rows = Vec::new();
    if model::top_fill_visible(snapshot) {
        rows.push(ShapeOptionRow::Fill);
    }
    if snapshot.active_tool == Tool::RegularPolygon
        || snapshot.tool_override == Some(Tool::RegularPolygon)
    {
        rows.push(ShapeOptionRow::PolygonSides);
    }
    rows
}

fn push_option_row(
    tree: &mut WidgetTree,
    snapshot: &ToolbarSnapshot,
    row: ShapeOptionRow,
    rect: (f64, f64, f64, f64),
) {
    let (x, y, w, h) = rect;
    match row {
        ShapeOptionRow::Fill => {
            tree.push(WidgetNode::new(
                ids::TOP_UTILITY_FILL.as_str(),
                rect,
                WidgetKind::Checkbox {
                    checked: snapshot.fill_enabled,
                    label: LabelSpec::new(
                        action_short_label(Action::ToggleFill),
                        TOP_LABEL_FONT_SIZE,
                        true,
                    ),
                },
                Some(Interaction::click(
                    ToolbarEvent::ToggleFill(!snapshot.fill_enabled),
                    Some(format_binding_label(
                        action_label(Action::ToggleFill),
                        snapshot
                            .binding_hints
                            .binding_for_action(Action::ToggleFill),
                    )),
                )),
            ));
        }
        ShapeOptionRow::PolygonSides => {
            let btn = h;
            tree.push(WidgetNode::new(
                "top.options.sides-minus",
                (x, y, btn, btn),
                WidgetKind::TextButton {
                    label: LabelSpec::new("−", TOP_LABEL_FONT_SIZE, true),
                    style: ButtonStyle::plain(),
                },
                Some(Interaction::click(
                    ToolbarEvent::NudgePolygonSides(-1),
                    Some("Fewer sides".to_string()),
                )),
            ));
            tree.push(WidgetNode::decor(
                "top.options.sides-label",
                (x + btn + 4.0, y, (w - 2.0 * (btn + 4.0)).max(0.0), h),
                WidgetKind::Label(LabelSpec::new(
                    format!("{} sides", snapshot.polygon_sides),
                    TOP_LABEL_FONT_SIZE,
                    true,
                )),
            ));
            tree.push(WidgetNode::new(
                "top.options.sides-plus",
                (x + w - btn, y, btn, btn),
                WidgetKind::TextButton {
                    label: LabelSpec::new("+", TOP_LABEL_FONT_SIZE, true),
                    style: ButtonStyle::plain(),
                },
                Some(Interaction::click(
                    ToolbarEvent::NudgePolygonSides(1),
                    Some("More sides".to_string()),
                )),
            ));
        }
    }
}

pub(super) fn shape_popover_height(snapshot: &ToolbarSnapshot) -> f64 {
    if !snapshot.shape_picker_open || !model::TopToolbarSpec::shape_picker_visible(snapshot) {
        return 0.0;
    }
    let plan = plan_top_strip(snapshot);
    let is_simple = snapshot.layout_mode == crate::config::ToolbarLayoutMode::Simple;
    let (_, btn_h) = planned_button_size(snapshot, &plan);
    let gap = planned_gap(&plan);
    let pad = ToolbarLayoutSpec::TOP_POPOVER_PAD;
    let rows = model::visible_shape_picker_rows(snapshot, is_simple);
    let option_rows = shape_option_rows(snapshot);
    if rows.is_empty() && option_rows.is_empty() {
        return 0.0;
    }
    let grid_h = if rows.is_empty() {
        -gap
    } else {
        rows.len() as f64 * (btn_h + gap) - gap
    };
    let content_h =
        pad * 2.0 + grid_h + option_rows.len() as f64 * (ToolbarLayoutSpec::TOP_OPTION_ROW_H + gap);
    ToolbarLayoutSpec::TOP_SHAPE_ROW_GAP + content_h + 4.0 + 6.0
}

/// The highlight ring row grows the bar only while the highlight tool is
/// active — the lane is no longer permanently reserved.
pub(super) fn ring_row_height(snapshot: &ToolbarSnapshot) -> f64 {
    let plan = plan_top_strip(snapshot);
    ring_row_height_planned(snapshot, &plan)
}

pub(super) fn ring_row_height_planned(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> f64 {
    if !model::TopToolbarSpec::contextual_highlight_ring_visible(snapshot, plan) {
        return 0.0;
    }
    ToolbarLayoutSpec::TOP_ICON_FILL_OFFSET + ToolbarLayoutSpec::TOP_ICON_FILL_HEIGHT + 2.0
}

fn popover_anchor_below_ring(
    anchor: (f64, f64, f64, f64),
    snapshot: &ToolbarSnapshot,
    plan: &TopStripPlan,
    button_y: f64,
    button_h: f64,
) -> (f64, f64, f64, f64) {
    if ring_row_height_planned(snapshot, plan) <= 0.0 {
        return anchor;
    }
    let ring_bottom = button_y
        + button_h
        + ToolbarLayoutSpec::TOP_ICON_FILL_OFFSET
        + ToolbarLayoutSpec::TOP_ICON_FILL_HEIGHT;
    (anchor.0, ring_bottom - anchor.3, anchor.2, anchor.3)
}

pub(super) fn overflow_height(snapshot: &ToolbarSnapshot) -> f64 {
    if !snapshot.top_overflow_open {
        return 0.0;
    }
    let plan = plan_top_strip(snapshot);
    let dropped_count = model::TopToolbarSpec::overflow_control_count(snapshot, &plan);
    if dropped_count == 0 {
        return 0.0;
    }
    let (_, btn_h) = planned_button_size(snapshot, &plan);
    let cols = dropped_count.min(5);
    let rows = dropped_count.div_ceil(cols) as f64;
    rows * btn_h + (rows - 1.0) * planned_gap(&plan) + 8.0 * 2.0 + 6.0 + 4.0
}

fn tool_button_node(
    snapshot: &ToolbarSnapshot,
    tool: Tool,
    id: impl Into<super::super::node::WidgetId>,
    rect: (f64, f64, f64, f64),
    use_icons: bool,
) -> WidgetNode {
    control_button_node(
        snapshot,
        model::TopToolbarControl::Tool(tool),
        id,
        rect,
        use_icons,
    )
}

fn control_button_node(
    snapshot: &ToolbarSnapshot,
    control: model::TopToolbarControl,
    id: impl Into<super::super::node::WidgetId>,
    rect: (f64, f64, f64, f64),
    use_icons: bool,
) -> WidgetNode {
    let tooltip = control.tooltip(snapshot);
    control_button_node_with_tooltip(snapshot, control, id, rect, use_icons, tooltip)
}

fn overflow_control_button_node(
    snapshot: &ToolbarSnapshot,
    control: model::TopToolbarControl,
    id: impl Into<super::super::node::WidgetId>,
    rect: (f64, f64, f64, f64),
    use_icons: bool,
) -> WidgetNode {
    let tooltip = control.overflow_tooltip(snapshot);
    control_button_node_with_tooltip(snapshot, control, id, rect, use_icons, tooltip)
}

fn control_button_node_with_tooltip(
    snapshot: &ToolbarSnapshot,
    control: model::TopToolbarControl,
    id: impl Into<super::super::node::WidgetId>,
    rect: (f64, f64, f64, f64),
    use_icons: bool,
    tooltip: String,
) -> WidgetNode {
    let enabled = control.enabled(snapshot);
    let style = if !enabled {
        ButtonStyle::disabled()
    } else {
        match control.role() {
            model::TopToolbarControlRole::Destructive => ButtonStyle::destructive(),
            _ => ButtonStyle::active(control.active(snapshot)),
        }
    };
    let kind = if use_icons {
        let icon_size = if control == model::TopToolbarControl::ShapePicker {
            ToolbarLayoutSpec::TOP_ICON_SIZE
        } else {
            ToolbarLayoutSpec::TOP_ICON_SIZE.min((rect.2 - 4.0).max(8.0))
        };
        WidgetKind::IconButton {
            glyph: IconFn(toolbar_icons::top_toolbar_icon_painter(
                control.icon(snapshot).expect("button icon"),
            )),
            icon_size,
            style,
        }
    } else {
        WidgetKind::TextButton {
            label: LabelSpec::new(control.label(snapshot), TOP_LABEL_FONT_SIZE, true),
            style,
        }
    };
    let interact = enabled.then(|| Interaction::click(control.event(snapshot), Some(tooltip)));
    let badge = (rect.2 > super::TOP_COMPACT_BUTTON)
        .then(|| control.shortcut_badge(snapshot))
        .flatten();
    // Icon buttons carry their shortcut as a caption under the icon
    // (Excalidraw pattern); text buttons keep the boxed corner micro-badge.
    let placement = if use_icons {
        ShortcutBadgePlacement::Below
    } else {
        ShortcutBadgePlacement::Corner
    };
    WidgetNode::new(id, rect, kind, interact).with_shortcut_badge(badge.as_deref(), placement)
}
