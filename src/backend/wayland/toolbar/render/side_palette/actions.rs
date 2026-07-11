mod helpers;

use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::backend::wayland::toolbar::rows::row_item_width;
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarSnapshot;
use crate::ui::toolbar::model::{
    ToolbarActionsModel, ToolbarButtonModel, ToolbarCommandGroup, ToolbarCommandGroupKind,
};
use crate::ui::toolbar::{ToolbarEvent, ToolbarSideSection};
use crate::ui_text::UiTextStyle;

use super::super::widgets::constants::{FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL};
use super::super::widgets::draw_group_card;
use super::section_header::draw_collapsible_header;
use helpers::{
    ActionButton, ActionIconFn, IconActionLayout, TextActionLayout, render_icon_action_group,
    render_icon_action_row_split, render_text_action_group,
};

pub(super) fn draw_actions_section(layout: &mut SidePaletteLayout, y: &mut f64) {
    let ctx = layout.ctx;
    let snapshot = layout.snapshot;
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

    let Some(model) = ToolbarActionsModel::from_snapshot(snapshot) else {
        return;
    };

    let actions_card_h = layout.spec.side_actions_height(snapshot);
    draw_group_card(ctx, card_x, *y, card_w, actions_card_h);
    draw_collapsible_header(
        layout,
        *y,
        label_style,
        ToolbarSideSection::Actions,
        ToolbarSideSection::Actions.label(),
        ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
    );
    if snapshot.side_section_collapsed(ToolbarSideSection::Actions) {
        *y += actions_card_h + section_gap;
        return;
    }

    let hits = &mut layout.hits;
    let actions_y = *y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;

    if use_icons {
        render_icon_action_sections(
            ctx,
            hits,
            hover,
            snapshot,
            actions_y,
            x,
            content_width,
            &model,
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
            &model,
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
    model: &ToolbarActionsModel,
) {
    let icon_btn_size = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON;
    let icon_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
    let icon_size = ToolbarLayoutSpec::SIDE_ACTION_ICON_SIZE;
    let mut action_y = start_y;
    let mut has_group = false;
    for group in model.groups() {
        let actions = build_action_buttons(snapshot, group);
        if actions.is_empty() {
            continue;
        }
        if has_group {
            action_y += icon_gap;
        }
        action_y += draw_group_sub_label(ctx, x, action_y, group.kind);
        let layout = IconActionLayout {
            x,
            content_width,
            start_y: action_y,
            button_size: icon_btn_size,
            icon_size,
            gap: icon_gap,
            columns: icon_group_columns(group),
            add_gap: false,
        };
        let (next_y, has_rows) = if group.kind == ToolbarCommandGroupKind::History {
            // Destructive history actions (Clear) sit right-aligned, away
            // from Undo/Redo.
            let (leading, trailing): (Vec<_>, Vec<_>) = actions
                .iter()
                .cloned()
                .partition(|action| !action.event.is_destructive());
            render_icon_action_row_split(ctx, hits, hover, snapshot, layout, &leading, &trailing)
        } else {
            render_icon_action_group(ctx, hits, hover, snapshot, layout, &actions)
        };
        if has_rows {
            action_y = next_y;
            has_group = true;
        }
    }
}

/// Draw the small muted group label ("History", "Zoom") above an action
/// group; returns the height consumed.
fn draw_group_sub_label(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    kind: ToolbarCommandGroupKind,
) -> f64 {
    let Some(label) = kind.sub_label() else {
        return 0.0;
    };
    let style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: FONT_SIZE_LABEL * 0.85,
    };
    let layout = crate::ui_text::text_layout(ctx, style, label, None);
    ctx.set_source_rgba(0.62, 0.65, 0.72, 0.9);
    layout.show_at_baseline(ctx, x, y + 11.0);
    ToolbarLayoutSpec::SIDE_ACTION_GROUP_LABEL_HEIGHT
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
    model: &ToolbarActionsModel,
) {
    let action_h = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT;
    let action_row_gap = ToolbarLayoutSpec::SIDE_ACTION_CONTENT_GAP_TEXT;
    let action_group_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
    let action_col_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
    let mut action_y = start_y;
    let mut has_group = false;
    for group in model.groups() {
        let actions = build_action_buttons(snapshot, group);
        if actions.is_empty() {
            continue;
        }
        if has_group {
            action_y += action_group_gap;
        }
        action_y += draw_group_sub_label(ctx, x, action_y, group.kind);
        let (action_w, columns, column_gap, enabled_style) =
            text_group_layout(content_width, action_col_gap, group.kind);
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
                column_gap,
                group_gap: action_group_gap,
                row_gap: action_row_gap,
                columns,
                add_gap: false,
                label_style,
                enabled_style,
            },
            &actions,
        );
        if has_rows {
            action_y = next_y;
            has_group = true;
        }
    }
}

fn build_action_buttons(
    snapshot: &ToolbarSnapshot,
    group: &ToolbarCommandGroup,
) -> Vec<ActionButton> {
    group
        .buttons
        .iter()
        .map(|button| ActionButton {
            event: button.event.clone(),
            icon_fn: icon_for_button(snapshot, button),
            enabled: button.enabled,
        })
        .collect()
}

fn icon_group_columns(group: &ToolbarCommandGroup) -> usize {
    match group.kind {
        ToolbarCommandGroupKind::History => group.buttons.len(),
        ToolbarCommandGroupKind::Zoom | ToolbarCommandGroupKind::AdvancedActions => 5,
        ToolbarCommandGroupKind::Pages | ToolbarCommandGroupKind::Boards => group.buttons.len(),
    }
}

fn text_group_layout(
    content_width: f64,
    action_col_gap: f64,
    kind: ToolbarCommandGroupKind,
) -> (f64, usize, f64, bool) {
    match kind {
        ToolbarCommandGroupKind::History => (content_width, 1, 0.0, false),
        ToolbarCommandGroupKind::Zoom => (
            row_item_width(content_width, 2, action_col_gap),
            2,
            action_col_gap,
            true,
        ),
        ToolbarCommandGroupKind::AdvancedActions => (
            row_item_width(content_width, 2, action_col_gap),
            2,
            action_col_gap,
            false,
        ),
        ToolbarCommandGroupKind::Pages | ToolbarCommandGroupKind::Boards => {
            (content_width, 1, 0.0, false)
        }
    }
}

fn icon_for_button(snapshot: &ToolbarSnapshot, button: &ToolbarButtonModel) -> ActionIconFn {
    match &button.event {
        ToolbarEvent::Undo => toolbar_icons::draw_icon_undo as ActionIconFn,
        ToolbarEvent::Redo => toolbar_icons::draw_icon_redo as ActionIconFn,
        ToolbarEvent::ClearCanvas => toolbar_icons::draw_icon_clear as ActionIconFn,
        ToolbarEvent::ZoomIn => toolbar_icons::draw_icon_zoom_in as ActionIconFn,
        ToolbarEvent::ZoomOut => toolbar_icons::draw_icon_zoom_out as ActionIconFn,
        ToolbarEvent::ResetZoom => toolbar_icons::draw_icon_zoom_reset as ActionIconFn,
        ToolbarEvent::ToggleZoomLock => {
            if snapshot.zoom_locked {
                toolbar_icons::draw_icon_lock as ActionIconFn
            } else {
                toolbar_icons::draw_icon_unlock as ActionIconFn
            }
        }
        ToolbarEvent::UndoAll => toolbar_icons::draw_icon_undo_all as ActionIconFn,
        ToolbarEvent::RedoAll => toolbar_icons::draw_icon_redo_all as ActionIconFn,
        ToolbarEvent::UndoAllDelayed => toolbar_icons::draw_icon_undo_all_delay as ActionIconFn,
        ToolbarEvent::RedoAllDelayed => toolbar_icons::draw_icon_redo_all_delay as ActionIconFn,
        ToolbarEvent::ToggleFreeze => {
            if snapshot.frozen_active {
                toolbar_icons::draw_icon_unfreeze as ActionIconFn
            } else {
                toolbar_icons::draw_icon_freeze as ActionIconFn
            }
        }
        _ => toolbar_icons::draw_icon_clear as ActionIconFn,
    }
}
