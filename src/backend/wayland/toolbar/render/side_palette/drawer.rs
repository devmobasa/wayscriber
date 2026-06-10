use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::input::ToolbarDrawerTab;
use crate::ui::toolbar::ToolbarEvent;
use crate::ui_text::UiTextStyle;

use super::super::widgets::constants::{FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL};
use super::super::widgets::*;

pub(super) fn draw_drawer_tabs(layout: &mut SidePaletteLayout, y: &mut f64) {
    let ctx = layout.ctx;
    let snapshot = layout.snapshot;
    let hits = &mut layout.hits;
    let hover = layout.hover;
    let x = layout.x;
    let card_x = layout.card_x;
    let card_w = layout.card_w;
    let content_width = layout.content_width;
    let section_gap = layout.section_gap;
    let label_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: FONT_SIZE_LABEL,
    };

    if !snapshot.drawer_open {
        return;
    }

    let tabs_card_h = layout.spec.side_drawer_tabs_height(snapshot);
    draw_group_card(ctx, card_x, *y, card_w, tabs_card_h);

    let tab_y = *y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let tab_h = ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT;
    let tab_gap = ToolbarLayoutSpec::SIDE_TOGGLE_GAP;
    let tabs = ToolbarDrawerTab::ALL;
    let tab_columns = 3usize;
    let tab_w = (content_width - tab_gap * (tab_columns - 1) as f64) / tab_columns as f64;

    for (idx, tab) in tabs.iter().enumerate() {
        let tab_col = idx % tab_columns;
        let tab_row = idx / tab_columns;
        let tab_x = x + (tab_w + tab_gap) * tab_col as f64;
        let tab_y = tab_y + (tab_h + tab_gap) * tab_row as f64;
        let tab_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, tab_x, tab_y, tab_w, tab_h))
            .unwrap_or(false);
        draw_button(
            ctx,
            tab_x,
            tab_y,
            tab_w,
            tab_h,
            snapshot.drawer_tab == *tab,
            tab_hover,
        );
        draw_label_center(ctx, label_style, tab_x, tab_y, tab_w, tab_h, tab.label());
        hits.push(HitRegion {
            rect: (tab_x, tab_y, tab_w, tab_h),
            event: ToolbarEvent::SetDrawerTab(*tab),
            kind: HitKind::Click,
            tooltip: Some(format!("Drawer: {}", tab.label())),
        });
    }

    *y += tabs_card_h + section_gap;
}
