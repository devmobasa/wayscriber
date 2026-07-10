use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::backend::wayland::toolbar::rows::{grid_layout, row_item_width};
use crate::toolbar_icons;
use crate::ui::toolbar::model::{ToolbarActivation, ToolbarIcon, ToolbarSettingsModel};
use crate::ui::toolbar::{ToolbarEvent, ToolbarSideSection};
use crate::ui_text::UiTextStyle;

use super::super::widgets::constants::{FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL};
use super::super::widgets::*;
use super::section_header::draw_collapsible_header;

pub(super) fn draw_settings_section(layout: &mut SidePaletteLayout, y: &mut f64) {
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
    let toggle_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: 12.0,
    };

    let Some(settings_model) = ToolbarSettingsModel::from_snapshot(snapshot) else {
        return;
    };

    let settings_card_h = layout.spec.side_settings_height(snapshot);
    draw_group_card(ctx, card_x, *y, card_w, settings_card_h);
    let customizing = snapshot.customize_items_open;
    let dedicated_panel = customizing;
    let header_label = if customizing {
        "Customize toolbar"
    } else {
        ToolbarSideSection::Settings.label()
    };
    draw_collapsible_header(
        layout,
        *y,
        label_style,
        ToolbarSideSection::Settings,
        header_label,
        ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
    );
    if !dedicated_panel && snapshot.side_section_collapsed(ToolbarSideSection::Settings) {
        *y += settings_card_h + section_gap;
        return;
    }

    let hits = &mut layout.hits;
    let toggle_h = ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT;
    let toggle_gap = ToolbarLayoutSpec::SIDE_TOGGLE_GAP;

    let mut toggle_y = *y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;

    // Layout-mode presets: Simple / Regular / Advanced. Non-destructive —
    // switching re-baselines the sections without touching explicit
    // user overrides.
    if !dedicated_panel {
        let seg_h = ToolbarLayoutSpec::SIDE_SEGMENT_HEIGHT;
        let seg_w = content_width;
        let mode_control = crate::ui::toolbar::model::layout_mode_control(snapshot.layout_mode);
        draw_layout_mode_segments(
            ctx,
            hits,
            hover,
            &mode_control,
            x,
            toggle_y,
            seg_w,
            seg_h,
            toggle_style,
        );
        toggle_y += seg_h + toggle_gap;
    }

    let toggle_col_gap = toggle_gap;
    let toggle_col_w = row_item_width(content_width, 2, toggle_col_gap);
    let toggle_rows = settings_model.toggle_rows();
    let toggle_row_count = toggle_rows.len();
    for (row_index, row) in toggle_rows.into_iter().enumerate() {
        let row_y = toggle_y + row_index as f64 * (toggle_h + toggle_gap);
        // A lone wide toggle spans the full content width; narrow toggles
        // sit in half-width cells.
        let full_row = row.len() == 1 && row[0].wide;
        for (col, toggle) in row.into_iter().enumerate() {
            let (cell_x, cell_w) = if full_row {
                (x, content_width)
            } else {
                (
                    x + col as f64 * (toggle_col_w + toggle_col_gap),
                    toggle_col_w,
                )
            };
            let toggle_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, cell_x, row_y, cell_w, toggle_h))
                .unwrap_or(false);
            draw_checkbox(
                ctx,
                cell_x,
                row_y,
                cell_w,
                toggle_h,
                toggle.checked,
                toggle_hover,
                toggle_style,
                toggle.label.as_ref(),
            );
            hits.push(HitRegion {
                focus_id: None,
                rect: (cell_x, row_y, cell_w, toggle_h),
                event: activation_event(&toggle.activation),
                kind: HitKind::Click,
                tooltip: toggle.tooltip.as_string(),
            });
        }
    }

    let mut buttons_y = toggle_y;
    if toggle_row_count > 0 {
        buttons_y += toggle_row_count as f64 * (toggle_h + toggle_gap) - toggle_gap;
    }
    buttons_y += toggle_gap;
    let button_h = ToolbarLayoutSpec::SIDE_SETTINGS_BUTTON_HEIGHT;
    let button_gap = ToolbarLayoutSpec::SIDE_SETTINGS_BUTTON_GAP;
    let button_w = row_item_width(content_width, 2, button_gap);
    let icon_size = 16.0;

    let buttons = settings_model.buttons();
    let button_layout = grid_layout(
        x,
        buttons_y,
        button_w,
        button_h,
        button_gap,
        button_gap,
        2,
        buttons.len(),
    );
    let button_label_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 11.0,
    };
    for (item, button) in button_layout.items.iter().zip(buttons.iter()) {
        let button_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, item.x, item.y, item.w, item.h))
            .unwrap_or(false);
        draw_button(ctx, item.x, item.y, item.w, item.h, false, button_hover);
        if use_icons {
            // Icon plus a left-aligned label: the icon-only glyphs were
            // ambiguous and left the row looking sparse and crammed at once.
            set_icon_color(ctx, button_hover);
            let icon_x = item.x + 6.0;
            draw_settings_icon(
                ctx,
                button.icon,
                icon_x,
                item.y + (item.h - icon_size) / 2.0,
                icon_size,
            );
            let text_x = icon_x + icon_size + 5.0;
            let text_w = item.x + item.w - text_x - 4.0;
            let display =
                ellipsize_to_width(ctx, button_label_style, button.label.as_ref(), text_w);
            draw_label_left(
                ctx,
                button_label_style,
                text_x,
                item.y,
                text_w,
                item.h,
                &display,
            );
        } else {
            draw_label_center(
                ctx,
                label_style,
                item.x,
                item.y,
                item.w,
                item.h,
                button.label.as_ref(),
            );
        }
        hits.push(HitRegion {
            focus_id: None,
            rect: (item.x, item.y, item.w, item.h),
            event: button.event.clone(),
            kind: HitKind::Click,
            tooltip: button.tooltip.as_string(),
        });
    }

    let mut customize_y = buttons_y;
    if button_layout.rows > 0 {
        customize_y += button_layout.height;
    }
    customize_y += toggle_gap;
    let groups = settings_model.groups();
    if !groups.is_empty() {
        draw_label_left(
            ctx,
            label_style,
            x,
            customize_y,
            content_width,
            toggle_h,
            "Choose a group",
        );
        customize_y += toggle_h + toggle_gap;
    }
    let group_layout = grid_layout(
        x,
        customize_y,
        button_w,
        button_h,
        button_gap,
        button_gap,
        2,
        groups.len(),
    );
    for (item, group) in group_layout.items.iter().zip(groups.iter()) {
        let group_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, item.x, item.y, item.w, item.h))
            .unwrap_or(false);
        draw_button(ctx, item.x, item.y, item.w, item.h, false, group_hover);
        draw_label_center(
            ctx,
            label_style,
            item.x,
            item.y,
            item.w,
            item.h,
            group.label.as_ref(),
        );
        hits.push(HitRegion {
            focus_id: None,
            rect: (item.x, item.y, item.w, item.h),
            event: group.event.clone(),
            kind: HitKind::Click,
            tooltip: group.tooltip.as_string(),
        });
    }

    let mut items_y = customize_y;
    if group_layout.rows > 0 {
        items_y += group_layout.height + toggle_gap;
    }
    let item_overrides = settings_model.item_overrides();
    if !item_overrides.is_empty() {
        draw_label_left(
            ctx,
            label_style,
            x,
            items_y,
            content_width,
            toggle_h,
            snapshot
                .customize_items_group
                .map_or("Uncheck items to hide", |group| group.label()),
        );
        items_y += toggle_h + toggle_gap;
    }
    let item_layout = grid_layout(
        x,
        items_y,
        content_width,
        toggle_h,
        toggle_col_gap,
        toggle_gap,
        1,
        item_overrides.len(),
    );
    for (item, override_item) in item_layout.items.iter().zip(item_overrides.iter()) {
        let item_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, item.x, item.y, item.w, item.h))
            .unwrap_or(false);
        let order = override_item.order.as_ref();
        let order_gap = 4.0;
        let handle_w = if order.is_some() { 24.0 } else { 0.0 };
        let move_btn_w = if order.is_some() { 28.0 } else { 0.0 };
        let move_buttons_w = if order.is_some() {
            move_btn_w * 2.0 + order_gap * 2.0
        } else {
            0.0
        };
        if order.is_some() {
            let handle_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, item.x, item.y, handle_w, item.h))
                .unwrap_or(false);
            draw_button(ctx, item.x, item.y, handle_w, item.h, false, handle_hover);
            draw_label_center(ctx, toggle_style, item.x, item.y, handle_w, item.h, "=");
        }
        let checkbox_x = item.x + handle_w + if order.is_some() { order_gap } else { 0.0 };
        let checkbox_w =
            item.w - handle_w - move_buttons_w - if order.is_some() { order_gap } else { 0.0 };
        draw_checkbox(
            ctx,
            checkbox_x,
            item.y,
            checkbox_w,
            item.h,
            override_item.shown,
            item_hover,
            toggle_style,
            override_item.label.as_ref(),
        );
        hits.push(HitRegion {
            focus_id: None,
            rect: (checkbox_x, item.y, checkbox_w, item.h),
            event: activation_event(&override_item.activation),
            kind: HitKind::Click,
            tooltip: override_item.tooltip.as_string(),
        });
        if let Some(order) = order {
            let up_x = item.x + item.w - move_btn_w * 2.0 - order_gap;
            let down_x = item.x + item.w - move_btn_w;
            for (button_x, label, enabled, activation, tooltip) in [
                (up_x, "^", order.can_move_up, &order.move_up, "Move up"),
                (
                    down_x,
                    "v",
                    order.can_move_down,
                    &order.move_down,
                    "Move down",
                ),
            ] {
                let button_hover = enabled
                    && hover
                        .map(|(hx, hy)| point_in_rect(hx, hy, button_x, item.y, move_btn_w, item.h))
                        .unwrap_or(false);
                draw_button(
                    ctx,
                    button_x,
                    item.y,
                    move_btn_w,
                    item.h,
                    false,
                    button_hover,
                );
                draw_label_center(
                    ctx,
                    toggle_style,
                    button_x,
                    item.y,
                    move_btn_w,
                    item.h,
                    label,
                );
                if enabled {
                    hits.push(HitRegion {
                        focus_id: None,
                        rect: (button_x, item.y, move_btn_w, item.h),
                        event: activation_event(activation),
                        kind: HitKind::Click,
                        tooltip: Some(format!("{} {}", tooltip, override_item.label)),
                    });
                }
            }
            hits.push(HitRegion {
                focus_id: None,
                rect: (item.x, item.y, item.w, item.h),
                event: ToolbarEvent::StartToolbarItemDrag {
                    group: order.group,
                    id: override_item.id,
                },
                kind: HitKind::DragToolbarItem {
                    group: order.group,
                    id: override_item.id,
                    target_index: order.index,
                },
                tooltip: Some(format!("Drag {} to reorder", override_item.label)),
            });
        }
    }

    *y += settings_card_h + section_gap;
}

fn activation_event(activation: &ToolbarActivation) -> crate::ui::toolbar::ToolbarEvent {
    activation.compatibility_event()
}

fn draw_settings_icon(ctx: &cairo::Context, icon: ToolbarIcon, x: f64, y: f64, size: f64) {
    match icon {
        ToolbarIcon::Back => draw_back_icon(ctx, x, y, size),
        ToolbarIcon::Settings => toolbar_icons::draw_icon_settings(ctx, x, y, size),
        ToolbarIcon::Visibility => toolbar_icons::draw_icon_visibility(ctx, x, y, size),
        ToolbarIcon::File => toolbar_icons::draw_icon_file(ctx, x, y, size),
        ToolbarIcon::More | ToolbarIcon::Board => {}
    }
}

fn draw_back_icon(ctx: &cairo::Context, x: f64, y: f64, size: f64) {
    let mid_y = y + size * 0.5;
    ctx.set_line_width(2.0);
    ctx.move_to(x + size * 0.65, y + size * 0.25);
    ctx.line_to(x + size * 0.35, mid_y);
    ctx.line_to(x + size * 0.65, y + size * 0.75);
    let _ = ctx.stroke();
}

#[allow(clippy::too_many_arguments)]
fn draw_layout_mode_segments(
    ctx: &cairo::Context,
    hits: &mut Vec<HitRegion>,
    hover: Option<(f64, f64)>,
    control: &crate::ui::toolbar::model::ToolbarControl,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    label_style: UiTextStyle<'_>,
) {
    let crate::ui::toolbar::model::ToolbarControlKind::Segmented(segmented) = &control.kind else {
        return;
    };
    let segments = segmented.segments();
    if segments.is_empty() {
        return;
    }
    let active = segmented
        .active_segment()
        .and_then(|active| segments.iter().position(|s| s.id == active))
        .unwrap_or(0);
    // Same treatment as the pane navigation row: equal-width segments with
    // a small gap, spanning the full content width.
    let seg_gap = 4.0;
    let seg_w = (w - seg_gap * (segments.len() as f64 - 1.0)) / segments.len() as f64;
    for (index, segment) in segments.iter().enumerate() {
        let seg_x = x + (seg_w + seg_gap) * index as f64;
        let is_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, seg_x, y, seg_w, h))
            .unwrap_or(false);
        draw_button(ctx, seg_x, y, seg_w, h, index == active, is_hover);
        draw_label_center(ctx, label_style, seg_x, y, seg_w, h, segment.label.as_ref());
        hits.push(HitRegion {
            focus_id: None,
            rect: (seg_x, y, seg_w, h),
            event: segment.activation.compatibility_event(),
            kind: HitKind::Click,
            tooltip: segment.tooltip.as_string(),
        });
    }
}
