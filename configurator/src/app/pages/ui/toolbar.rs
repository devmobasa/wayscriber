use super::*;

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let (layout_modes, layout_labels) =
        options(ToolbarLayoutModeOption::list(), |value| value.label());
    let (side_layouts, side_layout_labels) =
        options(ToolbarSideLayoutOption::list(), |value| value.label());
    let (zoom_chips, zoom_chip_labels) =
        options(ZoomChipDisplayOption::list(), |value| value.label());
    let (rebinds, rebind_labels) = options(ToolbarRebindModifierOption::ALL.to_vec(), |value| {
        value.label()
    });
    let (override_modes, override_mode_labels) =
        options(ToolbarLayoutModeOption::list(), |value| value.label());

    let mut page = PageBuilder::new(sender, TabId::Ui);

    page.group("Toolbar").custom(&note(
        "These settings are configured defaults. Toolbar pins, position, display form, item visibility/order, pane state, and board pins changed in the overlay are saved separately as runtime preferences.",
    ));

    page.group("Layout")
        .combo_row(
            "Layout mode",
            "",
            layout_modes,
            layout_labels,
            |app| app.draft.ui_toolbar_layout_mode,
            Message::ToolbarLayoutModeChanged,
        )
        .combo_row(
            "Side layout",
            "Pill (the default) retires the side palette: drawing properties live in the top strip's style pill, canvas management in the status HUD and board picker, and Session/Settings in popovers on the top strip's overflow menu. Panel is the legacy escape hatch restoring the classic side palette; it is deprecated and planned for removal one release after the pill default.",
            side_layouts,
            side_layout_labels,
            |app| app.draft.ui_toolbar_side_layout,
            Message::ToolbarSideLayoutChanged,
        )
        .combo_row(
            "Zoom chip",
            "",
            zoom_chips,
            zoom_chip_labels,
            |app| app.draft.ui_toolbar_zoom_chip_display,
            Message::ToolbarZoomChipDisplayChanged,
        )
        .switch_row(
            "Show zoom chip",
            "",
            |app| app.draft.ui_toolbar_show_zoom_chip,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowZoomChip, value),
        )
        .combo_row(
            "Shortcut edit click",
            "",
            rebinds,
            rebind_labels,
            |app| app.draft.ui_toolbar_rebind_modifier,
            Message::ToolbarRebindModifierChanged,
        )
        .switch_row(
            "Configured default: pin top toolbar",
            "",
            |app| app.draft.ui_toolbar_top_pinned,
            |value| Message::ToggleChanged(ToggleField::UiToolbarTopPinned, value),
        )
        .switch_row(
            "Configured default: pin side toolbar",
            "",
            |app| app.draft.ui_toolbar_side_pinned,
            |value| Message::ToggleChanged(ToggleField::UiToolbarSidePinned, value),
        )
        .switch_row(
            "Use icon-only buttons",
            "",
            |app| app.draft.ui_toolbar_use_icons,
            |value| Message::ToggleChanged(ToggleField::UiToolbarUseIcons, value),
        );

    page.group("Sections")
        .switch_row(
            "Show extended colors",
            "",
            |app| app.draft.ui_toolbar_show_more_colors,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowMoreColors, value),
        )
        .switch_row(
            "Show presets",
            "",
            |app| app.draft.ui_toolbar_show_presets,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowPresets, value),
        )
        .switch_row(
            "Show actions",
            "",
            |app| app.draft.ui_toolbar_show_actions_section,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowActionsSection, value),
        )
        .switch_row(
            "Show zoom actions",
            "",
            |app| app.draft.ui_toolbar_show_zoom_actions,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowZoomActions, value),
        )
        .switch_row(
            "Show advanced actions",
            "",
            |app| app.draft.ui_toolbar_show_actions_advanced,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowActionsAdvanced, value),
        )
        .switch_row(
            "Show pages section",
            "",
            |app| app.draft.ui_toolbar_show_pages_section,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowPagesSection, value),
        )
        .switch_row(
            "Show boards section",
            "",
            |app| app.draft.ui_toolbar_show_boards_section,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowBoardsSection, value),
        )
        .switch_row(
            "Show multi-step undo/redo",
            "",
            |app| app.draft.ui_toolbar_show_step_section,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowStepSection, value),
        )
        .switch_row(
            "Always show text controls",
            "",
            |app| app.draft.ui_toolbar_show_text_controls,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowTextControls, value),
        )
        .switch_row(
            "Show delay sliders",
            "",
            |app| app.draft.ui_toolbar_show_delay_sliders,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowDelaySliders, value),
        )
        .switch_row(
            "Show marker opacity controls",
            "",
            |app| app.draft.ui_toolbar_show_marker_opacity_section,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowMarkerOpacitySection, value),
        )
        .switch_row(
            "Show tool preview bubble",
            "",
            |app| app.draft.ui_toolbar_show_tool_preview,
            |value| Message::ToggleChanged(ToggleField::UiToolbarShowToolPreview, value),
        )
        .switch_row(
            "Show preset action toasts",
            "",
            |app| app.draft.ui_toolbar_show_preset_toasts,
            |value| Message::ToggleChanged(ToggleField::UiToolbarPresetToasts, value),
        )
        .switch_row(
            "Force inline toolbars",
            "",
            |app| app.draft.ui_toolbar_force_inline,
            |value| Message::ToggleChanged(ToggleField::UiToolbarForceInline, value),
        );

    page.group("Mode overrides").combo_row(
        "Edit mode",
        "Overrides below apply to the mode selected here; \"Default\" keeps the mode preset.",
        override_modes,
        override_mode_labels,
        |app| app.override_mode,
        Message::ToolbarOverrideModeChanged,
    );
    for field in [
        ToolbarOverrideField::ShowPresets,
        ToolbarOverrideField::ShowActionsSection,
        ToolbarOverrideField::ShowZoomActions,
        ToolbarOverrideField::ShowActionsAdvanced,
        ToolbarOverrideField::ShowPagesSection,
        ToolbarOverrideField::ShowBoardsSection,
        ToolbarOverrideField::ShowStepSection,
        ToolbarOverrideField::ShowTextControls,
    ] {
        let (values, labels) = options(OverrideOption::list(), |value| value.label());
        page.combo_row(
            field.label(),
            "",
            values,
            labels,
            move |app| toolbar_override(app, field),
            move |value| Message::ToolbarOverrideChanged(field, value),
        );
    }

    page.group("Placement offsets")
        .entry_row(
            "Top offset X (px)",
            |app| app.draft.ui_toolbar_top_offset.clone(),
            |value| Message::TextChanged(TextField::ToolbarTopOffset, value),
        )
        .entry_row(
            "Top offset Y (px)",
            |app| app.draft.ui_toolbar_top_offset_y.clone(),
            |value| Message::TextChanged(TextField::ToolbarTopOffsetY, value),
        )
        .entry_row(
            "Side offset Y (px)",
            |app| app.draft.ui_toolbar_side_offset.clone(),
            |value| Message::TextChanged(TextField::ToolbarSideOffset, value),
        )
        .entry_row(
            "Side offset X (px)",
            |app| app.draft.ui_toolbar_side_offset_x.clone(),
            |value| Message::TextChanged(TextField::ToolbarSideOffsetX, value),
        )
        .custom(&note(
            "Configured defaults. Dragging a toolbar in the overlay saves that position as a runtime preference; editing a value here takes over from the saved drag.",
        ));

    page.finish()
}

fn toolbar_override(app: &ConfiguratorApp, field: ToolbarOverrideField) -> OverrideOption {
    let overrides = app
        .draft
        .ui_toolbar_mode_overrides
        .for_mode(app.override_mode);
    match field {
        ToolbarOverrideField::ShowPresets => overrides.show_presets,
        ToolbarOverrideField::ShowActionsSection => overrides.show_actions_section,
        ToolbarOverrideField::ShowActionsAdvanced => overrides.show_actions_advanced,
        ToolbarOverrideField::ShowZoomActions => overrides.show_zoom_actions,
        ToolbarOverrideField::ShowPagesSection => overrides.show_pages_section,
        ToolbarOverrideField::ShowBoardsSection => overrides.show_boards_section,
        ToolbarOverrideField::ShowStepSection => overrides.show_step_section,
        ToolbarOverrideField::ShowTextControls => overrides.show_text_controls,
    }
}
