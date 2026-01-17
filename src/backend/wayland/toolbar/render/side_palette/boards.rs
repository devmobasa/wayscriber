use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::backend::wayland::toolbar::rows::{grid_layout, row_item_width};
use crate::config::{action_label, action_short_label};
use crate::input::ToolbarDrawerTab;
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;
use crate::ui::toolbar::bindings::action_for_event;
use crate::ui_text::UiTextStyle;

use super::super::widgets::constants::{FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL};
use super::super::widgets::*;

pub(super) fn draw_boards_section(layout: &mut SidePaletteLayout, y: &mut f64) {
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

    if !snapshot.show_boards_section
        || !snapshot.drawer_open
        || snapshot.drawer_tab != ToolbarDrawerTab::View
    {
        return;
    }

    let boards_card_h = layout.spec.side_boards_height(snapshot);
    draw_group_card(ctx, card_x, *y, card_w, boards_card_h);
    draw_section_label(
        ctx,
        label_style,
        x,
        *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
        "Boards",
    );

    let boards_y = *y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let btn_h = if use_icons {
        ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON
    } else {
        ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT
    };
    let btn_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
    let can_cycle = snapshot.board_count > 1;
    let buttons = [
        (
            ToolbarEvent::BoardPrev,
            toolbar_icons::draw_icon_undo as fn(&cairo::Context, f64, f64, f64),
            can_cycle,
        ),
        (
            ToolbarEvent::BoardNext,
            toolbar_icons::draw_icon_redo as fn(&cairo::Context, f64, f64, f64),
            can_cycle,
        ),
        (
            ToolbarEvent::BoardNew,
            toolbar_icons::draw_icon_plus as fn(&cairo::Context, f64, f64, f64),
            true,
        ),
        (
            ToolbarEvent::BoardDelete,
            toolbar_icons::draw_icon_clear as fn(&cairo::Context, f64, f64, f64),
            true,
        ),
    ];

    let btn_w = row_item_width(content_width, buttons.len(), btn_gap);
    let layout = grid_layout(
        x,
        boards_y,
        btn_w,
        btn_h,
        btn_gap,
        0.0,
        buttons.len(),
        buttons.len(),
    );
    for (item, (evt, icon_fn, enabled)) in layout.items.iter().zip(buttons.iter()) {
        let label = button_label(evt);
        let bx = item.x;
        let by = item.y;
        let is_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, bx, by, btn_w, btn_h))
            .unwrap_or(false);
        draw_button(ctx, bx, by, btn_w, btn_h, *enabled, is_hover);
        if use_icons {
            if *enabled {
                set_icon_color(ctx, is_hover);
            } else {
                ctx.set_source_rgba(0.5, 0.5, 0.55, 0.5);
            }
            let icon_size = ToolbarLayoutSpec::SIDE_ACTION_ICON_SIZE;
            let icon_x = bx + (btn_w - icon_size) / 2.0;
            let icon_y = by + (btn_h - icon_size) / 2.0;
            icon_fn(ctx, icon_x, icon_y, icon_size);
        } else {
            draw_label_center(ctx, label_style, bx, by, btn_w, btn_h, label);
        }
        if *enabled {
            hits.push(HitRegion {
                rect: (bx, by, btn_w, btn_h),
                event: evt.clone(),
                kind: HitKind::Click,
                tooltip: Some(format_binding_label(
                    tooltip_label(evt),
                    snapshot.binding_hints.binding_for_event(evt),
                )),
            });
        }
    }

    *y += boards_card_h + section_gap;
}

fn button_label(event: &ToolbarEvent) -> &'static str {
    action_for_event(event)
        .map(action_short_label)
        .unwrap_or("Board")
}

fn tooltip_label(event: &ToolbarEvent) -> &'static str {
    action_for_event(event).map(action_label).unwrap_or("Board")
}
