//! Session/Settings popovers anchored to the top strip's overflow toggle.
//!
//! With `[ui.toolbar] side_layout = "pill"` the side palette is retired, so
//! the Session and Settings panes are re-hosted here as popovers opened from
//! the overflow menu's "Session..." / "Settings..." entries. The content
//! reuses the same renderer-neutral models the side panes render
//! (`ToolbarSessionModel` / `ToolbarSettingsModel`), so both surfaces expose
//! the same controls emitting the same events — minus the side palette's
//! chrome (pane tabs, collapsible section headers).
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
const MENU_CONTENT_W: f64 = crate::ui::theme::toolbar::MENU_CONTENT_W;
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
const MENU_LABEL_FONT: f64 = 12.0;
const MENU_META_FONT: f64 = 11.0;
const MENU_PATH_FONT: f64 = 9.5;
const MENU_ITEM_HANDLE_W: f64 = 24.0;
const MENU_ITEM_MOVE_W: f64 = 28.0;
const MENU_ITEM_GAP: f64 = 4.0;
/// Gap between the anchor and the popover panel plus the caret allowance
/// under it (mirrors `overflow_height`'s trailing margins).
const MENU_ANCHOR_GAP: f64 = 6.0;
const MENU_BOTTOM_MARGIN: f64 = 4.0;

/// Content nodes for whichever popover is open, in content-local
/// coordinates (origin at the top-left of the content column), plus the
/// natural (unclamped) content height. Session wins if both flags are
/// somehow set — the apply layer keeps them mutually exclusive.
fn open_menu_content(snapshot: &ToolbarSnapshot) -> Option<(&'static str, Vec<WidgetNode>)> {
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

/// Scroll bounds for the open Session/Settings popover as
/// (natural_height, viewport_height), both in pre-scale spec units; `None`
/// while neither popover is open. Max scroll = (natural - viewport).max(0).
pub(super) fn menu_scroll_bounds(snapshot: &ToolbarSnapshot) -> Option<(f64, f64)> {
    let (_, nodes) = open_menu_content(snapshot)?;
    let natural = content_height(&nodes);
    Some((natural, natural.min(MENU_MAX_CONTENT_H)))
}

/// Extra surface height the open Session/Settings popover needs below the
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

    let placement = popover::place_popover(popover::PopoverSpec {
        anchor,
        content: (MENU_CONTENT_W + MENU_PAD * 2.0, viewport + MENU_PAD * 2.0),
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
