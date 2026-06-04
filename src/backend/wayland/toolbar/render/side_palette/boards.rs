use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::backend::wayland::toolbar::rows::{capped_grid_columns, grid_layout, row_item_width};
use crate::toolbar_icons;
use crate::ui::toolbar::model::toolbar_boards_model;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSideSection};
use crate::ui_text::UiTextStyle;

use super::super::widgets::constants::{FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL};
use super::super::widgets::*;
use super::section_header::draw_collapsible_header;

pub(super) fn draw_boards_section(layout: &mut SidePaletteLayout, y: &mut f64) {
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

    let Some(model) = toolbar_boards_model(snapshot) else {
        return;
    };

    let boards_card_h = layout.spec.side_boards_height(snapshot);
    draw_group_card(ctx, card_x, *y, card_w, boards_card_h);

    // Section label with keybinding hint
    let label_y = *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL;
    draw_collapsible_header(
        layout,
        *y,
        label_style,
        ToolbarSideSection::Boards,
        ToolbarSideSection::Boards.label(),
        ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
    );

    // Draw keybinding hint for board picker (right-aligned)
    if let Some(binding) = snapshot
        .binding_hints
        .binding_for_event(&ToolbarEvent::ToggleBoardPicker)
    {
        let hint_style = UiTextStyle {
            family: FONT_FAMILY_DEFAULT,
            slant: cairo::FontSlant::Normal,
            weight: cairo::FontWeight::Normal,
            size: FONT_SIZE_LABEL * 0.85,
        };
        let hint_layout = crate::ui_text::text_layout(ctx, hint_style, binding, None);
        let hint_width = hint_layout.ink_extents().width();
        let chevron_reserve = ToolbarLayoutSpec::SIDE_COLLAPSE_CHEVRON_SIZE + 10.0;
        let hint_x = x + content_width - hint_width - chevron_reserve;
        ctx.set_source_rgba(0.55, 0.58, 0.65, 0.9);
        hint_layout.show_at_baseline(ctx, hint_x, label_y);
    }
    if snapshot.side_section_collapsed(ToolbarSideSection::Boards) {
        *y += boards_card_h + section_gap;
        return;
    }

    let hits = &mut layout.hits;
    let boards_y = *y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let btn_h = if use_icons {
        ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON
    } else {
        ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT
    };
    let btn_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
    let cols = capped_grid_columns(model.buttons.len(), 5);
    let btn_w = row_item_width(content_width, cols, btn_gap);
    let layout = grid_layout(
        x,
        boards_y,
        btn_w,
        btn_h,
        btn_gap,
        btn_gap,
        model.buttons.len(),
        cols,
    );
    for (item, button) in layout.items.iter().zip(model.buttons.iter()) {
        let label = button.short_label(snapshot, "Board");
        let bx = item.x;
        let by = item.y;
        let is_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, bx, by, btn_w, btn_h))
            .unwrap_or(false);
        draw_button(ctx, bx, by, btn_w, btn_h, button.enabled, is_hover);
        if use_icons {
            if button.enabled {
                set_icon_color(ctx, is_hover);
            } else {
                ctx.set_source_rgba(0.5, 0.5, 0.55, 0.5);
            }
            let icon_size = ToolbarLayoutSpec::SIDE_ACTION_ICON_SIZE;
            let icon_x = bx + (btn_w - icon_size) / 2.0;
            let icon_y = by + (btn_h - icon_size) / 2.0;
            board_icon(&button.event)(ctx, icon_x, icon_y, icon_size);
        } else {
            draw_label_center(ctx, label_style, bx, by, btn_w, btn_h, label);
        }
        if button.enabled {
            hits.push(HitRegion {
                rect: (bx, by, btn_w, btn_h),
                event: button.event.clone(),
                kind: HitKind::Click,
                tooltip: Some(format_binding_label(
                    button.tooltip_label(snapshot, "Board"),
                    button.binding_hint(snapshot),
                )),
            });
        }
    }

    *y += boards_card_h + section_gap;
}

fn board_icon(event: &ToolbarEvent) -> fn(&cairo::Context, f64, f64, f64) {
    match event {
        ToolbarEvent::BoardPrev => toolbar_icons::draw_icon_chevron_left,
        ToolbarEvent::BoardNext => toolbar_icons::draw_icon_chevron_right,
        ToolbarEvent::BoardNew => toolbar_icons::draw_icon_plus,
        ToolbarEvent::BoardDuplicate => toolbar_icons::draw_icon_copy,
        ToolbarEvent::BoardDelete => toolbar_icons::draw_icon_clear,
        _ => toolbar_icons::draw_icon_clear,
    }
}
