use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::toolbar_icons;
use crate::ui::toolbar::model::toolbar_pages_model;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSideSection};
use crate::ui_text::UiTextStyle;

use super::super::widgets::constants::{
    COLOR_TEXT_DISABLED, FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL, set_color,
};
use super::super::widgets::*;
use super::section_header::draw_collapsible_header;

pub(super) fn draw_pages_section(layout: &mut SidePaletteLayout, y: &mut f64) {
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

    let Some(model) = toolbar_pages_model(snapshot) else {
        return;
    };

    let pages_card_h = layout.spec.side_pages_height(snapshot);
    draw_group_card(ctx, card_x, *y, card_w, pages_card_h);
    draw_collapsible_header(
        layout,
        *y,
        label_style,
        ToolbarSideSection::Pages,
        ToolbarSideSection::Pages.label(),
        ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
    );
    if snapshot.side_section_collapsed(ToolbarSideSection::Pages) {
        *y += pages_card_h + section_gap;
        return;
    }

    let hits = &mut layout.hits;
    let pages_y = *y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let btn_h = if use_icons {
        ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON
    } else {
        ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT
    };
    let btn_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
    let rects = super::side_row_button_rects(
        super::SideRowLayout {
            x,
            row_y: pages_y,
            content_width,
            btn_h,
            btn_gap,
            use_icons,
            text_columns: model.buttons.len(),
        },
        &model.buttons,
    );
    for (rect, button) in rects.iter().zip(model.buttons.iter()) {
        let label = button.short_label(snapshot, "Page");
        let (bx, by, btn_w, btn_h) = *rect;
        let is_hover = button.enabled
            && hover
                .map(|(hx, hy)| point_in_rect(hx, hy, bx, by, btn_w, btn_h))
                .unwrap_or(false);
        if !button.enabled {
            draw_disabled_button(ctx, bx, by, btn_w, btn_h);
        } else if button.event.is_destructive() {
            draw_destructive_button(ctx, bx, by, btn_w, btn_h, is_hover);
        } else {
            draw_button(ctx, bx, by, btn_w, btn_h, false, is_hover);
        }
        if use_icons {
            if button.enabled {
                set_icon_color(ctx, is_hover);
            } else {
                set_color(ctx, COLOR_TEXT_DISABLED);
            }
            let icon_size = ToolbarLayoutSpec::SIDE_ACTION_ICON_SIZE;
            let icon_x = bx + (btn_w - icon_size) / 2.0;
            let icon_y = by + (btn_h - icon_size) / 2.0;
            page_icon(&button.event)(ctx, icon_x, icon_y, icon_size);
        } else if button.enabled {
            draw_label_center(ctx, label_style, bx, by, btn_w, btn_h, label);
        } else {
            draw_label_center_color(
                ctx,
                label_style,
                bx,
                by,
                btn_w,
                btn_h,
                label,
                COLOR_TEXT_DISABLED,
            );
        }
        if button.enabled {
            hits.push(HitRegion {
                focus_id: None,
                rect: (bx, by, btn_w, btn_h),
                event: button.event.clone(),
                kind: HitKind::Click,
                tooltip: Some(format_binding_label(
                    button.tooltip_label(snapshot, "Page"),
                    button.binding_hint(snapshot),
                )),
            });
        }
    }

    *y += pages_card_h + section_gap;
}

fn page_icon(event: &ToolbarEvent) -> fn(&cairo::Context, f64, f64, f64) {
    match event {
        ToolbarEvent::PagePrev => toolbar_icons::draw_icon_chevron_left,
        ToolbarEvent::PageNext => toolbar_icons::draw_icon_chevron_right,
        ToolbarEvent::PageNew => toolbar_icons::draw_icon_plus,
        ToolbarEvent::PageDuplicate => toolbar_icons::draw_icon_copy,
        ToolbarEvent::PageDelete => toolbar_icons::draw_icon_clear,
        _ => toolbar_icons::draw_icon_clear,
    }
}
