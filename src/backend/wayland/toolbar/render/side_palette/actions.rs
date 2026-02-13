mod helpers;

use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::backend::wayland::toolbar::rows::row_item_width;
use crate::input::ToolbarDrawerTab;
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;
use crate::ui::toolbar::ToolbarSnapshot;
use crate::ui_text::UiTextStyle;

use super::super::widgets::constants::{FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL};
use super::super::widgets::draw_group_card;
use super::super::widgets::draw_section_label;
use helpers::{
    ActionButton, ActionIconFn, IconActionLayout, TextActionLayout, render_icon_action_group,
    render_text_action_group,
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
    let basic_actions = build_basic_action_buttons(snapshot);
    let view_actions = build_view_action_buttons(snapshot);
    let show_delay_actions = show_advanced && snapshot.delay_actions_enabled;
    let advanced_actions = build_advanced_action_buttons(snapshot, show_delay_actions);

    if use_icons {
        render_icon_action_sections(
            ctx,
            hits,
            hover,
            snapshot,
            actions_y,
            x,
            content_width,
            &basic_actions,
            &view_actions,
            &advanced_actions,
            show_view_actions,
            show_advanced,
        );
    } else {
        render_text_action_sections(
            ctx,
            hits,
            hover,
            snapshot,
            actions_y,
            x,
            content_width,
            label_style,
            &basic_actions,
            &view_actions,
            &advanced_actions,
            show_view_actions,
            show_advanced,
        );
    }

    *y += actions_card_h + section_gap;
}

#[allow(clippy::too_many_arguments)]
fn render_icon_action_sections(
    ctx: &cairo::Context,
    hits: &mut Vec<HitRegion>,
    hover: Option<(f64, f64)>,
    snapshot: &ToolbarSnapshot,
    start_y: f64,
    x: f64,
    content_width: f64,
    basic_actions: &[ActionButton],
    view_actions: &[ActionButton],
    advanced_actions: &[ActionButton],
    show_view_actions: bool,
    show_advanced: bool,
) {
    let icon_btn_size = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON;
    let icon_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
    let icon_size = ToolbarLayoutSpec::SIDE_ACTION_ICON_SIZE;
    let (action_y, has_group) = if snapshot.show_actions_section {
        render_icon_action_group(
            ctx,
            hits,
            hover,
            snapshot,
            IconActionLayout {
                x,
                content_width,
                start_y,
                button_size: icon_btn_size,
                icon_size,
                gap: icon_gap,
                columns: basic_actions.len(),
                add_gap: false,
            },
            basic_actions,
        )
    } else {
        (start_y, false)
    };

    let (action_y, has_group) = if show_view_actions {
        let (next_y, has_rows) = render_icon_action_group(
            ctx,
            hits,
            hover,
            snapshot,
            IconActionLayout {
                x,
                content_width,
                start_y: action_y,
                button_size: icon_btn_size,
                icon_size,
                gap: icon_gap,
                columns: 5,
                add_gap: has_group,
            },
            view_actions,
        );
        (next_y, has_group || has_rows)
    } else {
        (action_y, has_group)
    };

    if show_advanced {
        let _ = render_icon_action_group(
            ctx,
            hits,
            hover,
            snapshot,
            IconActionLayout {
                x,
                content_width,
                start_y: action_y,
                button_size: icon_btn_size,
                icon_size,
                gap: icon_gap,
                columns: 5,
                add_gap: has_group,
            },
            advanced_actions,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_text_action_sections(
    ctx: &cairo::Context,
    hits: &mut Vec<HitRegion>,
    hover: Option<(f64, f64)>,
    snapshot: &ToolbarSnapshot,
    start_y: f64,
    x: f64,
    content_width: f64,
    label_style: UiTextStyle<'static>,
    basic_actions: &[ActionButton],
    view_actions: &[ActionButton],
    advanced_actions: &[ActionButton],
    show_view_actions: bool,
    show_advanced: bool,
) {
    let action_h = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT;
    let action_row_gap = ToolbarLayoutSpec::SIDE_ACTION_CONTENT_GAP_TEXT;
    let action_group_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
    let action_col_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
    let action_w = row_item_width(content_width, 2, action_col_gap);
    let (action_y, has_group) = if snapshot.show_actions_section {
        render_text_action_group(
            ctx,
            hits,
            hover,
            snapshot,
            TextActionLayout {
                x,
                start_y,
                width: content_width,
                height: action_h,
                column_gap: 0.0,
                group_gap: action_group_gap,
                row_gap: action_row_gap,
                columns: 1,
                add_gap: false,
                label_style,
                enabled_style: false,
            },
            basic_actions,
        )
    } else {
        (start_y, false)
    };

    let (action_y, has_group) = if show_view_actions {
        let (next_y, has_rows) = render_text_action_group(
            ctx,
            hits,
            hover,
            snapshot,
            TextActionLayout {
                x,
                start_y: action_y,
                width: action_w,
                height: action_h,
                column_gap: action_col_gap,
                group_gap: action_group_gap,
                row_gap: action_row_gap,
                columns: 2,
                add_gap: has_group,
                label_style,
                enabled_style: true,
            },
            view_actions,
        );
        (next_y, has_group || has_rows)
    } else {
        (action_y, has_group)
    };

    if show_advanced {
        let _ = render_text_action_group(
            ctx,
            hits,
            hover,
            snapshot,
            TextActionLayout {
                x,
                start_y: action_y,
                width: action_w,
                height: action_h,
                column_gap: action_col_gap,
                group_gap: action_group_gap,
                row_gap: action_row_gap,
                columns: 2,
                add_gap: has_group,
                label_style,
                enabled_style: false,
            },
            advanced_actions,
        );
    }
}

fn build_basic_action_buttons(snapshot: &ToolbarSnapshot) -> [ActionButton; 3] {
    [
        ActionButton {
            event: ToolbarEvent::Undo,
            icon_fn: toolbar_icons::draw_icon_undo as ActionIconFn,
            enabled: snapshot.undo_available,
        },
        ActionButton {
            event: ToolbarEvent::Redo,
            icon_fn: toolbar_icons::draw_icon_redo as ActionIconFn,
            enabled: snapshot.redo_available,
        },
        ActionButton {
            event: ToolbarEvent::ClearCanvas,
            icon_fn: toolbar_icons::draw_icon_clear as ActionIconFn,
            enabled: true,
        },
    ]
}

fn build_view_action_buttons(snapshot: &ToolbarSnapshot) -> [ActionButton; 4] {
    [
        ActionButton {
            event: ToolbarEvent::ZoomIn,
            icon_fn: toolbar_icons::draw_icon_zoom_in as ActionIconFn,
            enabled: true,
        },
        ActionButton {
            event: ToolbarEvent::ZoomOut,
            icon_fn: toolbar_icons::draw_icon_zoom_out as ActionIconFn,
            enabled: true,
        },
        ActionButton {
            event: ToolbarEvent::ResetZoom,
            icon_fn: toolbar_icons::draw_icon_zoom_reset as ActionIconFn,
            enabled: snapshot.zoom_active,
        },
        ActionButton {
            event: ToolbarEvent::ToggleZoomLock,
            icon_fn: if snapshot.zoom_locked {
                toolbar_icons::draw_icon_lock as ActionIconFn
            } else {
                toolbar_icons::draw_icon_unlock as ActionIconFn
            },
            enabled: snapshot.zoom_active,
        },
    ]
}

fn build_advanced_action_buttons(
    snapshot: &ToolbarSnapshot,
    show_delay_actions: bool,
) -> Vec<ActionButton> {
    let mut actions = Vec::with_capacity(6);

    actions.push(ActionButton {
        event: ToolbarEvent::UndoAll,
        icon_fn: toolbar_icons::draw_icon_undo_all as ActionIconFn,
        enabled: snapshot.undo_available,
    });
    actions.push(ActionButton {
        event: ToolbarEvent::RedoAll,
        icon_fn: toolbar_icons::draw_icon_redo_all as ActionIconFn,
        enabled: snapshot.redo_available,
    });

    if show_delay_actions {
        actions.push(ActionButton {
            event: ToolbarEvent::UndoAllDelayed,
            icon_fn: toolbar_icons::draw_icon_undo_all_delay as ActionIconFn,
            enabled: snapshot.undo_available,
        });
        actions.push(ActionButton {
            event: ToolbarEvent::RedoAllDelayed,
            icon_fn: toolbar_icons::draw_icon_redo_all_delay as ActionIconFn,
            enabled: snapshot.redo_available,
        });
    }

    actions.push(ActionButton {
        event: ToolbarEvent::ToggleFreeze,
        icon_fn: if snapshot.frozen_active {
            toolbar_icons::draw_icon_unfreeze as ActionIconFn
        } else {
            toolbar_icons::draw_icon_freeze as ActionIconFn
        },
        enabled: true,
    });

    actions
}
