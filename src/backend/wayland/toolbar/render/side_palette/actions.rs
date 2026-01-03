use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::input::ToolbarDrawerTab;
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;

use super::super::widgets::*;

pub(super) fn draw_actions_section(layout: &mut SidePaletteLayout, y: &mut f64) {
    let ctx = layout.ctx;
    let snapshot = layout.snapshot;
    let hits = &mut layout.hits;
    let hover = layout.hover;
    let x = layout.x;
    let card_x = layout.card_x;
    let card_w = layout.card_w;
    let content_width = layout.content_width;
    let section_gap = layout.section_gap;
    let use_icons = snapshot.use_icons;

    let show_drawer_view = snapshot.drawer_open && snapshot.drawer_tab == ToolbarDrawerTab::View;
    let show_advanced = snapshot.show_actions_advanced && show_drawer_view;
    let show_view_actions =
        show_drawer_view && (snapshot.show_actions_section || snapshot.show_actions_advanced);
    let show_actions = snapshot.show_actions_section || show_advanced;
    if !show_actions {
        return;
    }

    let mut actions_snapshot = snapshot.clone();
    actions_snapshot.show_actions_advanced = show_advanced;
    let actions_card_h = layout.spec.side_actions_height(&actions_snapshot);
    draw_group_card(ctx, card_x, *y, card_w, actions_card_h);
    draw_section_label(
        ctx,
        x,
        *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
        "Actions",
    );

    let actions_y = *y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    type IconFn = fn(&cairo::Context, f64, f64, f64);
    let basic_actions: &[(ToolbarEvent, IconFn, &str, bool)] = &[
        (
            ToolbarEvent::Undo,
            toolbar_icons::draw_icon_undo as IconFn,
            "Undo",
            snapshot.undo_available,
        ),
        (
            ToolbarEvent::Redo,
            toolbar_icons::draw_icon_redo as IconFn,
            "Redo",
            snapshot.redo_available,
        ),
        (
            ToolbarEvent::ClearCanvas,
            toolbar_icons::draw_icon_clear as IconFn,
            "Clear",
            true,
        ),
    ];
    let lock_label = if snapshot.zoom_locked {
        "Unlock Zoom"
    } else {
        "Lock Zoom"
    };
    let view_actions: Vec<(ToolbarEvent, IconFn, &str, bool)> = vec![
        (
            ToolbarEvent::ZoomIn,
            toolbar_icons::draw_icon_zoom_in as IconFn,
            "Zoom In",
            true,
        ),
        (
            ToolbarEvent::ZoomOut,
            toolbar_icons::draw_icon_zoom_out as IconFn,
            "Zoom Out",
            true,
        ),
        (
            ToolbarEvent::ResetZoom,
            toolbar_icons::draw_icon_zoom_reset as IconFn,
            "Reset Zoom",
            snapshot.zoom_active,
        ),
        (
            ToolbarEvent::ToggleZoomLock,
            if snapshot.zoom_locked {
                toolbar_icons::draw_icon_lock as IconFn
            } else {
                toolbar_icons::draw_icon_unlock as IconFn
            },
            lock_label,
            snapshot.zoom_active,
        ),
    ];
    let show_delay_actions = show_advanced && snapshot.delay_actions_enabled;
    let mut advanced_actions: Vec<(ToolbarEvent, IconFn, &str, bool)> = Vec::new();
    advanced_actions.push((
        ToolbarEvent::UndoAll,
        toolbar_icons::draw_icon_undo_all as IconFn,
        "Undo All",
        snapshot.undo_available,
    ));
    advanced_actions.push((
        ToolbarEvent::RedoAll,
        toolbar_icons::draw_icon_redo_all as IconFn,
        "Redo All",
        snapshot.redo_available,
    ));
    if show_delay_actions {
        advanced_actions.push((
            ToolbarEvent::UndoAllDelayed,
            toolbar_icons::draw_icon_undo_all_delay as IconFn,
            "Undo All Delay",
            snapshot.undo_available,
        ));
        advanced_actions.push((
            ToolbarEvent::RedoAllDelayed,
            toolbar_icons::draw_icon_redo_all_delay as IconFn,
            "Redo All Delay",
            snapshot.redo_available,
        ));
    }
    advanced_actions.push((
        ToolbarEvent::ToggleFreeze,
        if snapshot.frozen_active {
            toolbar_icons::draw_icon_unfreeze as IconFn
        } else {
            toolbar_icons::draw_icon_freeze as IconFn
        },
        if snapshot.frozen_active {
            "Unfreeze"
        } else {
            "Freeze"
        },
        true,
    ));

    if use_icons {
        let icon_btn_size = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON;
        let icon_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
        let icon_size = ToolbarLayoutSpec::SIDE_ACTION_ICON_SIZE;
        let mut action_y = actions_y;
        let mut has_group = false;

        if snapshot.show_actions_section {
            let icons_per_row = basic_actions.len();
            let total_icons = icons_per_row;
            let row_width =
                total_icons as f64 * icon_btn_size + (total_icons as f64 - 1.0) * icon_gap;
            let row_x = x + (content_width - row_width) / 2.0;
            for (idx, (evt, icon_fn, label, enabled)) in basic_actions.iter().enumerate() {
                let bx = row_x + (icon_btn_size + icon_gap) * idx as f64;
                let by = action_y;
                let is_hover = hover
                    .map(|(hx, hy)| point_in_rect(hx, hy, bx, by, icon_btn_size, icon_btn_size))
                    .unwrap_or(false);
                if *enabled {
                    draw_button(ctx, bx, by, icon_btn_size, icon_btn_size, false, is_hover);
                    set_icon_color(ctx, is_hover);
                } else {
                    draw_button(ctx, bx, by, icon_btn_size, icon_btn_size, false, false);
                    ctx.set_source_rgba(0.5, 0.5, 0.55, 0.5);
                }
                let icon_x = bx + (icon_btn_size - icon_size) / 2.0;
                let icon_y = by + (icon_btn_size - icon_size) / 2.0;
                icon_fn(ctx, icon_x, icon_y, icon_size);
                hits.push(HitRegion {
                    rect: (bx, by, icon_btn_size, icon_btn_size),
                    event: evt.clone(),
                    kind: HitKind::Click,
                    tooltip: Some(format_binding_label(
                        label,
                        snapshot.binding_hints.binding_for_event(evt),
                    )),
                });
            }
            action_y += icon_btn_size;
            has_group = true;
        }

        if show_view_actions {
            if has_group {
                action_y += icon_gap;
            }
            let icons_per_row = 5usize;
            let total_icons = view_actions.len();
            let rows = if total_icons > 0 {
                total_icons.div_ceil(icons_per_row)
            } else {
                0
            };
            for row in 0..rows {
                let row_start = row * icons_per_row;
                let row_end = (row_start + icons_per_row).min(total_icons);
                let icons_in_row = row_end - row_start;
                let row_width =
                    icons_in_row as f64 * icon_btn_size + (icons_in_row as f64 - 1.0) * icon_gap;
                let row_x = x + (content_width - row_width) / 2.0;
                for col in 0..icons_in_row {
                    let idx = row_start + col;
                    let (evt, icon_fn, label, enabled) = &view_actions[idx];
                    let bx = row_x + (icon_btn_size + icon_gap) * col as f64;
                    let by = action_y + (icon_btn_size + icon_gap) * row as f64;
                    let is_hover = hover
                        .map(|(hx, hy)| point_in_rect(hx, hy, bx, by, icon_btn_size, icon_btn_size))
                        .unwrap_or(false);
                    if *enabled {
                        draw_button(ctx, bx, by, icon_btn_size, icon_btn_size, false, is_hover);
                        set_icon_color(ctx, is_hover);
                    } else {
                        draw_button(ctx, bx, by, icon_btn_size, icon_btn_size, false, false);
                        ctx.set_source_rgba(0.5, 0.5, 0.55, 0.5);
                    }
                    let icon_x = bx + (icon_btn_size - icon_size) / 2.0;
                    let icon_y = by + (icon_btn_size - icon_size) / 2.0;
                    icon_fn(ctx, icon_x, icon_y, icon_size);
                    hits.push(HitRegion {
                        rect: (bx, by, icon_btn_size, icon_btn_size),
                        event: evt.clone(),
                        kind: HitKind::Click,
                        tooltip: Some(format_binding_label(
                            label,
                            snapshot.binding_hints.binding_for_event(evt),
                        )),
                    });
                }
            }
            if rows > 0 {
                action_y += icon_btn_size * rows as f64 + icon_gap * (rows as f64 - 1.0);
                has_group = true;
            }
        }

        if show_advanced {
            if has_group {
                action_y += icon_gap;
            }
            let icons_per_row = 5usize;
            let total_icons = advanced_actions.len();
            let rows = if total_icons > 0 {
                total_icons.div_ceil(icons_per_row)
            } else {
                0
            };
            for row in 0..rows {
                let row_start = row * icons_per_row;
                let row_end = (row_start + icons_per_row).min(total_icons);
                let icons_in_row = row_end - row_start;
                let row_width =
                    icons_in_row as f64 * icon_btn_size + (icons_in_row as f64 - 1.0) * icon_gap;
                let row_x = x + (content_width - row_width) / 2.0;
                for col in 0..icons_in_row {
                    let idx = row_start + col;
                    let (evt, icon_fn, label, enabled) = &advanced_actions[idx];
                    let bx = row_x + (icon_btn_size + icon_gap) * col as f64;
                    let by = action_y + (icon_btn_size + icon_gap) * row as f64;
                    let is_hover = hover
                        .map(|(hx, hy)| point_in_rect(hx, hy, bx, by, icon_btn_size, icon_btn_size))
                        .unwrap_or(false);
                    if *enabled {
                        draw_button(ctx, bx, by, icon_btn_size, icon_btn_size, false, is_hover);
                        set_icon_color(ctx, is_hover);
                    } else {
                        draw_button(ctx, bx, by, icon_btn_size, icon_btn_size, false, false);
                        ctx.set_source_rgba(0.5, 0.5, 0.55, 0.5);
                    }
                    let icon_x = bx + (icon_btn_size - icon_size) / 2.0;
                    let icon_y = by + (icon_btn_size - icon_size) / 2.0;
                    icon_fn(ctx, icon_x, icon_y, icon_size);
                    hits.push(HitRegion {
                        rect: (bx, by, icon_btn_size, icon_btn_size),
                        event: evt.clone(),
                        kind: HitKind::Click,
                        tooltip: Some(format_binding_label(
                            label,
                            snapshot.binding_hints.binding_for_event(evt),
                        )),
                    });
                }
            }
        }
    } else {
        let action_h = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT;
        let action_gap = ToolbarLayoutSpec::SIDE_ACTION_CONTENT_GAP_TEXT;
        let action_col_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
        let action_w = (content_width - action_col_gap) / 2.0;
        let mut action_y = actions_y;
        let mut has_group = false;

        if snapshot.show_actions_section {
            for (idx, (evt, _icon, label, enabled)) in basic_actions.iter().enumerate() {
                let by = action_y + (action_h + action_gap) * idx as f64;
                let is_hover = hover
                    .map(|(hx, hy)| point_in_rect(hx, hy, x, by, content_width, action_h))
                    .unwrap_or(false);
                draw_button(ctx, x, by, content_width, action_h, *enabled, is_hover);
                draw_label_center(ctx, x, by, content_width, action_h, label);
                if *enabled {
                    hits.push(HitRegion {
                        rect: (x, by, content_width, action_h),
                        event: evt.clone(),
                        kind: HitKind::Click,
                        tooltip: Some(format_binding_label(
                            label,
                            snapshot.binding_hints.binding_for_event(evt),
                        )),
                    });
                }
            }
            action_y += action_h * basic_actions.len() as f64
                + action_gap * (basic_actions.len() as f64 - 1.0);
            has_group = true;
        }

        if show_view_actions {
            if has_group {
                action_y += ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
            }
            for (idx, (evt, _icon, label, enabled)) in view_actions.iter().enumerate() {
                let row = idx / 2;
                let col = idx % 2;
                let bx = x + (action_w + action_col_gap) * col as f64;
                let by = action_y + (action_h + action_gap) * row as f64;
                let is_hover = hover
                    .map(|(hx, hy)| point_in_rect(hx, hy, bx, by, action_w, action_h))
                    .unwrap_or(false);
                draw_button(ctx, bx, by, action_w, action_h, *enabled, is_hover);
                draw_label_center(ctx, bx, by, action_w, action_h, label);
                if *enabled {
                    hits.push(HitRegion {
                        rect: (bx, by, action_w, action_h),
                        event: evt.clone(),
                        kind: HitKind::Click,
                        tooltip: Some(format_binding_label(
                            label,
                            snapshot.binding_hints.binding_for_event(evt),
                        )),
                    });
                }
            }
            let rows = view_actions.len().div_ceil(2);
            if rows > 0 {
                action_y += action_h * rows as f64 + action_gap * (rows as f64 - 1.0);
                has_group = true;
            }
        }

        if show_advanced {
            if has_group {
                action_y += ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
            }
            for (idx, (evt, _icon, label, enabled)) in advanced_actions.iter().enumerate() {
                let row = idx / 2;
                let col = idx % 2;
                let bx = x + (action_w + action_col_gap) * col as f64;
                let by = action_y + (action_h + action_gap) * row as f64;
                let is_hover = hover
                    .map(|(hx, hy)| point_in_rect(hx, hy, bx, by, action_w, action_h))
                    .unwrap_or(false);
                draw_button(ctx, bx, by, action_w, action_h, *enabled, is_hover);
                draw_label_center(ctx, bx, by, action_w, action_h, label);
                if *enabled {
                    hits.push(HitRegion {
                        rect: (bx, by, action_w, action_h),
                        event: evt.clone(),
                        kind: HitKind::Click,
                        tooltip: Some(format_binding_label(
                            label,
                            snapshot.binding_hints.binding_for_event(evt),
                        )),
                    });
                }
            }
        }
    }

    *y += actions_card_h + section_gap;
}
