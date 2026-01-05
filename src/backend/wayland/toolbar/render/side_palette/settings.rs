use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::input::ToolbarDrawerTab;
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;

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
        x,
        *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
        "Settings",
    );

    let toggle_h = ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT;
    let toggle_gap = ToolbarLayoutSpec::SIDE_TOGGLE_GAP;
    let mut toggles: Vec<(&str, bool, ToolbarEvent, Option<&str>)> = vec![
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

    let mut toggle_y = *y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let toggle_col_gap = toggle_gap;
    let toggle_col_w = (content_width - toggle_col_gap) / 2.0;
    ctx.set_font_size(12.0);
    for row in 0..toggles.len().div_ceil(2) {
        for col in 0..2 {
            let idx = row * 2 + col;
            if idx >= toggles.len() {
                break;
            }
            let (label, value, event, tooltip) = &toggles[idx];
            let toggle_x = x + col as f64 * (toggle_col_w + toggle_col_gap);
            let toggle_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, toggle_x, toggle_y, toggle_col_w, toggle_h))
                .unwrap_or(false);
            draw_checkbox(
                ctx,
                toggle_x,
                toggle_y,
                toggle_col_w,
                toggle_h,
                *value,
                toggle_hover,
                label,
            );
            hits.push(HitRegion {
                rect: (toggle_x, toggle_y, toggle_col_w, toggle_h),
                event: event.clone(),
                kind: HitKind::Click,
                tooltip: tooltip.map(|text| text.to_string()),
            });
        }
        if row + 1 < toggles.len().div_ceil(2) {
            toggle_y += toggle_h + toggle_gap;
        } else {
            toggle_y += toggle_h;
        }
    }
    ctx.set_font_size(13.0);

    let buttons_y = toggle_y + toggle_gap;
    let button_h = ToolbarLayoutSpec::SIDE_SETTINGS_BUTTON_HEIGHT;
    let button_gap = ToolbarLayoutSpec::SIDE_SETTINGS_BUTTON_GAP;
    let button_w = (content_width - button_gap) / 2.0;
    let icon_size = 16.0;

    let config_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, buttons_y, button_w, button_h))
        .unwrap_or(false);
    draw_button(ctx, x, buttons_y, button_w, button_h, false, config_hover);
    if use_icons {
        set_icon_color(ctx, config_hover);
        toolbar_icons::draw_icon_settings(
            ctx,
            x + (button_w - icon_size) / 2.0,
            buttons_y + (button_h - icon_size) / 2.0,
            icon_size,
        );
    } else {
        draw_label_center(ctx, x, buttons_y, button_w, button_h, "Config UI");
    }
    hits.push(HitRegion {
        rect: (x, buttons_y, button_w, button_h),
        event: ToolbarEvent::OpenConfigurator,
        kind: HitKind::Click,
        tooltip: Some(format_binding_label(
            "Config UI",
            snapshot.binding_hints.open_configurator.as_deref(),
        )),
    });

    let file_x = x + button_w + button_gap;
    let file_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, file_x, buttons_y, button_w, button_h))
        .unwrap_or(false);
    draw_button(
        ctx, file_x, buttons_y, button_w, button_h, false, file_hover,
    );
    if use_icons {
        set_icon_color(ctx, file_hover);
        toolbar_icons::draw_icon_file(
            ctx,
            file_x + (button_w - icon_size) / 2.0,
            buttons_y + (button_h - icon_size) / 2.0,
            icon_size,
        );
    } else {
        draw_label_center(ctx, file_x, buttons_y, button_w, button_h, "Config file");
    }
    hits.push(HitRegion {
        rect: (file_x, buttons_y, button_w, button_h),
        event: ToolbarEvent::OpenConfigFile,
        kind: HitKind::Click,
        tooltip: Some("Config file".to_string()),
    });

    *y += settings_card_h + section_gap;
}
