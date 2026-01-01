use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
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

    let show_actions = snapshot.show_actions_section || snapshot.show_actions_advanced;
    if !show_actions {
        return;
    }

    let actions_card_h = layout.spec.side_actions_height(snapshot);
    draw_group_card(ctx, card_x, *y, card_w, actions_card_h);
    draw_section_label(
        ctx,
        x,
        *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
        "Actions",
    );

    let mut actions_y = *y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
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
    let show_delay_actions = snapshot.show_step_section && snapshot.show_delay_sliders;
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
    advanced_actions.push((
        ToolbarEvent::ZoomIn,
        toolbar_icons::draw_icon_zoom_in as IconFn,
        "Zoom In",
        true,
    ));
    advanced_actions.push((
        ToolbarEvent::ZoomOut,
        toolbar_icons::draw_icon_zoom_out as IconFn,
        "Zoom Out",
        true,
    ));
    advanced_actions.push((
        ToolbarEvent::ResetZoom,
        toolbar_icons::draw_icon_zoom_reset as IconFn,
        "Reset Zoom",
        snapshot.zoom_active,
    ));
    advanced_actions.push((
        ToolbarEvent::ToggleZoomLock,
        if snapshot.zoom_locked {
            toolbar_icons::draw_icon_lock as IconFn
        } else {
            toolbar_icons::draw_icon_unlock as IconFn
        },
        lock_label,
        snapshot.zoom_active,
    ));

    if use_icons {
        let icon_btn_size = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON;
        let icon_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
        let icon_size = ToolbarLayoutSpec::SIDE_ACTION_ICON_SIZE;
        let mut actions: Vec<(ToolbarEvent, IconFn, &str, bool)> = Vec::new();
        if snapshot.show_actions_section {
            for (evt, icon_fn, label, enabled) in basic_actions {
                actions.push((evt.clone(), *icon_fn, *label, *enabled));
            }
        }
        if snapshot.show_actions_advanced {
            for (evt, icon_fn, label, enabled) in &advanced_actions {
                actions.push((evt.clone(), *icon_fn, *label, *enabled));
            }
        }
        let icons_per_row = 6usize;
        let total_icons = actions.len();
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
                let (evt, icon_fn, label, enabled) = &actions[idx];
                let bx = row_x + (icon_btn_size + icon_gap) * col as f64;
                let by = actions_y + (icon_btn_size + icon_gap) * row as f64;
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
    } else {
        if snapshot.show_actions_section {
            let action_h = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT;
            let action_gap = ToolbarLayoutSpec::SIDE_ACTION_CONTENT_GAP_TEXT;
            for (idx, (evt, _icon, label, enabled)) in basic_actions.iter().enumerate() {
                let by = actions_y + (action_h + action_gap) * idx as f64;
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
            actions_y += action_h * basic_actions.len() as f64
                + action_gap * (basic_actions.len() as f64 - 1.0);
        }

        if snapshot.show_actions_section && snapshot.show_actions_advanced {
            actions_y += ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
        }

        if snapshot.show_actions_advanced {
            let action_h = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT;
            let action_gap = ToolbarLayoutSpec::SIDE_ACTION_CONTENT_GAP_TEXT;
            let action_col_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
            let action_w = (content_width - action_col_gap) / 2.0;
            for (idx, (evt, _icon, label, enabled)) in advanced_actions.iter().enumerate() {
                let row = idx / 2;
                let col = idx % 2;
                let bx = x + (action_w + action_col_gap) * col as f64;
                let by = actions_y + (action_h + action_gap) * row as f64;
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
