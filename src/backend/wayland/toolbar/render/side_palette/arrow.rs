use super::super::widgets::constants::{
    COLOR_LABEL_HINT, FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL, FONT_SIZE_SMALL, set_color,
};
use super::super::widgets::*;
use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::input::Tool;
use crate::ui::toolbar::ToolbarEvent;
use crate::ui_text::{UiTextStyle, text_layout};

pub(super) fn draw_arrow_section(layout: &mut SidePaletteLayout, y: &mut f64) {
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
    let hint_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: FONT_SIZE_SMALL,
    };

    let show_arrow_controls = snapshot.active_tool == Tool::Arrow || snapshot.arrow_label_enabled;
    if !show_arrow_controls {
        return;
    }

    let card_h = if snapshot.arrow_label_enabled {
        ToolbarLayoutSpec::SIDE_TOGGLE_CARD_HEIGHT_WITH_RESET
    } else {
        ToolbarLayoutSpec::SIDE_TOGGLE_CARD_HEIGHT
    };
    draw_group_card(ctx, card_x, *y, card_w, card_h);
    draw_section_label(
        ctx,
        label_style,
        x,
        *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
        "Arrow labels",
    );
    if snapshot.arrow_label_enabled {
        let hint = format!("Next: {}", snapshot.arrow_label_next);
        let layout = text_layout(ctx, hint_style, &hint, None);
        let ext = layout.ink_extents();
        let hint_x = card_x + card_w - ext.width() - 8.0 - ext.x_bearing();
        let hint_y = *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL;
        set_color(ctx, COLOR_LABEL_HINT);
        layout.show_at_baseline(ctx, hint_x, hint_y);
    }

    let toggle_h = ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT;
    let toggle_y = *y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let toggle_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, toggle_y, content_width, toggle_h))
        .unwrap_or(false);
    draw_checkbox(
        ctx,
        x,
        toggle_y,
        content_width,
        toggle_h,
        snapshot.arrow_label_enabled,
        toggle_hover,
        label_style,
        "Auto-number",
    );
    hits.push(HitRegion {
        rect: (x, toggle_y, content_width, toggle_h),
        event: ToolbarEvent::ToggleArrowLabels(!snapshot.arrow_label_enabled),
        kind: HitKind::Click,
        tooltip: Some("Auto-number arrows 1, 2, 3.".to_string()),
    });

    if snapshot.arrow_label_enabled {
        let reset_y =
            toggle_y + ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT + ToolbarLayoutSpec::SIDE_TOGGLE_GAP;
        let reset_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, reset_y, content_width, toggle_h))
            .unwrap_or(false);
        draw_button(ctx, x, reset_y, content_width, toggle_h, false, reset_hover);
        draw_label_center(
            ctx,
            label_style,
            x,
            reset_y,
            content_width,
            toggle_h,
            "Reset",
        );
        hits.push(HitRegion {
            rect: (x, reset_y, content_width, toggle_h),
            event: ToolbarEvent::ResetArrowLabelCounter,
            kind: HitKind::Click,
            tooltip: Some("Reset numbering to 1.".to_string()),
        });
    }

    *y += card_h + section_gap;
}
