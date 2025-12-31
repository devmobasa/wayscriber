use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;

use super::super::widgets::*;

pub(super) fn draw_pages_section(layout: &mut SidePaletteLayout, y: &mut f64) {
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

    if !snapshot.show_actions_advanced {
        return;
    }

    let pages_card_h = layout.spec.side_pages_height(snapshot);
    draw_group_card(ctx, card_x, *y, card_w, pages_card_h);
    draw_section_label(
        ctx,
        x,
        *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
        "Pages",
    );

    let pages_y = *y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let btn_h = if use_icons {
        ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON
    } else {
        ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT
    };
    let btn_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
    let btn_w = (content_width - btn_gap * 4.0) / 5.0;
    let can_prev = snapshot.page_index > 0;
    let can_next = snapshot.page_index + 1 < snapshot.page_count;
    let buttons = [
        (
            ToolbarEvent::PagePrev,
            "Prev",
            toolbar_icons::draw_icon_undo as fn(&cairo::Context, f64, f64, f64),
            can_prev,
        ),
        (
            ToolbarEvent::PageNext,
            "Next",
            toolbar_icons::draw_icon_redo as fn(&cairo::Context, f64, f64, f64),
            can_next,
        ),
        (
            ToolbarEvent::PageNew,
            "New",
            toolbar_icons::draw_icon_plus as fn(&cairo::Context, f64, f64, f64),
            true,
        ),
        (
            ToolbarEvent::PageDuplicate,
            "Dup",
            toolbar_icons::draw_icon_save as fn(&cairo::Context, f64, f64, f64),
            true,
        ),
        (
            ToolbarEvent::PageDelete,
            "Del",
            toolbar_icons::draw_icon_clear as fn(&cairo::Context, f64, f64, f64),
            true,
        ),
    ];

    for (idx, (evt, label, icon_fn, enabled)) in buttons.iter().enumerate() {
        let bx = x + (btn_w + btn_gap) * idx as f64;
        let is_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, bx, pages_y, btn_w, btn_h))
            .unwrap_or(false);
        draw_button(ctx, bx, pages_y, btn_w, btn_h, *enabled, is_hover);
        if use_icons {
            if *enabled {
                set_icon_color(ctx, is_hover);
            } else {
                ctx.set_source_rgba(0.5, 0.5, 0.55, 0.5);
            }
            let icon_size = ToolbarLayoutSpec::SIDE_ACTION_ICON_SIZE;
            let icon_x = bx + (btn_w - icon_size) / 2.0;
            let icon_y = pages_y + (btn_h - icon_size) / 2.0;
            icon_fn(ctx, icon_x, icon_y, icon_size);
        } else {
            draw_label_center(ctx, bx, pages_y, btn_w, btn_h, label);
        }
        if *enabled {
            hits.push(HitRegion {
                rect: (bx, pages_y, btn_w, btn_h),
                event: evt.clone(),
                kind: HitKind::Click,
                tooltip: Some(format_binding_label(
                    label,
                    snapshot.binding_hints.binding_for_event(evt),
                )),
            });
        }
    }

    *y += pages_card_h + section_gap;
}
