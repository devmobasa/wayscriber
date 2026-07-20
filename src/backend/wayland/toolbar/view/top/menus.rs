//! Canvas/Session/Settings popovers anchored to the top strip's overflow toggle.
//!
//! With `[ui.toolbar] side_layout = "pill"` the side palette is retired, so
//! the Canvas, Session, and Settings panes are re-hosted here as popovers
//! opened from the matching overflow-menu entries. Their content reuses the
//! renderer-neutral models and events the side panes render, minus the side
//! palette's chrome (pane tabs, collapsible section headers).
//!
//! Content taller than [`MENU_MAX_CONTENT_H`] scrolls internally behind a
//! proportional scrollbar (the side palette's scroll pattern): the tree
//! painter has no clip, so rows are either fully inside the viewport or
//! withheld entirely (paint and hits alike).

use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::render::side_palette::session::{
    strip_session_extension, truncate_middle, truncate_start,
};
use crate::backend::wayland::toolbar::rows::{grid_layout, row_item_width};
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot, model};

use super::super::node::{ButtonStyle, Interaction, LabelSpec, WidgetKind, WidgetNode};
use super::super::popover;
use super::super::tree::WidgetTree;

/// Content width inside the popover padding (mirrors the side palette's
/// content column). Shared with the GTK popover viewport via the theme
/// token so the frontends cannot drift.
pub(super) const MENU_CONTENT_W: f64 = crate::ui::theme::toolbar::MENU_CONTENT_W;
/// Content column for the Canvas popover. It matches the text-menu width so
/// the Step Undo/Redo cluster has comfortable spacing, and is shared with the
/// GTK canvas viewport via the theme token so the frontends cannot drift.
pub(super) const CANVAS_MENU_CONTENT_W: f64 = crate::ui::theme::toolbar::CANVAS_MENU_CONTENT_W;
const MENU_PAD: f64 = 10.0;
const MENU_GAP: f64 = 5.0;
const MENU_BUTTON_H: f64 = 24.0;
const MENU_RECENT_H: f64 = 22.0;
const MENU_TOGGLE_H: f64 = 24.0;
const MENU_TOGGLE_GAP: f64 = 6.0;
const MENU_SEGMENT_H: f64 = 22.0;
const MENU_HEADER_H: f64 = 16.0;
const MENU_NAME_H: f64 = 14.0;
const MENU_PATH_H: f64 = 12.0;
const MENU_CONFIRM_LABEL_H: f64 = 18.0;
/// Cap on the visible content height; taller content scrolls internally.
/// Shared with the GTK `ScrolledWindow` cap via the theme token.
pub(super) const MENU_MAX_CONTENT_H: f64 = crate::ui::theme::toolbar::MENU_MAX_CONTENT_H;
const MENU_SCROLLBAR_W: f64 = 6.0;
pub(super) const MENU_LABEL_FONT: f64 = 12.0;
pub(super) const MENU_META_FONT: f64 = 11.0;
const MENU_PATH_FONT: f64 = 9.5;
const MENU_ITEM_HANDLE_W: f64 = 24.0;
const MENU_ITEM_MOVE_W: f64 = 28.0;
const MENU_ITEM_GAP: f64 = 4.0;
/// Gap between the anchor and the popover panel plus the caret allowance
/// under it (mirrors `overflow_height`'s trailing margins).
const MENU_ANCHOR_GAP: f64 = 6.0;
const MENU_BOTTOM_MARGIN: f64 = 4.0;
/// Gap between two Canvas-popover sections (Boards/Pages/Advanced/Zoom/Step).
const CANVAS_SECTION_GAP: f64 = 8.0;
const CANVAS_STEP_ROW_H: f64 = 24.0;
const CANVAS_STEP_BTN_W: f64 = 66.0;
const CANVAS_STEPPER_W: f64 = 24.0;
const CANVAS_STEPS_LABEL_W: f64 = 56.0;
const CANVAS_SLIDER_H: f64 = 18.0;

/// Content nodes for whichever popover is open, in content-local
/// coordinates (origin at the top-left of the content column), plus the
/// natural (unclamped) content height. Canvas wins over Session over
/// Settings if several flags are somehow set — the apply layer keeps them
/// mutually exclusive.
fn open_menu_content(snapshot: &ToolbarSnapshot) -> Option<(&'static str, Vec<WidgetNode>)> {
    if snapshot.canvas_popover_open {
        return canvas_menu_content(snapshot).map(|nodes| ("canvas", nodes));
    }
    if snapshot.session_popover_open {
        return session_menu_content(snapshot).map(|nodes| ("session", nodes));
    }
    if snapshot.settings_popover_open {
        return settings_menu_content(snapshot).map(|nodes| ("settings", nodes));
    }
    None
}

fn content_height(nodes: &[WidgetNode]) -> f64 {
    nodes
        .iter()
        .map(|node| node.rect.1 + node.rect.3)
        .fold(0.0, f64::max)
}

/// Scroll bounds for the open Canvas/Session/Settings popover as
/// (natural_height, viewport_height), both in pre-scale spec units; `None`
/// while no menu popover is open. Max scroll = (natural - viewport).max(0).
pub(super) fn menu_scroll_bounds(snapshot: &ToolbarSnapshot) -> Option<(f64, f64)> {
    let (_, nodes) = open_menu_content(snapshot)?;
    let natural = content_height(&nodes);
    Some((natural, natural.min(MENU_MAX_CONTENT_H)))
}

/// Extra surface height the open Canvas/Session/Settings popover needs below the
/// band (mirrors `overflow_height`).
pub(super) fn menu_popover_height(snapshot: &ToolbarSnapshot) -> f64 {
    let Some((_, nodes)) = open_menu_content(snapshot) else {
        return 0.0;
    };
    let viewport = content_height(&nodes).min(MENU_MAX_CONTENT_H);
    MENU_ANCHOR_GAP + viewport + MENU_PAD * 2.0 + MENU_BOTTOM_MARGIN
}

/// Build the open popover into the tree, anchored like the overflow menu.
pub(super) fn push_menu_popover(
    tree: &mut WidgetTree,
    snapshot: &ToolbarSnapshot,
    anchor: (f64, f64, f64, f64),
    bounds: (f64, f64),
) {
    let Some((key, nodes)) = open_menu_content(snapshot) else {
        return;
    };
    let natural = content_height(&nodes);
    let viewport = natural.min(MENU_MAX_CONTENT_H);
    let max_scroll = (natural - viewport).max(0.0);
    let scroll = if max_scroll > 0.0 {
        snapshot.top_popover_scroll.clamp(0.0, max_scroll)
    } else {
        0.0
    };

    // The Canvas popover builds its nodes at its shared canvas-column width;
    // the panel must size to match so its rows fill the width without a gap.
    let content_w = if key == "canvas" {
        CANVAS_MENU_CONTENT_W
    } else {
        MENU_CONTENT_W
    };
    let placement = popover::place_popover(popover::PopoverSpec {
        anchor,
        content: (content_w + MENU_PAD * 2.0, viewport + MENU_PAD * 2.0),
        bounds,
        gap: MENU_ANCHOR_GAP,
        margin: 4.0,
    });
    let (px, py, pw, _ph) = placement.rect;
    tree.push(WidgetNode::decor(
        format!("top.menu.{key}.panel"),
        placement.rect,
        WidgetKind::Popover {
            caret_x: placement.caret_x,
            caret_up: placement.side == popover::PopoverSide::Below,
        },
    ));
    for mut node in nodes {
        let (x, y, w, h) = node.rect;
        let shifted_y = y - scroll;
        // No painter clip: a row is either fully inside the scroll viewport
        // or withheld entirely (paint and hits alike).
        if shifted_y < -0.5 || shifted_y + h > viewport + 0.5 {
            continue;
        }
        node.rect = (px + MENU_PAD + x, py + MENU_PAD + shifted_y, w, h);
        tree.push(node);
    }
    if max_scroll > 0.0 {
        let track = (
            px + pw - (MENU_PAD + MENU_SCROLLBAR_W) / 2.0,
            py + MENU_PAD,
            MENU_SCROLLBAR_W,
            viewport,
        );
        tree.push(WidgetNode::new(
            format!("top.menu.{key}.scrollbar"),
            track,
            WidgetKind::VScrollbar {
                t: scroll / max_scroll,
                thumb: viewport / natural,
            },
            Some(Interaction {
                event: ToolbarEvent::ScrollTopPopover(scroll),
                kind: HitKind::DragScrollTopPopover { max_scroll },
                tooltip: None,
            }),
        ));
    }
}

fn text_button(
    id: String,
    rect: (f64, f64, f64, f64),
    label: LabelSpec,
    style: ButtonStyle,
    interact: Option<Interaction>,
) -> WidgetNode {
    WidgetNode::new(id, rect, WidgetKind::TextButton { label, style }, interact)
}

/// Stable id suffix for the session action buttons; the row set comes from
/// the shared model.
fn session_button_suffix(event: &ToolbarEvent) -> &'static str {
    match event {
        ToolbarEvent::OpenSession => "open",
        ToolbarEvent::SaveSessionAs => "save-as",
        ToolbarEvent::SessionInfo => "info",
        ToolbarEvent::ClearSession => "clear",
        ToolbarEvent::OpenConfigurator => "manager",
        _ => "button",
    }
}

/// Session popover content: active-session meta labels, the action button
/// grid (replaced by the Save-As overwrite confirmation while one is
/// pending), and the recent-session rows — the Session pane minus its
/// collapsible-card chrome.
fn session_menu_content(snapshot: &ToolbarSnapshot) -> Option<Vec<WidgetNode>> {
    let model = model::ToolbarSessionModel::for_popover(snapshot)?;
    let mut nodes = Vec::new();
    let mut y = 0.0;

    nodes.push(WidgetNode::decor(
        "top.menu.session.name",
        (0.0, y, MENU_CONTENT_W, MENU_NAME_H),
        WidgetKind::Label(LabelSpec::new(
            truncate_middle(&model.active_name, 28),
            MENU_META_FONT,
            true,
        )),
    ));
    y += MENU_NAME_H;
    nodes.push(WidgetNode::decor(
        "top.menu.session.path",
        (0.0, y, MENU_CONTENT_W, MENU_PATH_H),
        WidgetKind::Label(LabelSpec::new(
            truncate_start(&model.active_path_label, 34),
            MENU_PATH_FONT,
            false,
        )),
    ));
    y += MENU_PATH_H + MENU_GAP;

    if let Some(confirmation) = model.overwrite_confirmation.as_ref() {
        nodes.push(WidgetNode::decor(
            "top.menu.session.confirm-label",
            (0.0, y, MENU_CONTENT_W, MENU_CONFIRM_LABEL_H),
            WidgetKind::Label(LabelSpec::new(
                format!("Replace {}?", truncate_middle(&confirmation.label, 20)),
                MENU_META_FONT,
                false,
            )),
        ));
        y += MENU_CONFIRM_LABEL_H;
        let button_w = row_item_width(MENU_CONTENT_W, 2, MENU_GAP);
        let actions = [
            (
                "confirm-replace",
                "Replace",
                confirmation.confirm_event(),
                true,
            ),
            (
                "confirm-cancel",
                "Cancel",
                confirmation.cancel_event(),
                false,
            ),
        ];
        for (index, (suffix, label, event, destructive)) in actions.into_iter().enumerate() {
            nodes.push(text_button(
                format!("top.menu.session.{suffix}"),
                (
                    index as f64 * (button_w + MENU_GAP),
                    y,
                    button_w,
                    MENU_BUTTON_H,
                ),
                LabelSpec::new(label, MENU_LABEL_FONT, true),
                if destructive {
                    ButtonStyle::destructive()
                } else {
                    ButtonStyle::plain()
                },
                Some(Interaction::click(event, Some(label.to_string()))),
            ));
        }
        y += MENU_BUTTON_H;
    } else {
        let columns = model.button_columns();
        let button_w = row_item_width(MENU_CONTENT_W, columns, MENU_GAP);
        let grid = grid_layout(
            0.0,
            y,
            button_w,
            MENU_BUTTON_H,
            MENU_GAP,
            MENU_GAP,
            columns,
            model.buttons.len(),
        );
        for (item, button) in grid.items.iter().zip(model.buttons.iter()) {
            let destructive = matches!(button.event, ToolbarEvent::ClearSession);
            let style = if !button.enabled {
                ButtonStyle::disabled()
            } else if destructive {
                ButtonStyle::destructive()
            } else {
                ButtonStyle::plain()
            };
            nodes.push(text_button(
                format!("top.menu.session.{}", session_button_suffix(&button.event)),
                (item.x, item.y, item.w, item.h),
                LabelSpec::new(button.label, MENU_LABEL_FONT, true),
                style,
                button.enabled.then(|| {
                    Interaction::click(button.event.clone(), Some(button.label.to_string()))
                }),
            ));
        }
        y += grid.height;
    }

    for (index, recent) in model.recents.iter().enumerate() {
        y += MENU_GAP;
        let tooltip_path = recent.path.display().to_string();
        nodes.push(text_button(
            format!("top.menu.session.recent.{index}"),
            (0.0, y, MENU_CONTENT_W, MENU_RECENT_H),
            LabelSpec::new(
                truncate_middle(strip_session_extension(&recent.label), 24),
                MENU_META_FONT,
                false,
            ),
            ButtonStyle::plain(),
            Some(Interaction::click(
                recent.event(),
                Some(format_binding_label(
                    &format!("Open {}", recent.label),
                    Some(tooltip_path.as_str()),
                )),
            )),
        ));
        y += MENU_RECENT_H;
    }

    Some(nodes)
}

/// Settings popover content: the layout-mode segments, the feature-toggle
/// grid, the settings/customize button grid, and the customization
/// sub-panel (group chooser plus per-item show/hide, reorder, and drag
/// rows) — the Settings pane minus its collapsible-card chrome.
fn settings_menu_content(snapshot: &ToolbarSnapshot) -> Option<Vec<WidgetNode>> {
    let model = model::ToolbarSettingsModel::for_popover(snapshot)?;
    let customizing = snapshot.customize_items_open;
    let mut nodes = Vec::new();
    let mut y = 0.0;

    if !customizing {
        let control = model::layout_mode_control(snapshot.layout_mode);
        if let model::ToolbarControlKind::Segmented(segmented) = &control.kind {
            let segments = segmented.segments();
            if !segments.is_empty() {
                let active = segmented.active_segment();
                let segment_w = row_item_width(MENU_CONTENT_W, segments.len(), MENU_ITEM_GAP);
                for (index, segment) in segments.iter().enumerate() {
                    nodes.push(text_button(
                        format!("top.menu.settings.mode.{index}"),
                        (
                            index as f64 * (segment_w + MENU_ITEM_GAP),
                            y,
                            segment_w,
                            MENU_SEGMENT_H,
                        ),
                        LabelSpec::new(segment.label.as_ref(), MENU_LABEL_FONT, true),
                        ButtonStyle::active(active == Some(segment.id)),
                        Some(Interaction::click(
                            segment.activation.compatibility_event(),
                            segment.tooltip.as_string(),
                        )),
                    ));
                }
                y += MENU_SEGMENT_H + MENU_TOGGLE_GAP;
            }
        }
    }

    let toggle_w = row_item_width(MENU_CONTENT_W, 2, MENU_TOGGLE_GAP);
    let mut toggle_index = 0usize;
    for row in model.toggle_rows() {
        let full_row = row.len() == 1 && row[0].wide;
        for (col, toggle) in row.into_iter().enumerate() {
            let (cell_x, cell_w) = if full_row {
                (0.0, MENU_CONTENT_W)
            } else {
                (col as f64 * (toggle_w + MENU_TOGGLE_GAP), toggle_w)
            };
            nodes.push(WidgetNode::new(
                format!("top.menu.settings.toggle.{toggle_index}"),
                (cell_x, y, cell_w, MENU_TOGGLE_H),
                WidgetKind::Checkbox {
                    checked: toggle.checked,
                    label: LabelSpec::new(toggle.label.as_ref(), MENU_META_FONT, true),
                },
                Some(Interaction::click(
                    toggle.activation.compatibility_event(),
                    toggle.tooltip.as_string(),
                )),
            ));
            toggle_index += 1;
        }
        y += MENU_TOGGLE_H + MENU_TOGGLE_GAP;
    }

    let buttons = model.buttons();
    if !buttons.is_empty() {
        let button_w = row_item_width(MENU_CONTENT_W, 2, MENU_TOGGLE_GAP);
        let grid = grid_layout(
            0.0,
            y,
            button_w,
            MENU_BUTTON_H,
            MENU_TOGGLE_GAP,
            MENU_TOGGLE_GAP,
            2,
            buttons.len(),
        );
        for (index, (item, button)) in grid.items.iter().zip(buttons.iter()).enumerate() {
            nodes.push(text_button(
                format!("top.menu.settings.button.{index}"),
                (item.x, item.y, item.w, item.h),
                LabelSpec::new(button.label.as_ref(), MENU_LABEL_FONT, true),
                ButtonStyle::plain(),
                Some(Interaction::click(
                    button.event.clone(),
                    button.tooltip.as_string(),
                )),
            ));
        }
        y += grid.height + MENU_TOGGLE_GAP;
    }

    let groups = model.groups();
    if !groups.is_empty() {
        nodes.push(WidgetNode::decor(
            "top.menu.settings.groups-header",
            (0.0, y, MENU_CONTENT_W, MENU_HEADER_H),
            WidgetKind::Label(LabelSpec::new("Choose a group", MENU_LABEL_FONT, true)),
        ));
        y += MENU_HEADER_H + MENU_ITEM_GAP;
        let group_w = row_item_width(MENU_CONTENT_W, 2, MENU_TOGGLE_GAP);
        let grid = grid_layout(
            0.0,
            y,
            group_w,
            MENU_BUTTON_H,
            MENU_TOGGLE_GAP,
            MENU_TOGGLE_GAP,
            2,
            groups.len(),
        );
        for (index, (item, group)) in grid.items.iter().zip(groups.iter()).enumerate() {
            nodes.push(text_button(
                format!("top.menu.settings.group.{index}"),
                (item.x, item.y, item.w, item.h),
                LabelSpec::new(group.label.as_ref(), MENU_LABEL_FONT, true),
                ButtonStyle::plain(),
                Some(Interaction::click(
                    group.event.clone(),
                    group.tooltip.as_string(),
                )),
            ));
        }
        y += grid.height + MENU_TOGGLE_GAP;
    }

    let overrides = model.item_overrides();
    if !overrides.is_empty() {
        let header = snapshot
            .customize_items_group
            .map_or("Uncheck items to hide", |group| group.label());
        nodes.push(WidgetNode::decor(
            "top.menu.settings.items-header",
            (0.0, y, MENU_CONTENT_W, MENU_HEADER_H),
            WidgetKind::Label(LabelSpec::new(header, MENU_LABEL_FONT, true)),
        ));
        y += MENU_HEADER_H + MENU_ITEM_GAP;
        for (index, item) in overrides.iter().enumerate() {
            let order = item.order.as_ref();
            if let Some(order) = order {
                // Full-row drag region under the row's controls: the tree
                // hit-tester is topmost-first, so the checkbox and move
                // buttons pushed after it keep their clicks.
                nodes.push(WidgetNode::new(
                    format!("top.menu.settings.item.{index}.drag"),
                    (0.0, y, MENU_CONTENT_W, MENU_TOGGLE_H),
                    WidgetKind::HitArea,
                    Some(Interaction {
                        event: ToolbarEvent::StartToolbarItemDrag {
                            group: order.group,
                            id: item.id,
                        },
                        kind: HitKind::DragToolbarItem {
                            group: order.group,
                            id: item.id,
                            target_index: order.index,
                        },
                        tooltip: Some(format!("Drag {} to reorder", item.label)),
                    }),
                ));
                nodes.push(WidgetNode::decor(
                    format!("top.menu.settings.item.{index}.handle"),
                    (0.0, y, MENU_ITEM_HANDLE_W, MENU_TOGGLE_H),
                    WidgetKind::TextButton {
                        label: LabelSpec::new("=", MENU_META_FONT, true),
                        style: ButtonStyle::plain(),
                    },
                ));
            }
            let handle_w = if order.is_some() {
                MENU_ITEM_HANDLE_W + MENU_ITEM_GAP
            } else {
                0.0
            };
            let moves_w = if order.is_some() {
                MENU_ITEM_MOVE_W * 2.0 + MENU_ITEM_GAP * 2.0
            } else {
                0.0
            };
            nodes.push(WidgetNode::new(
                format!("top.menu.settings.item.{index}"),
                (
                    handle_w,
                    y,
                    MENU_CONTENT_W - handle_w - moves_w,
                    MENU_TOGGLE_H,
                ),
                WidgetKind::Checkbox {
                    checked: item.shown,
                    label: LabelSpec::new(item.label.as_ref(), MENU_META_FONT, true),
                },
                Some(Interaction::click(
                    item.activation.compatibility_event(),
                    item.tooltip.as_string(),
                )),
            ));
            if let Some(order) = order {
                let up_x = MENU_CONTENT_W - MENU_ITEM_MOVE_W * 2.0 - MENU_ITEM_GAP;
                let down_x = MENU_CONTENT_W - MENU_ITEM_MOVE_W;
                for (suffix, x, glyph, enabled, activation, tooltip) in [
                    (
                        "up",
                        up_x,
                        "^",
                        order.can_move_up,
                        &order.move_up,
                        "Move up",
                    ),
                    (
                        "down",
                        down_x,
                        "v",
                        order.can_move_down,
                        &order.move_down,
                        "Move down",
                    ),
                ] {
                    nodes.push(text_button(
                        format!("top.menu.settings.item.{index}.{suffix}"),
                        (x, y, MENU_ITEM_MOVE_W, MENU_TOGGLE_H),
                        LabelSpec::new(glyph, MENU_META_FONT, true),
                        if enabled {
                            ButtonStyle::plain()
                        } else {
                            ButtonStyle::disabled()
                        },
                        enabled.then(|| {
                            Interaction::click(
                                activation.compatibility_event(),
                                Some(format!("{} {}", tooltip, item.label)),
                            )
                        }),
                    ));
                }
            }
            y += MENU_TOGGLE_H + MENU_ITEM_GAP;
        }
    }

    Some(nodes)
}

/// Stable id suffix for a Canvas-popover command button; the row set comes
/// from the shared command-group models.
fn canvas_button_suffix(event: &ToolbarEvent) -> &'static str {
    match event {
        ToolbarEvent::BoardPrev => "board-prev",
        ToolbarEvent::BoardNext => "board-next",
        ToolbarEvent::BoardNew => "board-new",
        ToolbarEvent::BoardDuplicate => "board-duplicate",
        ToolbarEvent::BoardDelete => "board-delete",
        ToolbarEvent::PagePrev => "page-prev",
        ToolbarEvent::PageNext => "page-next",
        ToolbarEvent::PageNew => "page-new",
        ToolbarEvent::PageDuplicate => "page-duplicate",
        ToolbarEvent::PageDelete => "page-delete",
        ToolbarEvent::ZoomIn => "zoom-in",
        ToolbarEvent::ZoomOut => "zoom-out",
        ToolbarEvent::ResetZoom => "reset-zoom",
        ToolbarEvent::ToggleZoomLock => "zoom-lock",
        ToolbarEvent::UndoAll => "undo-all",
        ToolbarEvent::RedoAll => "redo-all",
        ToolbarEvent::UndoAllDelayed => "undo-all-delay",
        ToolbarEvent::RedoAllDelayed => "redo-all-delay",
        ToolbarEvent::ToggleFreeze => "freeze",
        _ => "button",
    }
}

/// One Canvas-popover command section: a bold section header over a single
/// row of the group's buttons, laid out edge-to-edge like the side pane's
/// command rows. Advances `y` past the rendered rows.
fn push_canvas_command_section(
    nodes: &mut Vec<WidgetNode>,
    y: &mut f64,
    snapshot: &ToolbarSnapshot,
    key: &str,
    title: &str,
    noun: &'static str,
    group: &model::ToolbarCommandGroup,
) {
    nodes.push(WidgetNode::decor(
        format!("top.menu.canvas.{key}.header"),
        (0.0, *y, CANVAS_MENU_CONTENT_W, MENU_HEADER_H),
        WidgetKind::Label(LabelSpec::new(title, MENU_LABEL_FONT, true)),
    ));
    *y += MENU_HEADER_H + MENU_ITEM_GAP;

    let columns = group.buttons.len().clamp(1, 5);
    let button_w = row_item_width(CANVAS_MENU_CONTENT_W, columns, MENU_GAP);
    let grid = grid_layout(
        0.0,
        *y,
        button_w,
        MENU_BUTTON_H,
        MENU_GAP,
        MENU_GAP,
        columns,
        group.buttons.len(),
    );
    for (item, button) in grid.items.iter().zip(group.buttons.iter()) {
        let label = button.short_label(snapshot, noun);
        let style = if !button.enabled {
            ButtonStyle::disabled()
        } else if button.event.is_destructive() {
            ButtonStyle::destructive()
        } else {
            ButtonStyle::plain()
        };
        let tooltip = format_binding_label(
            button.tooltip_label(snapshot, noun),
            button.binding_hint(snapshot),
        );
        nodes.push(text_button(
            format!(
                "top.menu.canvas.{key}.{}",
                canvas_button_suffix(&button.event)
            ),
            (item.x, item.y, item.w, item.h),
            LabelSpec::new(label, MENU_LABEL_FONT, true),
            style,
            button
                .enabled
                .then(|| Interaction::click(button.event.clone(), Some(tooltip))),
        ));
    }
    *y += grid.height;
}

/// One Step Undo/Redo row: the multi-step button, the −/count/+ stepper
/// cluster, and the per-direction delay slider beneath it.
fn push_canvas_step_row(
    nodes: &mut Vec<WidgetNode>,
    y: &mut f64,
    snapshot: &ToolbarSnapshot,
    is_undo: bool,
) {
    let side = if is_undo { "undo" } else { "redo" };
    let steps = if is_undo {
        snapshot.custom_undo_steps
    } else {
        snapshot.custom_redo_steps
    };
    let delay_ms = if is_undo {
        snapshot.custom_undo_delay_ms
    } else {
        snapshot.custom_redo_delay_ms
    };

    let row_y = *y;
    // Multi-step button.
    nodes.push(text_button(
        format!("top.menu.canvas.step.{side}.apply"),
        (0.0, row_y, CANVAS_STEP_BTN_W, CANVAS_STEP_ROW_H),
        LabelSpec::new(
            if is_undo { "Step Undo" } else { "Step Redo" },
            MENU_META_FONT,
            true,
        ),
        ButtonStyle::plain(),
        Some(Interaction::click(
            if is_undo {
                ToolbarEvent::CustomUndo
            } else {
                ToolbarEvent::CustomRedo
            },
            Some(if is_undo { "Step undo" } else { "Step redo" }.to_string()),
        )),
    ));

    let minus_x = CANVAS_STEP_BTN_W + MENU_GAP;
    nodes.push(text_button(
        format!("top.menu.canvas.step.{side}.minus"),
        (minus_x, row_y, CANVAS_STEPPER_W, CANVAS_STEP_ROW_H),
        LabelSpec::new("−", MENU_META_FONT, true),
        ButtonStyle::plain(),
        Some(Interaction::click(
            set_custom_steps_event(is_undo, steps.saturating_sub(1).max(1)),
            Some(format!("Decrease {side} steps")),
        )),
    ));

    let label_x = minus_x + CANVAS_STEPPER_W + MENU_ITEM_GAP;
    nodes.push(WidgetNode::decor(
        format!("top.menu.canvas.step.{side}.count"),
        (label_x, row_y, CANVAS_STEPS_LABEL_W, CANVAS_STEP_ROW_H),
        WidgetKind::Label(LabelSpec::new(
            format!("{steps} steps"),
            MENU_META_FONT,
            true,
        )),
    ));

    let plus_x = label_x + CANVAS_STEPS_LABEL_W + MENU_ITEM_GAP;
    nodes.push(text_button(
        format!("top.menu.canvas.step.{side}.plus"),
        (plus_x, row_y, CANVAS_STEPPER_W, CANVAS_STEP_ROW_H),
        LabelSpec::new("+", MENU_META_FONT, true),
        ButtonStyle::plain(),
        Some(Interaction::click(
            set_custom_steps_event(is_undo, steps.saturating_add(1)),
            Some(format!("Increase {side} steps")),
        )),
    ));
    *y += CANVAS_STEP_ROW_H + MENU_GAP;

    // Per-direction delay slider.
    push_canvas_delay_slider(
        nodes,
        y,
        format!("top.menu.canvas.step.{side}.delay"),
        delay_ms,
        if is_undo {
            ToolbarEvent::SetCustomUndoDelay(delay_ms as f64 / 1000.0)
        } else {
            ToolbarEvent::SetCustomRedoDelay(delay_ms as f64 / 1000.0)
        },
        if is_undo {
            HitKind::DragCustomUndoDelay
        } else {
            HitKind::DragCustomRedoDelay
        },
        format!(
            "{} step delay: {:.1}s (drag)",
            if is_undo { "Undo" } else { "Redo" },
            delay_ms as f64 / 1000.0
        ),
    );
}

fn set_custom_steps_event(is_undo: bool, steps: usize) -> ToolbarEvent {
    if is_undo {
        ToolbarEvent::SetCustomUndoSteps(steps)
    } else {
        ToolbarEvent::SetCustomRedoSteps(steps)
    }
}

/// A full-width delay slider node whose drag maps pointer x → seconds via the
/// shared `DELAY_SECONDS` spec (the tree hit-tester reads the node rect).
fn push_canvas_delay_slider(
    nodes: &mut Vec<WidgetNode>,
    y: &mut f64,
    id: String,
    delay_ms: u64,
    event: ToolbarEvent,
    kind: HitKind,
    tooltip: String,
) {
    let t = model::delay_t_from_ms(delay_ms);
    nodes.push(WidgetNode::new(
        id,
        (0.0, *y, CANVAS_MENU_CONTENT_W, CANVAS_SLIDER_H),
        WidgetKind::Slider { t },
        Some(Interaction {
            event,
            kind,
            tooltip: Some(tooltip),
        }),
    ));
    *y += CANVAS_SLIDER_H + MENU_GAP;
}

/// Step Undo/Redo configuration section: the "Step buttons" / "Delay
/// sliders" toggles, the per-direction step rows (when Step buttons are on),
/// and the global undo/redo delay sliders (when Delay sliders are on).
fn push_canvas_step_section(nodes: &mut Vec<WidgetNode>, y: &mut f64, snapshot: &ToolbarSnapshot) {
    nodes.push(WidgetNode::decor(
        "top.menu.canvas.step.header",
        (0.0, *y, CANVAS_MENU_CONTENT_W, MENU_HEADER_H),
        WidgetKind::Label(LabelSpec::new("Step Undo/Redo", MENU_LABEL_FONT, true)),
    ));
    *y += MENU_HEADER_H + MENU_ITEM_GAP;

    nodes.push(WidgetNode::new(
        "top.menu.canvas.step.toggle.buttons",
        (0.0, *y, CANVAS_MENU_CONTENT_W, MENU_TOGGLE_H),
        WidgetKind::Checkbox {
            checked: snapshot.custom_section_enabled,
            label: LabelSpec::new("Step buttons", MENU_META_FONT, true),
        },
        Some(Interaction::click(
            ToolbarEvent::ToggleCustomSection(!snapshot.custom_section_enabled),
            Some("Step buttons: undo/redo several strokes at once.".to_string()),
        )),
    ));
    *y += MENU_TOGGLE_H + MENU_TOGGLE_GAP;

    nodes.push(WidgetNode::new(
        "top.menu.canvas.step.toggle.delays",
        (0.0, *y, CANVAS_MENU_CONTENT_W, MENU_TOGGLE_H),
        WidgetKind::Checkbox {
            checked: snapshot.show_delay_sliders,
            label: LabelSpec::new("Delay sliders", MENU_META_FONT, true),
        },
        Some(Interaction::click(
            ToolbarEvent::ToggleDelaySliders(!snapshot.show_delay_sliders),
            Some("Delay sliders: undo/redo delays.".to_string()),
        )),
    ));
    *y += MENU_TOGGLE_H + MENU_TOGGLE_GAP;

    if snapshot.custom_section_enabled {
        push_canvas_step_row(nodes, y, snapshot, true);
        push_canvas_step_row(nodes, y, snapshot, false);
    }

    if snapshot.show_delay_sliders {
        for (is_undo, delay_ms) in [
            (true, snapshot.undo_all_delay_ms),
            (false, snapshot.redo_all_delay_ms),
        ] {
            let side = if is_undo { "undo" } else { "redo" };
            nodes.push(WidgetNode::decor(
                format!("top.menu.canvas.step.global.{side}.label"),
                (0.0, *y, CANVAS_MENU_CONTENT_W, MENU_PATH_H),
                WidgetKind::Label(LabelSpec::new(
                    format!(
                        "{} delay: {:.1}s",
                        if is_undo { "Undo" } else { "Redo" },
                        delay_ms as f64 / 1000.0
                    ),
                    MENU_META_FONT,
                    false,
                )),
            ));
            *y += MENU_PATH_H + MENU_ITEM_GAP;
            push_canvas_delay_slider(
                nodes,
                y,
                format!("top.menu.canvas.step.global.{side}.slider"),
                delay_ms,
                if is_undo {
                    ToolbarEvent::SetUndoDelay(delay_ms as f64 / 1000.0)
                } else {
                    ToolbarEvent::SetRedoDelay(delay_ms as f64 / 1000.0)
                },
                if is_undo {
                    HitKind::DragUndoDelay
                } else {
                    HitKind::DragRedoDelay
                },
                format!(
                    "{}-all delay: {:.1}s (drag)",
                    if is_undo { "Undo" } else { "Redo" },
                    delay_ms as f64 / 1000.0
                ),
            );
        }
    }
}

/// Canvas popover content: the re-homed side Canvas pane minus its
/// collapsible-card chrome — the Boards, Pages, Advanced, and Zoom command
/// sections (each gated on its display toggle) followed by the Step
/// Undo/Redo configuration. `None` when every section is toggled off.
fn canvas_menu_content(snapshot: &ToolbarSnapshot) -> Option<Vec<WidgetNode>> {
    let mut nodes = Vec::new();
    let mut y = 0.0;
    let mut rendered = false;

    let sections = [
        (
            "boards",
            "Boards",
            "Board",
            model::toolbar_boards_model_for_popover(snapshot),
        ),
        (
            "pages",
            "Pages",
            "Page",
            model::toolbar_pages_model_for_popover(snapshot),
        ),
        (
            "advanced",
            "Advanced",
            "Action",
            model::toolbar_advanced_group_for_popover(snapshot),
        ),
        (
            "zoom",
            "Zoom",
            "Zoom",
            model::toolbar_zoom_group_for_popover(snapshot),
        ),
    ];
    for (key, title, noun, group) in sections {
        let Some(group) = group else { continue };
        if rendered {
            y += CANVAS_SECTION_GAP;
        }
        push_canvas_command_section(&mut nodes, &mut y, snapshot, key, title, noun, &group);
        rendered = true;
    }

    if snapshot.show_step_section {
        if rendered {
            y += CANVAS_SECTION_GAP;
        }
        push_canvas_step_section(&mut nodes, &mut y, snapshot);
        rendered = true;
    }

    rendered.then_some(nodes)
}
