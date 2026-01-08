use super::{
    HitKind, HitRegion, SideLayoutContext, ToolbarEvent, ToolbarLayoutMode, ToolbarLayoutSpec,
    format_binding_label,
};
use crate::backend::wayland::toolbar::rows::{grid_layout, row_item_width};
use crate::config::{Action, action_label};

pub(super) fn push_settings_hits(ctx: &SideLayoutContext<'_>, y: f64, hits: &mut Vec<HitRegion>) {
    if !ctx.snapshot.show_settings_section
        || !ctx.snapshot.drawer_open
        || ctx.snapshot.drawer_tab != crate::input::ToolbarDrawerTab::App
    {
        return;
    }

    let toggle_h = ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT;
    let toggle_gap = ToolbarLayoutSpec::SIDE_TOGGLE_GAP;
    let mut toggles: Vec<(ToolbarEvent, Option<&str>)> = vec![
        (
            ToolbarEvent::ToggleTextControls(!ctx.snapshot.show_text_controls),
            Some("Text: font size/family."),
        ),
        (
            ToolbarEvent::ToggleStatusBar(!ctx.snapshot.show_status_bar),
            Some("Status bar: color/tool readout."),
        ),
        (
            ToolbarEvent::TogglePresetToasts(!ctx.snapshot.show_preset_toasts),
            Some("Preset toasts: apply/save/clear."),
        ),
    ];
    if ctx.snapshot.layout_mode != ToolbarLayoutMode::Simple {
        toggles.extend_from_slice(&[
            (
                ToolbarEvent::TogglePresets(!ctx.snapshot.show_presets),
                Some("Presets: quick slots."),
            ),
            (
                ToolbarEvent::ToggleActionsSection(!ctx.snapshot.show_actions_section),
                Some("Actions: undo/redo/clear."),
            ),
            (
                ToolbarEvent::ToggleZoomActions(!ctx.snapshot.show_zoom_actions),
                Some("Zoom: in/out/reset/lock."),
            ),
            (
                ToolbarEvent::ToggleActionsAdvanced(!ctx.snapshot.show_actions_advanced),
                Some("Advanced: undo-all/delay/freeze."),
            ),
            (
                ToolbarEvent::TogglePagesSection(!ctx.snapshot.show_pages_section),
                Some("Pages: prev/next/new/dup/del."),
            ),
            (
                ToolbarEvent::ToggleStepSection(!ctx.snapshot.show_step_section),
                Some("Step: step undo/redo."),
            ),
        ]);
    }

    let toggle_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let toggle_col_gap = toggle_gap;
    let toggle_col_w = row_item_width(ctx.content_width, 2, toggle_col_gap);
    let toggle_layout = grid_layout(
        ctx.x,
        toggle_y,
        toggle_col_w,
        toggle_h,
        toggle_col_gap,
        toggle_gap,
        2,
        toggles.len(),
    );
    for (item, (evt, tooltip)) in toggle_layout.items.iter().zip(toggles.iter()) {
        hits.push(HitRegion {
            rect: (item.x, item.y, item.w, item.h),
            event: evt.clone(),
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
    let button_w = row_item_width(ctx.content_width, 2, button_gap);
    let button_layout = grid_layout(ctx.x, buttons_y, button_w, button_h, button_gap, 0.0, 2, 2);
    if let Some(item) = button_layout.items.first() {
        hits.push(HitRegion {
            rect: (item.x, item.y, item.w, item.h),
            event: ToolbarEvent::OpenConfigurator,
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                action_label(Action::OpenConfigurator),
                ctx.snapshot
                    .binding_hints
                    .binding_for_action(Action::OpenConfigurator),
            )),
        });
    }
    if let Some(item) = button_layout.items.get(1) {
        hits.push(HitRegion {
            rect: (item.x, item.y, item.w, item.h),
            event: ToolbarEvent::OpenConfigFile,
            kind: HitKind::Click,
            tooltip: Some("Config file".to_string()),
        });
    }
}
