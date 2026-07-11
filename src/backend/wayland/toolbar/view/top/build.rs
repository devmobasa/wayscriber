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
use crate::ui::toolbar::bindings::{tool_label, tool_tooltip_label};
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot, model};

use super::super::node::{
    ButtonStyle, IconFn, Interaction, LabelSpec, ShortcutBadgePlacement, WidgetKind, WidgetNode,
};
use super::super::tree::WidgetTree;
use super::{
    MINI_LABEL_FONT_SIZE, TOP_CHIP_SIZE, TOP_COMPACT_CHROME, TOP_COMPACT_GAP,
    TOP_COMPACT_MARGIN_RIGHT, TOP_DIVIDER_SPAN, TOP_LABEL_FONT_SIZE, TOP_SWATCH_GAP,
    TOP_SWATCH_SIZE, TopStripPlan, bar_band_height, base_bar_height, plan_top_strip,
    planned_button_size, planned_gap, planned_use_icons,
};

pub(super) fn build_top_view_planned(
    snapshot: &ToolbarSnapshot,
    plan: &TopStripPlan,
    width: f64,
    height: f64,
) -> WidgetTree {
    if snapshot.top_minimized {
        return build_top_minimized_tab(width, height);
    }
    let gap = planned_gap(plan);
    let mut tree = WidgetTree::new((width, height));

    tree.push(WidgetNode::decor(
        "top.panel",
        (0.0, 0.0, width, bar_band_height(snapshot, plan)),
        WidgetKind::Panel,
    ));

    let mut x = if plan.compact {
        4.0
    } else {
        ToolbarLayoutSpec::TOP_START_X
    };
    let handle_size = ToolbarLayoutSpec::TOP_HANDLE_SIZE;
    let handle_visible = model::toolbar_item_visible(snapshot, ids::TOP_CHROME_DRAG);
    if handle_visible {
        tree.push(WidgetNode::new(
            ids::TOP_CHROME_DRAG.as_str(),
            (x, ToolbarLayoutSpec::TOP_HANDLE_Y, handle_size, handle_size),
            WidgetKind::DragHandle,
            Some(Interaction {
                event: ToolbarEvent::MoveTopToolbar { x: 0.0, y: 0.0 },
                kind: HitKind::DragMoveTop,
                tooltip: Some("Drag toolbar".to_string()),
            }),
        ));
        x += handle_size + gap;
    }

    let is_simple = snapshot.layout_mode == crate::config::ToolbarLayoutMode::Simple;
    let current_shape_tool =
        model::current_shape_tool(snapshot.active_tool, snapshot.tool_override);
    let fill_tool_active = model::fill_tool_active(snapshot.active_tool, snapshot.tool_override);

    let use_icons = planned_use_icons(snapshot, plan);
    let (btn_w, btn_h) = planned_button_size(snapshot, plan);
    let base_height = base_bar_height(snapshot);
    let y = (base_height - btn_h) / 2.0;

    let push_divider = |tree: &mut WidgetTree, x: &mut f64, key: &str| {
        let divider_span = if plan.compact { 3.0 } else { TOP_DIVIDER_SPAN };
        tree.push(WidgetNode::decor(
            format!("top.divider.{key}"),
            (*x + (divider_span - 1.0) / 2.0, y + 6.0, 1.0, btn_h - 12.0),
            WidgetKind::Divider { vertical: true },
        ));
        *x += divider_span + gap;
    };

    // --- Tool groups: pens | shapes ----------------------------------------
    let mut previous_group: Option<model::TopToolGroup> = None;
    let mut tool_drawn = false;
    for tool in model::visible_top_tool_buttons(is_simple, snapshot) {
        if plan.dropped_tools.contains(&tool) {
            continue;
        }
        let group = model::top_tool_group(tool);
        if let Some(previous) = previous_group
            && previous != group
        {
            push_divider(&mut tree, &mut x, "tools");
        }
        previous_group = Some(group);
        let active = snapshot.active_tool == tool || snapshot.tool_override == Some(tool);
        tree.push(tool_button_node(
            snapshot,
            tool,
            model::toolbar_item_id_for_tool(tool).as_str(),
            (x, y, btn_w, btn_h),
            active,
            use_icons,
        ));
        x += btn_w + gap;
        tool_drawn = true;
    }

    // --- Shapes picker (family icon, caret grid below) ----------------------
    let mut picker_anchor: Option<(f64, f64, f64, f64)> = None;
    if model::top_shape_picker_visible(snapshot) {
        if previous_group == Some(model::TopToolGroup::Pens) {
            push_divider(&mut tree, &mut x, "tools");
        }
        let picker_active = snapshot.shape_picker_open || current_shape_tool.is_some();
        let interact = Interaction::click(
            ToolbarEvent::ToggleShapePicker(!snapshot.shape_picker_open),
            Some("Shapes".to_string()),
        );
        let kind = if use_icons {
            WidgetKind::IconButton {
                glyph: IconFn(toolbar_icons::draw_icon_shape_picker),
                icon_size: ToolbarLayoutSpec::TOP_ICON_SIZE,
                style: ButtonStyle::active(picker_active),
            }
        } else {
            WidgetKind::TextButton {
                label: LabelSpec::new("Shapes", TOP_LABEL_FONT_SIZE, true),
                style: ButtonStyle::active(picker_active),
            }
        };
        tree.push(WidgetNode::new(
            ids::TOP_UTILITY_SHAPE_PICKER.as_str(),
            (x, y, btn_w, btn_h),
            kind,
            Some(interact),
        ));
        picker_anchor = Some((x, y, btn_w, btn_h));
        x += btn_w + gap;
        tool_drawn = true;
    }

    // --- Annotation utilities    // --- Annotation utilities (Clear is pulled out and isolated below) ------
    let utilities: Vec<model::TopUtilityButton> =
        model::visible_top_utility_buttons(snapshot, is_simple, snapshot.use_icons)
            .into_iter()
            .filter(|button| *button != model::TopUtilityButton::ClearCanvas)
            .filter(|button| !plan.dropped_utilities.contains(button))
            .collect();
    let clear_visible = model::visible_top_utility_buttons(snapshot, is_simple, snapshot.use_icons)
        .contains(&model::TopUtilityButton::ClearCanvas);
    if !utilities.is_empty() && tool_drawn {
        push_divider(&mut tree, &mut x, "annotations");
    }
    for button in utilities {
        match button {
            model::TopUtilityButton::Text => {
                tree.push(utility_node(
                    snapshot,
                    ids::TOP_UTILITY_TEXT.as_str(),
                    (x, y, btn_w, btn_h),
                    IconFn(toolbar_icons::draw_icon_text),
                    action_short_label(Action::EnterTextMode),
                    ButtonStyle::active(snapshot.text_active),
                    ToolbarEvent::EnterTextMode,
                    format_binding_label(
                        action_label(Action::EnterTextMode),
                        snapshot
                            .binding_hints
                            .binding_for_action(Action::EnterTextMode),
                    ),
                    use_icons,
                ));
                x += btn_w + gap;
            }
            model::TopUtilityButton::StickyNote => {
                tree.push(utility_node(
                    snapshot,
                    ids::TOP_UTILITY_STICKY_NOTE.as_str(),
                    (x, y, btn_w, btn_h),
                    IconFn(toolbar_icons::draw_icon_note),
                    action_short_label(Action::EnterStickyNoteMode),
                    ButtonStyle::active(snapshot.note_active),
                    ToolbarEvent::EnterStickyNoteMode,
                    format_binding_label(
                        action_label(Action::EnterStickyNoteMode),
                        snapshot
                            .binding_hints
                            .binding_for_action(Action::EnterStickyNoteMode),
                    ),
                    use_icons,
                ));
                x += btn_w + gap;
            }
            model::TopUtilityButton::Screenshot => {
                tree.push(utility_node(
                    snapshot,
                    ids::TOP_UTILITY_SCREENSHOT.as_str(),
                    (x, y, btn_w, btn_h),
                    IconFn(toolbar_icons::draw_icon_screenshot),
                    "Shot",
                    ButtonStyle::plain(),
                    ToolbarEvent::CaptureScreenshot,
                    format_binding_label(
                        action_label(Action::CaptureSelection),
                        snapshot
                            .binding_hints
                            .binding_for_action(Action::CaptureSelection),
                    ),
                    use_icons,
                ));
                x += btn_w + gap;
            }
            model::TopUtilityButton::Highlight => {
                tree.push(utility_node(
                    snapshot,
                    ids::TOP_UTILITY_HIGHLIGHT.as_str(),
                    (x, y, btn_w, btn_h),
                    IconFn(toolbar_icons::draw_icon_highlight),
                    "Highlight",
                    ButtonStyle::active(snapshot.any_highlight_active),
                    ToolbarEvent::ToggleAllHighlight(!snapshot.any_highlight_active),
                    format_binding_label(
                        action_label(Action::ToggleHighlightTool),
                        snapshot
                            .binding_hints
                            .binding_for_action(Action::ToggleHighlightTool),
                    ),
                    use_icons,
                ));
                if snapshot.highlight_tool_active && model::top_highlight_ring_visible(snapshot) {
                    let ring_y = y + btn_h + ToolbarLayoutSpec::TOP_ICON_FILL_OFFSET;
                    tree.push(WidgetNode::new(
                        ids::TOP_UTILITY_HIGHLIGHT_RING.as_str(),
                        (x, ring_y, btn_w, ToolbarLayoutSpec::TOP_ICON_FILL_HEIGHT),
                        WidgetKind::MiniCheckbox {
                            checked: snapshot.highlight_tool_ring_enabled,
                            label: LabelSpec::new("Ring", MINI_LABEL_FONT_SIZE, false),
                        },
                        Some(Interaction::click(
                            ToolbarEvent::ToggleHighlightToolRing(
                                !snapshot.highlight_tool_ring_enabled,
                            ),
                            Some("Highlight ring".to_string()),
                        )),
                    ));
                }
                x += btn_w + gap;
            }
            model::TopUtilityButton::ClearCanvas | model::TopUtilityButton::IconMode => {}
        }
    }

    // --- Quick colors + current-color chip ----------------------------------
    if model::toolbar_item_visible(snapshot, ids::TOP_GROUP_QUICK_COLORS) {
        push_divider(&mut tree, &mut x, "colors");
        let swatches_have_badges = snapshot
            .quick_colors
            .rendered_entries()
            .iter()
            .take(plan.swatch_count)
            .enumerate()
            .any(|(index, _)| snapshot.binding_hints.quick_color_badge(index).is_some());
        let swatch_y = if swatches_have_badges {
            // Reserve one 10px badge row plus a 1px gap above every swatch,
            // keeping bound and unbound colors aligned in both bar modes.
            y + (btn_h - (10.0 + 1.0 + TOP_SWATCH_SIZE)) / 2.0 + 11.0
        } else {
            y + (btn_h - TOP_SWATCH_SIZE) / 2.0
        };
        for (index, entry) in snapshot
            .quick_colors
            .rendered_entries()
            .iter()
            .take(plan.swatch_count)
            .enumerate()
        {
            let action = crate::config::QuickColorPalette::action_for_index(index);
            let binding =
                action.and_then(|action| snapshot.binding_hints.binding_for_action(action));
            tree.push(
                WidgetNode::new(
                    format!("top.quick-color.{index}"),
                    (x, swatch_y, TOP_SWATCH_SIZE, TOP_SWATCH_SIZE),
                    WidgetKind::Swatch {
                        color: (entry.color.r, entry.color.g, entry.color.b, entry.color.a),
                        selected: entry.color == snapshot.color,
                    },
                    Some(Interaction::click(
                        ToolbarEvent::SetColor(entry.color),
                        Some(format_binding_label(&entry.label, binding)),
                    )),
                )
                .with_shortcut_badge(
                    snapshot.binding_hints.quick_color_badge(index),
                    ShortcutBadgePlacement::Above,
                ),
            );
            x += TOP_SWATCH_SIZE + TOP_SWATCH_GAP;
        }
        // The chip shows the current color and opens the full picker; it
        // never collapses under width pressure.
        let chip_y = y + (btn_h - TOP_CHIP_SIZE) / 2.0;
        x += gap - TOP_SWATCH_GAP;
        tree.push(WidgetNode::new(
            ids::TOP_GROUP_QUICK_COLORS.as_str(),
            (x, chip_y, TOP_CHIP_SIZE, TOP_CHIP_SIZE),
            WidgetKind::Swatch {
                color: (
                    snapshot.color.r,
                    snapshot.color.g,
                    snapshot.color.b,
                    snapshot.color.a,
                ),
                selected: true,
            },
            Some(Interaction::click(
                ToolbarEvent::OpenColorPickerPopup,
                Some("Color picker".to_string()),
            )),
        ));
        x += TOP_CHIP_SIZE + gap;
    }

    // --- History -------------------------------------------------------------
    let undo_visible = model::toolbar_item_visible(snapshot, ids::TOP_UTILITY_UNDO);
    let redo_visible = model::toolbar_item_visible(snapshot, ids::TOP_UTILITY_REDO);
    if undo_visible || redo_visible {
        push_divider(&mut tree, &mut x, "history");
    }
    if undo_visible {
        tree.push(history_node(
            snapshot,
            ids::TOP_UTILITY_UNDO.as_str(),
            (x, y, btn_w, btn_h),
            IconFn(toolbar_icons::draw_icon_undo),
            Action::Undo,
            ToolbarEvent::Undo,
            snapshot.undo_available,
            use_icons,
        ));
        x += btn_w + gap;
    }
    if redo_visible {
        tree.push(history_node(
            snapshot,
            ids::TOP_UTILITY_REDO.as_str(),
            (x, y, btn_w, btn_h),
            IconFn(toolbar_icons::draw_icon_redo),
            Action::Redo,
            ToolbarEvent::Redo,
            snapshot.redo_available,
            use_icons,
        ));
        x += btn_w + gap;
    }

    // --- Destructive Clear, isolated by a double gap --------------------------
    if clear_visible {
        x += gap;
        tree.push(utility_node(
            snapshot,
            ids::TOP_UTILITY_CLEAR_CANVAS.as_str(),
            (x, y, btn_w, btn_h),
            IconFn(toolbar_icons::draw_icon_clear),
            action_short_label(Action::ClearCanvas),
            ButtonStyle::destructive(),
            ToolbarEvent::ClearCanvas,
            format_binding_label(
                action_label(Action::ClearCanvas),
                snapshot
                    .binding_hints
                    .binding_for_action(Action::ClearCanvas),
            ),
            use_icons,
        ));
    }

    // --- Shapes popover: the grid plus per-tool options ------------------------
    if snapshot.shape_picker_open
        && model::top_shape_picker_visible(snapshot)
        && let Some(anchor) = picker_anchor
    {
        let rows = model::visible_shape_picker_rows(snapshot, is_simple);
        let option_rows = shape_option_rows(snapshot, fill_tool_active);
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
                    let active =
                        snapshot.active_tool == tool || snapshot.tool_override == Some(tool);
                    tree.push(tool_button_node(
                        snapshot,
                        tool,
                        format!(
                            "top.picker.{}",
                            model::toolbar_item_id_for_tool(tool).as_str()
                        ),
                        (shape_x, row_y, btn_w, btn_h),
                        active,
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

    // --- Right-aligned chrome ---------------------------------------------------
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
    let mut right_x = width - chrome_margin_right - chrome_size;
    if model::toolbar_item_visible(snapshot, ids::TOP_CHROME_CLOSE) {
        tree.push(WidgetNode::new(
            ids::TOP_CHROME_CLOSE.as_str(),
            (right_x, chrome_y, chrome_size, chrome_size),
            WidgetKind::MinimizeButton,
            Some(Interaction::click(
                ToolbarEvent::SetTopMinimized(true),
                Some("Minimize (leaves a restore tab)".to_string()),
            )),
        ));
        right_x -= chrome_size + chrome_gap;
    }
    let mut overflow_anchor = None;
    if plan.show_overflow {
        let rect = (right_x, chrome_y, chrome_size, chrome_size);
        overflow_anchor = Some(rect);
        tree.push(WidgetNode::new(
            ids::TOP_CHROME_OVERFLOW.as_str(),
            rect,
            WidgetKind::IconButton {
                glyph: IconFn(toolbar_icons::draw_icon_more),
                icon_size: chrome_size * 0.7,
                style: ButtonStyle::active(snapshot.top_overflow_open),
            },
            Some(Interaction::click(
                ToolbarEvent::ToggleTopOverflow(!snapshot.top_overflow_open),
                Some("More tools".to_string()),
            )),
        ));
        right_x -= chrome_size + chrome_gap;
    }
    if model::toolbar_item_visible(snapshot, ids::TOP_CHROME_PIN) {
        tree.push(WidgetNode::new(
            ids::TOP_CHROME_PIN.as_str(),
            (right_x, chrome_y, chrome_size, chrome_size),
            WidgetKind::PinButton {
                pinned: snapshot.top_pinned,
            },
            Some(Interaction::click(
                ToolbarEvent::PinTopToolbar(!snapshot.top_pinned),
                Some(if snapshot.top_pinned {
                    "Pinned: opens at startup (click to disable)".to_string()
                } else {
                    "Pin: click to open at startup".to_string()
                }),
            )),
        ));
    }

    // --- Overflow popover: the width-dropped items ---------------------------
    if snapshot.top_overflow_open
        && let Some(anchor) = overflow_anchor
    {
        let dropped_count = plan.dropped_tools.len() + plan.dropped_utilities.len();
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
            let mut index = 0usize;
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
            for tool in &plan.dropped_tools {
                let active = snapshot.active_tool == *tool || snapshot.tool_override == Some(*tool);
                tree.push(tool_button_node(
                    snapshot,
                    *tool,
                    format!(
                        "top.overflow.{}",
                        model::toolbar_item_id_for_tool(*tool).as_str()
                    ),
                    item_rect(index),
                    active,
                    use_icons,
                ));
                index += 1;
            }
            for utility in &plan.dropped_utilities {
                let rect = item_rect(index);
                match utility {
                    model::TopUtilityButton::Text => tree.push(utility_node(
                        snapshot,
                        "top.overflow.top.utility.text",
                        rect,
                        IconFn(toolbar_icons::draw_icon_text),
                        action_short_label(Action::EnterTextMode),
                        ButtonStyle::active(snapshot.text_active),
                        ToolbarEvent::EnterTextMode,
                        action_label(Action::EnterTextMode).to_string(),
                        use_icons,
                    )),
                    model::TopUtilityButton::StickyNote => tree.push(utility_node(
                        snapshot,
                        "top.overflow.top.utility.sticky-note",
                        rect,
                        IconFn(toolbar_icons::draw_icon_note),
                        action_short_label(Action::EnterStickyNoteMode),
                        ButtonStyle::active(snapshot.note_active),
                        ToolbarEvent::EnterStickyNoteMode,
                        action_label(Action::EnterStickyNoteMode).to_string(),
                        use_icons,
                    )),
                    model::TopUtilityButton::Screenshot => tree.push(utility_node(
                        snapshot,
                        "top.overflow.top.utility.screenshot",
                        rect,
                        IconFn(toolbar_icons::draw_icon_screenshot),
                        "Shot",
                        ButtonStyle::plain(),
                        ToolbarEvent::CaptureScreenshot,
                        action_label(Action::CaptureSelection).to_string(),
                        use_icons,
                    )),
                    model::TopUtilityButton::Highlight => tree.push(utility_node(
                        snapshot,
                        "top.overflow.top.utility.highlight",
                        rect,
                        IconFn(toolbar_icons::draw_icon_highlight),
                        "Highlight",
                        ButtonStyle::active(snapshot.any_highlight_active),
                        ToolbarEvent::ToggleAllHighlight(!snapshot.any_highlight_active),
                        action_label(Action::ToggleHighlightTool).to_string(),
                        use_icons,
                    )),
                    model::TopUtilityButton::ClearCanvas | model::TopUtilityButton::IconMode => {}
                }
                index += 1;
            }
        }
    }

    tree
}

/// Minimized top strip: the whole tab is one restore button. It is not an
/// item id on purpose — the way back must not be hideable.
fn build_top_minimized_tab(width: f64, height: f64) -> WidgetTree {
    let mut tree = WidgetTree::new((width, height));
    tree.push(WidgetNode::decor(
        "top.panel",
        (0.0, 0.0, width, height),
        WidgetKind::Panel,
    ));
    tree.push(WidgetNode::new(
        "top.chrome.restore",
        (0.0, 0.0, width, height),
        WidgetKind::IconButton {
            glyph: IconFn(toolbar_icons::draw_icon_restore),
            icon_size: (height * 0.75).min(18.0),
            style: ButtonStyle::plain(),
        },
        Some(Interaction::click(
            ToolbarEvent::SetTopMinimized(false),
            Some("Show toolbar".to_string()),
        )),
    ));
    tree
}

/// Per-tool option rows shown at the bottom of the shapes popover: the
/// controls that used to hang under the bar as mini-checkboxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShapeOptionRow {
    Fill,
    PolygonSides,
}

fn shape_option_rows(snapshot: &ToolbarSnapshot, fill_tool_active: bool) -> Vec<ShapeOptionRow> {
    let mut rows = Vec::new();
    if fill_tool_active && model::top_fill_visible(snapshot) {
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
    if !snapshot.shape_picker_open || !model::top_shape_picker_visible(snapshot) {
        return 0.0;
    }
    let is_simple = snapshot.layout_mode == crate::config::ToolbarLayoutMode::Simple;
    let plan = plan_top_strip(snapshot);
    let (_, btn_h) = planned_button_size(snapshot, &plan);
    let gap = planned_gap(&plan);
    let pad = ToolbarLayoutSpec::TOP_POPOVER_PAD;
    let rows = model::visible_shape_picker_rows(snapshot, is_simple);
    let fill_tool_active = model::fill_tool_active(snapshot.active_tool, snapshot.tool_override);
    let option_rows = shape_option_rows(snapshot, fill_tool_active);
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
    let is_simple = snapshot.layout_mode == crate::config::ToolbarLayoutMode::Simple;
    if !planned_use_icons(snapshot, plan)
        || is_simple
        || !snapshot.highlight_tool_active
        || !model::top_highlight_ring_visible(snapshot)
    {
        return 0.0;
    }
    let highlight_shown =
        model::visible_top_utility_buttons(snapshot, is_simple, snapshot.use_icons)
            .contains(&model::TopUtilityButton::Highlight)
            && !plan
                .dropped_utilities
                .contains(&model::TopUtilityButton::Highlight);
    if !highlight_shown {
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
    let dropped_count = plan.dropped_tools.len() + plan.dropped_utilities.len();
    if dropped_count == 0 || !plan.show_overflow {
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
    active: bool,
    use_icons: bool,
) -> WidgetNode {
    let tooltip = tool_tooltip(snapshot, tool, tool_tooltip_label(tool));
    let kind = if use_icons {
        WidgetKind::IconButton {
            glyph: semantic_icon_fn(model::semantic_icon_for_tool(tool)),
            icon_size: ToolbarLayoutSpec::TOP_ICON_SIZE.min((rect.2 - 4.0).max(8.0)),
            style: ButtonStyle::active(active),
        }
    } else {
        WidgetKind::TextButton {
            label: LabelSpec::new(tool_label(tool), TOP_LABEL_FONT_SIZE, true),
            style: ButtonStyle::active(active),
        }
    };
    let badge = (rect.2 > super::TOP_COMPACT_BUTTON)
        .then(|| snapshot.binding_hints.badge_for_tool(tool))
        .flatten();
    WidgetNode::new(
        id,
        rect,
        kind,
        Some(Interaction::click(
            ToolbarEvent::SelectTool(tool),
            Some(tooltip),
        )),
    )
    .with_shortcut_badge(badge, ShortcutBadgePlacement::Corner)
}

#[allow(clippy::too_many_arguments)]
fn utility_node(
    snapshot: &ToolbarSnapshot,
    id: impl Into<super::super::node::WidgetId>,
    rect: (f64, f64, f64, f64),
    glyph: IconFn,
    label: &str,
    style: ButtonStyle,
    event: ToolbarEvent,
    tooltip: String,
    use_icons: bool,
) -> WidgetNode {
    let badge = (rect.2 > super::TOP_COMPACT_BUTTON)
        .then(|| snapshot.binding_hints.badge_for_event(&event))
        .flatten();
    let kind = if use_icons {
        WidgetKind::IconButton {
            glyph,
            icon_size: ToolbarLayoutSpec::TOP_ICON_SIZE.min((rect.2 - 4.0).max(8.0)),
            style,
        }
    } else {
        WidgetKind::TextButton {
            label: LabelSpec::new(label, TOP_LABEL_FONT_SIZE, true),
            style,
        }
    };
    WidgetNode::new(
        id,
        rect,
        kind,
        Some(Interaction::click(event, Some(tooltip))),
    )
    .with_shortcut_badge(badge, ShortcutBadgePlacement::Corner)
}

/// Undo/Redo button: dimmed and non-interactive while unavailable.
#[allow(clippy::too_many_arguments)]
fn history_node(
    snapshot: &ToolbarSnapshot,
    id: &'static str,
    rect: (f64, f64, f64, f64),
    glyph: IconFn,
    action: Action,
    event: ToolbarEvent,
    enabled: bool,
    use_icons: bool,
) -> WidgetNode {
    let style = if enabled {
        ButtonStyle::plain()
    } else {
        ButtonStyle::disabled()
    };
    let kind = if use_icons {
        WidgetKind::IconButton {
            glyph,
            icon_size: ToolbarLayoutSpec::TOP_ICON_SIZE.min((rect.2 - 4.0).max(8.0)),
            style,
        }
    } else {
        WidgetKind::TextButton {
            label: LabelSpec::new(action_short_label(action), TOP_LABEL_FONT_SIZE, true),
            style,
        }
    };
    let interact = enabled.then(|| {
        Interaction::click(
            event,
            Some(format_binding_label(
                action_label(action),
                snapshot.binding_hints.binding_for_action(action),
            )),
        )
    });
    let badge = (rect.2 > super::TOP_COMPACT_BUTTON)
        .then(|| snapshot.binding_hints.badge_for_action(action))
        .flatten();
    WidgetNode::new(id, rect, kind, interact)
        .with_shortcut_badge(badge, ShortcutBadgePlacement::Corner)
}

/// Tooltip text for a tool button: label plus binding and/or drag hint.
fn tool_tooltip(snapshot: &ToolbarSnapshot, tool: Tool, label: &str) -> String {
    let default_hint = model::default_drag_hint(tool);
    let binding = match (snapshot.binding_hints.for_tool(tool), default_hint) {
        (Some(binding), Some(fallback)) => Some(format!("{}, {}", binding, fallback)),
        (Some(binding), None) => Some(binding.to_string()),
        (None, Some(fallback)) => Some(fallback.to_string()),
        (None, None) => None,
    };
    format_binding_label(label, binding.as_deref())
}

/// Glyph function for a semantic tool icon.
fn semantic_icon_fn(icon: model::SemanticToolIcon) -> IconFn {
    use model::SemanticToolIcon as S;
    IconFn(match icon {
        S::Select => toolbar_icons::draw_icon_select,
        S::Pen => toolbar_icons::draw_icon_pen,
        S::Line => toolbar_icons::draw_icon_line,
        S::Rect => toolbar_icons::draw_icon_rect,
        S::Circle => toolbar_icons::draw_icon_circle,
        S::Triangle => toolbar_icons::draw_icon_triangle,
        S::Parallelogram => toolbar_icons::draw_icon_parallelogram,
        S::Rhombus => toolbar_icons::draw_icon_rhombus,
        S::Polygon => toolbar_icons::draw_icon_polygon,
        S::FreeformPolygon => toolbar_icons::draw_icon_freeform_polygon,
        S::Arrow => toolbar_icons::draw_icon_arrow,
        S::Blur => toolbar_icons::draw_icon_blur,
        S::Marker => toolbar_icons::draw_icon_marker,
        S::Highlight => toolbar_icons::draw_icon_highlight,
        S::StepMarker => toolbar_icons::draw_icon_step_marker,
        S::Eraser => toolbar_icons::draw_icon_eraser,
    })
}
