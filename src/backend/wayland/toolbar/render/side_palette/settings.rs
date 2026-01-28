use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::backend::wayland::toolbar::rows::{grid_layout, row_item_width};
use crate::config::{Action, action_label, action_short_label};
use crate::input::ToolbarDrawerTab;
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;
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

    if !snapshot.show_settings_section
        || !snapshot.drawer_open
        || snapshot.drawer_tab != ToolbarDrawerTab::App
    {
        return;
    }

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
    let mut toggles: Vec<(&str, bool, ToolbarEvent, Option<&str>)> = vec![
        (
            "Context UI",
            snapshot.context_aware_ui,
            ToolbarEvent::ToggleContextAwareUi(!snapshot.context_aware_ui),
            Some("Show/hide controls based on active tool."),
        ),
        (
            "Text controls",
            snapshot.show_text_controls,
            ToolbarEvent::ToggleTextControls(!snapshot.show_text_controls),
            Some("Text: font size/family."),
        ),
        (
            "Status bar",
            snapshot.show_status_bar,
            ToolbarEvent::ToggleStatusBar(!snapshot.show_status_bar),
            Some("Status bar: color/tool readout."),
        ),
        (
            "Status board",
            snapshot.show_status_board_badge,
            ToolbarEvent::ToggleStatusBoardBadge(!snapshot.show_status_board_badge),
            Some("Status bar: board label."),
        ),
        (
            "Status page",
            snapshot.show_status_page_badge,
            ToolbarEvent::ToggleStatusPageBadge(!snapshot.show_status_page_badge),
            Some("Status bar: page counter."),
        ),
        (
            "Overlay badge",
            snapshot.show_floating_badge_always,
            ToolbarEvent::ToggleFloatingBadgeAlways(!snapshot.show_floating_badge_always),
            Some("Board/page badge when status bar is visible."),
        ),
        (
            "Preset toasts",
            snapshot.show_preset_toasts,
            ToolbarEvent::TogglePresetToasts(!snapshot.show_preset_toasts),
            Some("Preset toasts: apply/save/clear."),
        ),
    ];
    if snapshot.layout_mode != crate::config::ToolbarLayoutMode::Simple {
        toggles.extend_from_slice(&[
            (
                "Show presets",
                snapshot.show_presets,
                ToolbarEvent::TogglePresets(!snapshot.show_presets),
                Some("Presets: quick slots."),
            ),
            (
                "Show actions",
                snapshot.show_actions_section,
                ToolbarEvent::ToggleActionsSection(!snapshot.show_actions_section),
                Some("Actions: undo/redo/clear."),
            ),
            (
                "Zoom actions",
                snapshot.show_zoom_actions,
                ToolbarEvent::ToggleZoomActions(!snapshot.show_zoom_actions),
                Some("Zoom: in/out/reset/lock."),
            ),
            (
                "Adv. Actions",
                snapshot.show_actions_advanced,
                ToolbarEvent::ToggleActionsAdvanced(!snapshot.show_actions_advanced),
                Some("Advanced: undo-all/delay/freeze."),
            ),
            (
                "Boards",
                snapshot.show_boards_section,
                ToolbarEvent::ToggleBoardsSection(!snapshot.show_boards_section),
                Some("Boards: prev/next/new/del."),
            ),
            (
                "Pages",
                snapshot.show_pages_section,
                ToolbarEvent::TogglePagesSection(!snapshot.show_pages_section),
                Some("Pages: prev/next/new/dup/del."),
            ),
            (
                "Step controls",
                snapshot.show_step_section,
                ToolbarEvent::ToggleStepSection(!snapshot.show_step_section),
                Some("Step: step undo/redo."),
            ),
        ]);
    }

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
    for (item, (label, value, event, tooltip)) in toggle_layout.items.iter().zip(toggles.iter()) {
        let toggle_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, item.x, item.y, item.w, item.h))
            .unwrap_or(false);
        draw_checkbox(
            ctx,
            item.x,
            item.y,
            item.w,
            item.h,
            *value,
            toggle_hover,
            toggle_style,
            label,
        );
        hits.push(HitRegion {
            rect: (item.x, item.y, item.w, item.h),
            event: event.clone(),
            kind: HitKind::Click,
            tooltip: tooltip.map(|text| text.to_string()),
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

    let button_layout = grid_layout(x, buttons_y, button_w, button_h, button_gap, 0.0, 2, 2);
    if let Some(item) = button_layout.items.first() {
        let config_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, item.x, item.y, item.w, item.h))
            .unwrap_or(false);
        draw_button(ctx, item.x, item.y, item.w, item.h, false, config_hover);
        if use_icons {
            set_icon_color(ctx, config_hover);
            toolbar_icons::draw_icon_settings(
                ctx,
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
                action_short_label(Action::OpenConfigurator),
            );
        }
        hits.push(HitRegion {
            rect: (item.x, item.y, item.w, item.h),
            event: ToolbarEvent::OpenConfigurator,
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                action_label(Action::OpenConfigurator),
                snapshot
                    .binding_hints
                    .binding_for_action(Action::OpenConfigurator),
            )),
        });
    }

    if let Some(item) = button_layout.items.get(1) {
        let file_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, item.x, item.y, item.w, item.h))
            .unwrap_or(false);
        draw_button(ctx, item.x, item.y, item.w, item.h, false, file_hover);
        if use_icons {
            set_icon_color(ctx, file_hover);
            toolbar_icons::draw_icon_file(
                ctx,
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
                "Config file",
            );
        }
        hits.push(HitRegion {
            rect: (item.x, item.y, item.w, item.h),
            event: ToolbarEvent::OpenConfigFile,
            kind: HitKind::Click,
            tooltip: Some("Config file".to_string()),
        });
    }

    *y += settings_card_h + section_gap;
}
