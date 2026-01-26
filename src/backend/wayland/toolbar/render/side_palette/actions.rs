use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::backend::wayland::toolbar::rows::{centered_grid_layout, grid_layout, row_item_width};
use crate::config::{Action, action_label, action_short_label};
use crate::input::ToolbarDrawerTab;
use crate::toolbar_icons;
use crate::ui::toolbar::bindings::action_for_event;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot};
use crate::ui_text::UiTextStyle;

use super::super::widgets::constants::{
    COLOR_TEXT_DISABLED, FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL, set_color,
};
use super::super::widgets::{
    draw_button, draw_destructive_button, draw_group_card, draw_label_center, draw_section_label,
    point_in_rect, set_icon_color,
};

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
    let label_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: FONT_SIZE_LABEL,
    };

    let show_drawer_view = snapshot.drawer_open && snapshot.drawer_tab == ToolbarDrawerTab::View;
    let show_advanced = snapshot.show_actions_advanced && show_drawer_view;
    let show_view_actions = show_drawer_view
        && snapshot.show_zoom_actions
        && (snapshot.show_actions_section || snapshot.show_actions_advanced);
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
        label_style,
        x,
        *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
        "Actions",
    );

    let actions_y = *y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    type IconFn = fn(&cairo::Context, f64, f64, f64);
    let basic_actions: &[(ToolbarEvent, IconFn, bool)] = &[
        (
            ToolbarEvent::Undo,
            toolbar_icons::draw_icon_undo as IconFn,
            snapshot.undo_available,
        ),
        (
            ToolbarEvent::Redo,
            toolbar_icons::draw_icon_redo as IconFn,
            snapshot.redo_available,
        ),
        (
            ToolbarEvent::ClearCanvas,
            toolbar_icons::draw_icon_clear as IconFn,
            true,
        ),
    ];
    let view_actions: Vec<(ToolbarEvent, IconFn, bool)> = vec![
        (
            ToolbarEvent::ZoomIn,
            toolbar_icons::draw_icon_zoom_in as IconFn,
            true,
        ),
        (
            ToolbarEvent::ZoomOut,
            toolbar_icons::draw_icon_zoom_out as IconFn,
            true,
        ),
        (
            ToolbarEvent::ResetZoom,
            toolbar_icons::draw_icon_zoom_reset as IconFn,
            snapshot.zoom_active,
        ),
        (
            ToolbarEvent::ToggleZoomLock,
            if snapshot.zoom_locked {
                toolbar_icons::draw_icon_lock as IconFn
            } else {
                toolbar_icons::draw_icon_unlock as IconFn
            },
            snapshot.zoom_active,
        ),
    ];
    let show_delay_actions = show_advanced && snapshot.delay_actions_enabled;
    let mut advanced_actions: Vec<(ToolbarEvent, IconFn, bool)> = Vec::new();
    advanced_actions.push((
        ToolbarEvent::UndoAll,
        toolbar_icons::draw_icon_undo_all as IconFn,
        snapshot.undo_available,
    ));
    advanced_actions.push((
        ToolbarEvent::RedoAll,
        toolbar_icons::draw_icon_redo_all as IconFn,
        snapshot.redo_available,
    ));
    if show_delay_actions {
        advanced_actions.push((
            ToolbarEvent::UndoAllDelayed,
            toolbar_icons::draw_icon_undo_all_delay as IconFn,
            snapshot.undo_available,
        ));
        advanced_actions.push((
            ToolbarEvent::RedoAllDelayed,
            toolbar_icons::draw_icon_redo_all_delay as IconFn,
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
        true,
    ));

    if use_icons {
        let icon_btn_size = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON;
        let icon_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
        let icon_size = ToolbarLayoutSpec::SIDE_ACTION_ICON_SIZE;
        let mut action_y = actions_y;
        let mut has_group = false;

        if snapshot.show_actions_section {
            let layout = centered_grid_layout(
                x,
                content_width,
                action_y,
                icon_btn_size,
                icon_gap,
                basic_actions.len(),
                basic_actions.len(),
            );
            for (item, (evt, icon_fn, enabled)) in layout.items.iter().zip(basic_actions.iter()) {
                let tooltip_label = tooltip_label(evt, snapshot);
                let bx = item.x;
                let by = item.y;
                let is_hover = hover
                    .map(|(hx, hy)| point_in_rect(hx, hy, bx, by, icon_btn_size, icon_btn_size))
                    .unwrap_or(false);
                let is_destructive = matches!(evt, ToolbarEvent::ClearCanvas);
                if *enabled {
                    if is_destructive {
                        draw_destructive_button(
                            ctx,
                            bx,
                            by,
                            icon_btn_size,
                            icon_btn_size,
                            is_hover,
                        );
                    } else {
                        draw_button(ctx, bx, by, icon_btn_size, icon_btn_size, false, is_hover);
                    }
                    set_icon_color(ctx, is_hover);
                } else {
                    draw_button(ctx, bx, by, icon_btn_size, icon_btn_size, false, false);
                    set_color(ctx, COLOR_TEXT_DISABLED);
                }
                let icon_x = bx + (icon_btn_size - icon_size) / 2.0;
                let icon_y = by + (icon_btn_size - icon_size) / 2.0;
                icon_fn(ctx, icon_x, icon_y, icon_size);
                hits.push(HitRegion {
                    rect: (bx, by, icon_btn_size, icon_btn_size),
                    event: evt.clone(),
                    kind: HitKind::Click,
                    tooltip: Some(format_binding_label(
                        tooltip_label,
                        snapshot.binding_hints.binding_for_event(evt),
                    )),
                });
            }
            action_y += layout.height;
            has_group = layout.rows > 0;
        }

        if show_view_actions {
            if has_group {
                action_y += icon_gap;
            }
            let layout = centered_grid_layout(
                x,
                content_width,
                action_y,
                icon_btn_size,
                icon_gap,
                5,
                view_actions.len(),
            );
            for (item, (evt, icon_fn, enabled)) in layout.items.iter().zip(view_actions.iter()) {
                let tooltip_label = tooltip_label(evt, snapshot);
                let bx = item.x;
                let by = item.y;
                let is_hover = hover
                    .map(|(hx, hy)| point_in_rect(hx, hy, bx, by, icon_btn_size, icon_btn_size))
                    .unwrap_or(false);
                if *enabled {
                    draw_button(ctx, bx, by, icon_btn_size, icon_btn_size, false, is_hover);
                    set_icon_color(ctx, is_hover);
                } else {
                    draw_button(ctx, bx, by, icon_btn_size, icon_btn_size, false, false);
                    set_color(ctx, COLOR_TEXT_DISABLED);
                }
                let icon_x = bx + (icon_btn_size - icon_size) / 2.0;
                let icon_y = by + (icon_btn_size - icon_size) / 2.0;
                icon_fn(ctx, icon_x, icon_y, icon_size);
                hits.push(HitRegion {
                    rect: (bx, by, icon_btn_size, icon_btn_size),
                    event: evt.clone(),
                    kind: HitKind::Click,
                    tooltip: Some(format_binding_label(
                        tooltip_label,
                        snapshot.binding_hints.binding_for_event(evt),
                    )),
                });
            }
            if layout.rows > 0 {
                action_y += layout.height;
                has_group = true;
            }
        }

        if show_advanced {
            if has_group {
                action_y += icon_gap;
            }
            let layout = centered_grid_layout(
                x,
                content_width,
                action_y,
                icon_btn_size,
                icon_gap,
                5,
                advanced_actions.len(),
            );
            for (item, (evt, icon_fn, enabled)) in layout.items.iter().zip(advanced_actions.iter())
            {
                let tooltip_label = tooltip_label(evt, snapshot);
                let bx = item.x;
                let by = item.y;
                let is_hover = hover
                    .map(|(hx, hy)| point_in_rect(hx, hy, bx, by, icon_btn_size, icon_btn_size))
                    .unwrap_or(false);
                let is_destructive =
                    matches!(evt, ToolbarEvent::UndoAll | ToolbarEvent::UndoAllDelayed);
                if *enabled {
                    if is_destructive {
                        draw_destructive_button(
                            ctx,
                            bx,
                            by,
                            icon_btn_size,
                            icon_btn_size,
                            is_hover,
                        );
                    } else {
                        draw_button(ctx, bx, by, icon_btn_size, icon_btn_size, false, is_hover);
                    }
                    set_icon_color(ctx, is_hover);
                } else {
                    draw_button(ctx, bx, by, icon_btn_size, icon_btn_size, false, false);
                    set_color(ctx, COLOR_TEXT_DISABLED);
                }
                let icon_x = bx + (icon_btn_size - icon_size) / 2.0;
                let icon_y = by + (icon_btn_size - icon_size) / 2.0;
                icon_fn(ctx, icon_x, icon_y, icon_size);
                hits.push(HitRegion {
                    rect: (bx, by, icon_btn_size, icon_btn_size),
                    event: evt.clone(),
                    kind: HitKind::Click,
                    tooltip: Some(format_binding_label(
                        tooltip_label,
                        snapshot.binding_hints.binding_for_event(evt),
                    )),
                });
            }
        }
    } else {
        let action_h = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT;
        let action_gap = ToolbarLayoutSpec::SIDE_ACTION_CONTENT_GAP_TEXT;
        let action_col_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
        let action_w = row_item_width(content_width, 2, action_col_gap);
        let mut action_y = actions_y;
        let mut has_group = false;

        if snapshot.show_actions_section {
            let layout = grid_layout(
                x,
                action_y,
                content_width,
                action_h,
                0.0,
                action_gap,
                1,
                basic_actions.len(),
            );
            for (item, (evt, _icon, enabled)) in layout.items.iter().zip(basic_actions.iter()) {
                let label = button_label(evt, snapshot);
                let tooltip_label = tooltip_label(evt, snapshot);
                let is_hover = hover
                    .map(|(hx, hy)| point_in_rect(hx, hy, item.x, item.y, item.w, item.h))
                    .unwrap_or(false);
                let is_destructive = matches!(evt, ToolbarEvent::ClearCanvas);
                if is_destructive && *enabled {
                    draw_destructive_button(ctx, item.x, item.y, item.w, item.h, is_hover);
                } else {
                    draw_button(ctx, item.x, item.y, item.w, item.h, false, is_hover);
                }
                draw_label_center(ctx, label_style, item.x, item.y, item.w, item.h, label);
                if *enabled {
                    hits.push(HitRegion {
                        rect: (item.x, item.y, item.w, item.h),
                        event: evt.clone(),
                        kind: HitKind::Click,
                        tooltip: Some(format_binding_label(
                            tooltip_label,
                            snapshot.binding_hints.binding_for_event(evt),
                        )),
                    });
                }
            }
            action_y += layout.height;
            has_group = layout.rows > 0;
        }

        if show_view_actions {
            if has_group {
                action_y += ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
            }
            let layout = grid_layout(
                x,
                action_y,
                action_w,
                action_h,
                action_col_gap,
                action_gap,
                2,
                view_actions.len(),
            );
            for (item, (evt, _icon, enabled)) in layout.items.iter().zip(view_actions.iter()) {
                let label = button_label(evt, snapshot);
                let tooltip_label = tooltip_label(evt, snapshot);
                let is_hover = hover
                    .map(|(hx, hy)| point_in_rect(hx, hy, item.x, item.y, item.w, item.h))
                    .unwrap_or(false);
                draw_button(ctx, item.x, item.y, item.w, item.h, *enabled, is_hover);
                draw_label_center(ctx, label_style, item.x, item.y, item.w, item.h, label);
                if *enabled {
                    hits.push(HitRegion {
                        rect: (item.x, item.y, item.w, item.h),
                        event: evt.clone(),
                        kind: HitKind::Click,
                        tooltip: Some(format_binding_label(
                            tooltip_label,
                            snapshot.binding_hints.binding_for_event(evt),
                        )),
                    });
                }
            }
            if layout.rows > 0 {
                action_y += layout.height;
                has_group = true;
            }
        }

        if show_advanced {
            if has_group {
                action_y += ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
            }
            let layout = grid_layout(
                x,
                action_y,
                action_w,
                action_h,
                action_col_gap,
                action_gap,
                2,
                advanced_actions.len(),
            );
            for (item, (evt, _icon, enabled)) in layout.items.iter().zip(advanced_actions.iter()) {
                let label = button_label(evt, snapshot);
                let tooltip_label = tooltip_label(evt, snapshot);
                let is_hover = hover
                    .map(|(hx, hy)| point_in_rect(hx, hy, item.x, item.y, item.w, item.h))
                    .unwrap_or(false);
                let is_destructive =
                    matches!(evt, ToolbarEvent::UndoAll | ToolbarEvent::UndoAllDelayed);
                if is_destructive && *enabled {
                    draw_destructive_button(ctx, item.x, item.y, item.w, item.h, is_hover);
                } else {
                    draw_button(ctx, item.x, item.y, item.w, item.h, false, is_hover);
                }
                draw_label_center(ctx, label_style, item.x, item.y, item.w, item.h, label);
                if *enabled {
                    hits.push(HitRegion {
                        rect: (item.x, item.y, item.w, item.h),
                        event: evt.clone(),
                        kind: HitKind::Click,
                        tooltip: Some(format_binding_label(
                            tooltip_label,
                            snapshot.binding_hints.binding_for_event(evt),
                        )),
                    });
                }
            }
        }
    }

    *y += actions_card_h + section_gap;
}

fn button_label(event: &ToolbarEvent, snapshot: &ToolbarSnapshot) -> &'static str {
    match event {
        ToolbarEvent::ToggleFreeze => {
            if snapshot.frozen_active {
                "Unfreeze"
            } else {
                action_short_label(Action::ToggleFrozenMode)
            }
        }
        ToolbarEvent::ToggleZoomLock => {
            if snapshot.zoom_locked {
                "Unlock Zoom"
            } else {
                action_short_label(Action::ToggleZoomLock)
            }
        }
        _ => action_for_event(event)
            .map(action_short_label)
            .unwrap_or("Action"),
    }
}

fn tooltip_label(event: &ToolbarEvent, snapshot: &ToolbarSnapshot) -> &'static str {
    match event {
        ToolbarEvent::ToggleFreeze => {
            if snapshot.frozen_active {
                "Unfreeze"
            } else {
                action_label(Action::ToggleFrozenMode)
            }
        }
        ToolbarEvent::ToggleZoomLock => {
            if snapshot.zoom_locked {
                "Unlock Zoom"
            } else {
                action_label(Action::ToggleZoomLock)
            }
        }
        _ => action_for_event(event)
            .map(action_label)
            .unwrap_or("Action"),
    }
}
