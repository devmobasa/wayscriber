use super::{
    format_binding_label, HitKind, HitRegion, SideLayoutContext, ToolbarEvent, ToolbarLayoutMode,
    ToolbarLayoutSpec,
};

pub(super) fn push_settings_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) {
    if !ctx.snapshot.show_settings_section {
        return;
    }

    let toggle_h = ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT;
    let toggle_gap = ToolbarLayoutSpec::SIDE_TOGGLE_GAP;
    let mut toggles: Vec<(ToolbarEvent, Option<&str>)> = vec![
        (
            ToolbarEvent::ToggleToolPreview(!ctx.snapshot.show_tool_preview),
            Some("Tool preview: cursor bubble."),
        ),
        (
            ToolbarEvent::TogglePresetToasts(!ctx.snapshot.show_preset_toasts),
            Some("Preset toasts: apply/save/clear."),
        ),
    ];
    if ctx.snapshot.layout_mode == ToolbarLayoutMode::Advanced {
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
                ToolbarEvent::ToggleActionsAdvanced(!ctx.snapshot.show_actions_advanced),
                Some("Advanced: undo-all/delay/zoom."),
            ),
            (
                ToolbarEvent::ToggleStepSection(!ctx.snapshot.show_step_section),
                Some("Step: step undo/redo."),
            ),
            (
                ToolbarEvent::ToggleTextControls(!ctx.snapshot.show_text_controls),
                Some("Text: font size/family."),
            ),
        ]);
    }

    let mut toggle_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    for (idx, (evt, tooltip)) in toggles.iter().enumerate() {
        hits.push(HitRegion {
            rect: (ctx.x, toggle_y, ctx.content_width, toggle_h),
            event: evt.clone(),
            kind: HitKind::Click,
            tooltip: tooltip.map(|text| text.to_string()),
        });
        if idx + 1 < toggles.len() {
            toggle_y += toggle_h + toggle_gap;
        } else {
            toggle_y += toggle_h;
        }
    }

    let buttons_y = toggle_y + toggle_gap;
    let button_h = ToolbarLayoutSpec::SIDE_SETTINGS_BUTTON_HEIGHT;
    let button_gap = ToolbarLayoutSpec::SIDE_SETTINGS_BUTTON_GAP;
    let button_w = (ctx.content_width - button_gap) / 2.0;
    hits.push(HitRegion {
        rect: (ctx.x, buttons_y, button_w, button_h),
        event: ToolbarEvent::OpenConfigurator,
        kind: HitKind::Click,
        tooltip: Some(format_binding_label(
            "Config UI",
            ctx.snapshot.binding_hints.open_configurator.as_deref(),
        )),
    });
    hits.push(HitRegion {
        rect: (ctx.x + button_w + button_gap, buttons_y, button_w, button_h),
        event: ToolbarEvent::OpenConfigFile,
        kind: HitKind::Click,
        tooltip: Some("Config file".to_string()),
    });
}
