use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::backend::wayland::toolbar::rows::{grid_layout, row_item_width};
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;
use crate::ui::toolbar::model::{ToolbarSessionButton, ToolbarSessionModel};
use crate::ui_text::{UiTextStyle, draw_text_baseline};

use super::super::widgets::constants::{
    COLOR_TEXT_DISABLED, FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL, set_color,
};
use super::super::widgets::{
    draw_button, draw_destructive_button, draw_group_card, draw_label_center, draw_section_label,
    point_in_rect, set_icon_color,
};

pub(super) fn draw_session_section(layout: &mut SidePaletteLayout, y: &mut f64) {
    let Some(model) = ToolbarSessionModel::from_snapshot(layout.snapshot) else {
        return;
    };

    let ctx = layout.ctx;
    let card_h = layout.spec.side_session_height(layout.snapshot);
    draw_group_card(ctx, layout.card_x, *y, layout.card_w, card_h);

    let label_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: FONT_SIZE_LABEL,
    };
    let meta_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 11.0,
    };
    let path_style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 9.5,
    };

    draw_section_label(
        ctx,
        label_style,
        layout.x,
        *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
        "Session",
    );

    let meta_y = *y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    draw_text_baseline(
        ctx,
        meta_style,
        &truncate_label(&model.active_name, 28),
        layout.x,
        meta_y + 12.0,
        None,
    );
    draw_text_baseline(
        ctx,
        path_style,
        &truncate_label(&model.active_path_label, 34),
        layout.x,
        meta_y + 27.0,
        None,
    );

    let mut row_y = meta_y
        + ToolbarLayoutSpec::SIDE_SESSION_META_HEIGHT
        + ToolbarLayoutSpec::SIDE_SESSION_ROW_GAP;
    draw_session_buttons(layout, &model, row_y, label_style);
    row_y +=
        ToolbarLayoutSpec::SIDE_SESSION_BUTTON_HEIGHT + ToolbarLayoutSpec::SIDE_SESSION_ROW_GAP;
    draw_recent_sessions(layout, &model, row_y, meta_style);

    *y += card_h + layout.section_gap;
}

fn draw_session_buttons(
    layout: &mut SidePaletteLayout,
    model: &ToolbarSessionModel,
    y: f64,
    label_style: UiTextStyle<'static>,
) {
    let button_gap = ToolbarLayoutSpec::SIDE_SESSION_ROW_GAP;
    let button_h = ToolbarLayoutSpec::SIDE_SESSION_BUTTON_HEIGHT;
    let button_w = row_item_width(layout.content_width, model.buttons.len(), button_gap);
    let grid = grid_layout(
        layout.x,
        y,
        button_w,
        button_h,
        button_gap,
        0.0,
        model.buttons.len().max(1),
        model.buttons.len(),
    );
    for (item, button) in grid.items.iter().zip(model.buttons.iter()) {
        draw_session_button(layout, button, item.x, item.y, item.w, item.h, label_style);
    }
}

fn draw_session_button(
    layout: &mut SidePaletteLayout,
    button: &ToolbarSessionButton,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    label_style: UiTextStyle<'static>,
) {
    let hover = layout
        .hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, y, w, h))
        .unwrap_or(false)
        && button.enabled;
    if matches!(button.event, ToolbarEvent::ClearSession) && button.enabled {
        draw_destructive_button(layout.ctx, x, y, w, h, hover);
    } else {
        draw_button(layout.ctx, x, y, w, h, false, hover);
    }

    if layout.snapshot.use_icons {
        if button.enabled {
            set_icon_color(layout.ctx, hover);
        } else {
            set_color(layout.ctx, COLOR_TEXT_DISABLED);
        }
        let icon_size = 15.0;
        let icon_x = x + (w - icon_size) / 2.0;
        let icon_y = y + (h - icon_size) / 2.0;
        match button.event {
            ToolbarEvent::OpenSession => {
                toolbar_icons::draw_icon_file(layout.ctx, icon_x, icon_y, icon_size)
            }
            ToolbarEvent::SaveSessionAs => {
                toolbar_icons::draw_icon_save(layout.ctx, icon_x, icon_y, icon_size)
            }
            ToolbarEvent::ClearSession => {
                toolbar_icons::draw_icon_clear(layout.ctx, icon_x, icon_y, icon_size)
            }
            _ => {}
        }
    } else {
        draw_label_center(layout.ctx, label_style, x, y, w, h, button.label);
    }

    if button.enabled {
        layout.hits.push(HitRegion {
            rect: (x, y, w, h),
            event: button.event.clone(),
            kind: HitKind::Click,
            tooltip: Some(button.label.to_string()),
        });
    }
}

fn draw_recent_sessions(
    layout: &mut SidePaletteLayout,
    model: &ToolbarSessionModel,
    mut y: f64,
    label_style: UiTextStyle<'static>,
) {
    for recent in &model.recents {
        let hover = layout
            .hover
            .map(|(hx, hy)| {
                point_in_rect(
                    hx,
                    hy,
                    layout.x,
                    y,
                    layout.content_width,
                    ToolbarLayoutSpec::SIDE_SESSION_RECENT_HEIGHT,
                )
            })
            .unwrap_or(false);
        draw_button(
            layout.ctx,
            layout.x,
            y,
            layout.content_width,
            ToolbarLayoutSpec::SIDE_SESSION_RECENT_HEIGHT,
            false,
            hover,
        );
        draw_text_baseline(
            layout.ctx,
            label_style,
            &truncate_label(&recent.label, 28),
            layout.x + 7.0,
            y + 14.0,
            None,
        );
        let tooltip_path = recent.path.display().to_string();
        layout.hits.push(HitRegion {
            rect: (
                layout.x,
                y,
                layout.content_width,
                ToolbarLayoutSpec::SIDE_SESSION_RECENT_HEIGHT,
            ),
            event: recent.event(),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                &format!("Open {}", recent.label),
                Some(tooltip_path.as_str()),
            )),
        });
        y +=
            ToolbarLayoutSpec::SIDE_SESSION_RECENT_HEIGHT + ToolbarLayoutSpec::SIDE_SESSION_ROW_GAP;
    }
}

fn truncate_label(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value.to_string()
    } else {
        let mut truncated = value
            .chars()
            .take(max_chars.saturating_sub(3))
            .collect::<String>();
        truncated.push_str("...");
        truncated
    }
}
