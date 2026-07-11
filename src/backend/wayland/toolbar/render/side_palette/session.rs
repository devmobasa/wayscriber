use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::backend::wayland::toolbar::rows::{grid_layout, row_item_width};
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;
use crate::ui::toolbar::model::{ToolbarSessionButton, ToolbarSessionModel};
use crate::ui_text::{UiTextStyle, draw_text_baseline, text_layout};

use super::super::widgets::constants::{
    COLOR_TEXT_DISABLED, COLOR_TEXT_PRIMARY, FONT_FAMILY_DEFAULT, FONT_SIZE_LABEL, set_color,
};
use super::super::widgets::{
    draw_button, draw_destructive_button, draw_group_card, draw_label_center, point_in_rect,
    set_icon_color,
};
use super::section_header::draw_collapsible_header;
use crate::ui::toolbar::ToolbarSideSection;

const SESSION_BUTTON_ICON_SIZE: f64 = 12.0;
const SESSION_BUTTON_ICON_GAP: f64 = 4.0;
const SESSION_BUTTON_ICON_LABEL_SIZE: f64 = 11.0;
const SESSION_RECENT_ICON_SIZE: f64 = 13.0;
const SESSION_RECENT_ICON_GAP: f64 = 6.0;
const SESSION_RECENT_PADDING_X: f64 = 7.0;

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

    draw_collapsible_header(
        layout,
        *y,
        label_style,
        ToolbarSideSection::Session,
        ToolbarSideSection::Session.label(),
        ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
    );
    if layout
        .snapshot
        .side_section_collapsed(ToolbarSideSection::Session)
    {
        *y += card_h + layout.section_gap;
        return;
    }

    let meta_y = *y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    draw_text_baseline(
        ctx,
        meta_style,
        &truncate_middle(&model.active_name, 28),
        layout.x,
        meta_y + 12.0,
        None,
    );
    draw_text_baseline(
        ctx,
        path_style,
        &truncate_start(&model.active_path_label, 34),
        layout.x,
        meta_y + 27.0,
        None,
    );

    let mut row_y = meta_y
        + ToolbarLayoutSpec::SIDE_SESSION_META_HEIGHT
        + ToolbarLayoutSpec::SIDE_SESSION_ROW_GAP;
    let controls_h = draw_session_controls(layout, &model, row_y, label_style, meta_style);
    row_y += controls_h + ToolbarLayoutSpec::SIDE_SESSION_ROW_GAP;
    draw_recent_sessions(layout, &model, row_y, meta_style);

    *y += card_h + layout.section_gap;
}

fn draw_session_controls(
    layout: &mut SidePaletteLayout,
    model: &ToolbarSessionModel,
    y: f64,
    label_style: UiTextStyle<'static>,
    meta_style: UiTextStyle<'static>,
) -> f64 {
    if model.overwrite_confirmation.is_some() {
        draw_save_as_overwrite_confirmation(layout, model, y, label_style, meta_style)
    } else {
        draw_session_buttons(layout, model, y, label_style)
    }
}

fn draw_session_buttons(
    layout: &mut SidePaletteLayout,
    model: &ToolbarSessionModel,
    y: f64,
    label_style: UiTextStyle<'static>,
) -> f64 {
    let button_gap = ToolbarLayoutSpec::SIDE_SESSION_ROW_GAP;
    let button_h = ToolbarLayoutSpec::SIDE_SESSION_BUTTON_HEIGHT;
    let columns = model.button_columns();
    let button_w = row_item_width(layout.content_width, columns, button_gap);
    let button_label_style = if layout.snapshot.use_icons {
        UiTextStyle {
            size: SESSION_BUTTON_ICON_LABEL_SIZE,
            ..label_style
        }
    } else {
        label_style
    };
    let grid = grid_layout(
        layout.x,
        y,
        button_w,
        button_h,
        button_gap,
        button_gap,
        columns,
        model.buttons.len(),
    );
    for (item, button) in grid.items.iter().zip(model.buttons.iter()) {
        draw_session_button(
            layout,
            button,
            item.x,
            item.y,
            item.w,
            item.h,
            button_label_style,
        );
    }
    grid.height
}

fn draw_save_as_overwrite_confirmation(
    layout: &mut SidePaletteLayout,
    model: &ToolbarSessionModel,
    y: f64,
    label_style: UiTextStyle<'static>,
    meta_style: UiTextStyle<'static>,
) -> f64 {
    let Some(confirmation) = model.overwrite_confirmation.as_ref() else {
        return 0.0;
    };
    let message = format!("Replace {}?", truncate_middle(&confirmation.label, 20));
    draw_text_baseline(layout.ctx, meta_style, &message, layout.x, y + 11.0, None);

    let button_gap = ToolbarLayoutSpec::SIDE_SESSION_ROW_GAP;
    let button_h = ToolbarLayoutSpec::SIDE_SESSION_BUTTON_HEIGHT;
    let button_y = y + ToolbarLayoutSpec::SIDE_SESSION_CONFIRM_LABEL_HEIGHT;
    let button_w = row_item_width(layout.content_width, 2, button_gap);
    let grid = grid_layout(
        layout.x, button_y, button_w, button_h, button_gap, button_gap, 2, 2,
    );
    let actions = [
        ("Replace", confirmation.confirm_event(), true),
        ("Cancel", confirmation.cancel_event(), false),
    ];
    for (item, (label, event, destructive)) in grid.items.iter().zip(actions) {
        let hover = layout
            .hover
            .map(|(hx, hy)| point_in_rect(hx, hy, item.x, item.y, item.w, item.h))
            .unwrap_or(false);
        if destructive {
            draw_destructive_button(layout.ctx, item.x, item.y, item.w, item.h, hover);
        } else {
            draw_button(layout.ctx, item.x, item.y, item.w, item.h, false, hover);
        }
        draw_label_center(
            layout.ctx,
            label_style,
            item.x,
            item.y,
            item.w,
            item.h,
            label,
        );
        layout.hits.push(HitRegion {
            focus_id: None,
            rect: (item.x, item.y, item.w, item.h),
            event,
            kind: HitKind::Click,
            tooltip: Some(label.to_string()),
        });
    }

    ToolbarLayoutSpec::SIDE_SESSION_CONFIRM_LABEL_HEIGHT + grid.height
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
        draw_session_button_icon_label(layout, button, x, y, w, h, label_style, hover);
    } else {
        draw_label_center(layout.ctx, label_style, x, y, w, h, button.label);
    }

    if button.enabled {
        layout.hits.push(HitRegion {
            focus_id: None,
            rect: (x, y, w, h),
            event: button.event.clone(),
            kind: HitKind::Click,
            tooltip: Some(button.label.to_string()),
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_session_button_icon_label(
    layout: &SidePaletteLayout,
    button: &ToolbarSessionButton,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    label_style: UiTextStyle<'static>,
    hover: bool,
) {
    let Some(icon) = session_button_icon(&button.event) else {
        return;
    };

    let label_layout = text_layout(layout.ctx, label_style, button.label, None);
    let label_ext = label_layout.ink_extents();
    let group_w = SESSION_BUTTON_ICON_SIZE + SESSION_BUTTON_ICON_GAP + label_ext.width();
    let group_x = (x + (w - group_w) / 2.0).max(x + 3.0);
    let icon_y = y + (h - SESSION_BUTTON_ICON_SIZE) / 2.0;
    let label_x =
        group_x + SESSION_BUTTON_ICON_SIZE + SESSION_BUTTON_ICON_GAP - label_ext.x_bearing();
    let label_y = y + (h - label_ext.height()) / 2.0 - label_ext.y_bearing();

    if button.enabled {
        set_icon_color(layout.ctx, hover);
    } else {
        set_color(layout.ctx, COLOR_TEXT_DISABLED);
    }
    icon(layout.ctx, group_x, icon_y, SESSION_BUTTON_ICON_SIZE);

    if button.enabled {
        set_color(layout.ctx, COLOR_TEXT_PRIMARY);
    } else {
        set_color(layout.ctx, COLOR_TEXT_DISABLED);
    }
    label_layout.show_at_baseline(layout.ctx, label_x, label_y);
}

fn session_button_icon(event: &ToolbarEvent) -> Option<fn(&cairo::Context, f64, f64, f64)> {
    match event {
        ToolbarEvent::OpenSession => Some(toolbar_icons::draw_icon_file),
        ToolbarEvent::SaveSessionAs => Some(toolbar_icons::draw_icon_save),
        ToolbarEvent::SessionInfo => Some(toolbar_icons::draw_icon_info),
        ToolbarEvent::ClearSession => Some(toolbar_icons::draw_icon_clear),
        ToolbarEvent::OpenConfigurator => Some(toolbar_icons::draw_icon_settings),
        _ => None,
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
        set_icon_color(layout.ctx, hover);
        let icon_x = layout.x + SESSION_RECENT_PADDING_X;
        let icon_y =
            y + (ToolbarLayoutSpec::SIDE_SESSION_RECENT_HEIGHT - SESSION_RECENT_ICON_SIZE) / 2.0;
        toolbar_icons::draw_icon_file(layout.ctx, icon_x, icon_y, SESSION_RECENT_ICON_SIZE);
        set_color(layout.ctx, COLOR_TEXT_PRIMARY);
        draw_text_baseline(
            layout.ctx,
            label_style,
            &truncate_middle(strip_session_extension(&recent.label), 24),
            icon_x + SESSION_RECENT_ICON_SIZE + SESSION_RECENT_ICON_GAP,
            y + 14.0,
            None,
        );
        let tooltip_path = recent.path.display().to_string();
        layout.hits.push(HitRegion {
            focus_id: None,
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

/// Middle-ellipsize so both the head and the distinguishing tail survive.
/// Tail truncation made e.g. two different "lecture-05-…" files render
/// identically in the recents list.
fn truncate_middle(value: &str, max_chars: usize) -> String {
    let count = value.chars().count();
    if count <= max_chars {
        return value.to_string();
    }
    let keep = max_chars.saturating_sub(1).max(2);
    let head = keep.div_ceil(2);
    let tail = keep - head;
    let mut truncated: String = value.chars().take(head).collect();
    truncated.push('…');
    truncated.extend(value.chars().skip(count - tail));
    truncated
}

/// Keep the tail of a path — the leading directories are the least
/// informative part of a session path.
fn truncate_start(value: &str, max_chars: usize) -> String {
    let count = value.chars().count();
    if count <= max_chars {
        return value.to_string();
    }
    let keep = max_chars.saturating_sub(1);
    let mut truncated = String::from("…");
    truncated.extend(value.chars().skip(count - keep));
    truncated
}

/// Drop the constant session-file extension in list rows; it costs the
/// characters that distinguish one session from another.
fn strip_session_extension(value: &str) -> &str {
    value
        .strip_suffix(".wayscriber-session")
        .or_else(|| value.strip_suffix(".wayscriber"))
        .unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_middle_keeps_head_and_tail() {
        assert_eq!(truncate_middle("short", 10), "short");
        let truncated = truncate_middle("lecture-05-quantum-mechanics-part-two", 20);
        assert_eq!(truncated.chars().count(), 20);
        assert!(truncated.starts_with("lecture-05"));
        assert!(truncated.ends_with("part-two"));
        assert!(truncated.contains('…'));
    }

    #[test]
    fn truncate_start_keeps_path_tail() {
        assert_eq!(truncate_start("/tmp/x", 10), "/tmp/x");
        let truncated = truncate_start("/home/user/.local/share/wayscriber/sessions", 20);
        assert_eq!(truncated.chars().count(), 20);
        assert!(truncated.starts_with('…'));
        assert!(truncated.ends_with("wayscriber/sessions"));
    }

    #[test]
    fn strip_session_extension_drops_known_suffixes() {
        assert_eq!(
            strip_session_extension("lecture.wayscriber-session"),
            "lecture"
        );
        assert_eq!(strip_session_extension("lecture.wayscriber"), "lecture");
        assert_eq!(strip_session_extension("lecture.txt"), "lecture.txt");
    }
}
