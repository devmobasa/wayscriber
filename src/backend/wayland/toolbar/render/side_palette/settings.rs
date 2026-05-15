use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::backend::wayland::toolbar::rows::{grid_layout, row_item_width};
use crate::toolbar_icons;
use crate::ui::toolbar::model::{ToolbarActivation, ToolbarIcon, ToolbarSettingsModel};
use crate::ui_text::UiTextStyle;

use super::super::widgets::constants::{FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL};
use super::super::widgets::*;

pub(super) fn draw_settings_section(layout: &mut SidePaletteLayout, y: &mut f64) {
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
    draw_section_label(
        ctx,
        label_style,
        x,
        *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
        "Settings",
    );

    let toggle_h = ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT;
    let toggle_gap = ToolbarLayoutSpec::SIDE_TOGGLE_GAP;
    let toggles = settings_model.toggles();

    let toggle_y = *y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let toggle_col_gap = toggle_gap;
    let toggle_col_w = row_item_width(content_width, 2, toggle_col_gap);
    let toggle_layout = grid_layout(
        x,
        toggle_y,
        toggle_col_w,
        toggle_h,
        toggle_col_gap,
        toggle_gap,
        2,
        toggles.len(),
    );
    for (item, toggle) in toggle_layout.items.iter().zip(toggles.iter()) {
        let toggle_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, item.x, item.y, item.w, item.h))
            .unwrap_or(false);
        draw_checkbox(
            ctx,
            item.x,
            item.y,
            item.w,
            item.h,
            toggle.checked,
            toggle_hover,
            toggle_style,
            toggle.label.as_ref(),
        );
        hits.push(HitRegion {
            rect: (item.x, item.y, item.w, item.h),
            event: activation_event(&toggle.activation),
            kind: HitKind::Click,
            tooltip: toggle.tooltip.as_string(),
        });
    }

    let mut buttons_y = toggle_y;
    if toggle_layout.rows > 0 {
        buttons_y += toggle_layout.height;
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
        0.0,
        2,
        buttons.len(),
    );
    for (item, button) in button_layout.items.iter().zip(buttons.iter()) {
        let button_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, item.x, item.y, item.w, item.h))
            .unwrap_or(false);
        draw_button(ctx, item.x, item.y, item.w, item.h, false, button_hover);
        if use_icons {
            set_icon_color(ctx, button_hover);
            draw_settings_icon(
                ctx,
                button.icon,
                item.x + (item.w - icon_size) / 2.0,
                item.y + (item.h - icon_size) / 2.0,
                icon_size,
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
            rect: (item.x, item.y, item.w, item.h),
            event: button.event.clone(),
            kind: HitKind::Click,
            tooltip: button.tooltip.as_string(),
        });
    }

    *y += settings_card_h + section_gap;
}

fn activation_event(activation: &ToolbarActivation) -> crate::ui::toolbar::ToolbarEvent {
    activation.compatibility_event()
}

fn draw_settings_icon(ctx: &cairo::Context, icon: ToolbarIcon, x: f64, y: f64, size: f64) {
    match icon {
        ToolbarIcon::Settings => toolbar_icons::draw_icon_settings(ctx, x, y, size),
        ToolbarIcon::File => toolbar_icons::draw_icon_file(ctx, x, y, size),
        ToolbarIcon::More | ToolbarIcon::Board => {}
    }
}
