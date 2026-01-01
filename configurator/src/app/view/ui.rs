use iced::theme;
use iced::widget::{Row, button, column, pick_list, row, scrollable, text};
use iced::{Element, Length};

use crate::messages::Message;
use crate::models::{
    QuadField, StatusPositionOption, TextField, ToggleField, ToolbarLayoutModeOption,
    ToolbarOverrideField, UiTabId,
};

use super::super::state::ConfiguratorApp;
use super::widgets::{
    color_quad_editor, labeled_control, labeled_input, labeled_input_with_feedback, override_row,
    toggle_row, validate_f64_range, validate_u64_range,
};

impl ConfiguratorApp {
    pub(super) fn ui_tab(&self) -> Element<'_, Message> {
        let tab_bar = UiTabId::ALL.iter().fold(
            Row::new().spacing(8).align_items(iced::Alignment::Center),
            |row, tab| {
                let label = tab.title();
                let button = button(label)
                    .padding([4, 10])
                    .style(if *tab == self.active_ui_tab {
                        theme::Button::Primary
                    } else {
                        theme::Button::Secondary
                    })
                    .on_press(Message::UiTabSelected(*tab));
                row.push(button)
            },
        );

        let content = match self.active_ui_tab {
            UiTabId::Toolbar => self.ui_toolbar_tab(),
            UiTabId::StatusBar => self.ui_status_bar_tab(),
            UiTabId::HelpOverlay => self.ui_help_overlay_tab(),
            UiTabId::ClickHighlight => self.ui_click_highlight_tab(),
        };

        let general = column![
            text("General UI").size(18),
            labeled_input(
                "Preferred output (GNOME fallback)",
                &self.draft.ui_preferred_output,
                &self.defaults.ui_preferred_output,
                TextField::UiPreferredOutput,
            ),
            text("Used for the GNOME xdg-shell fallback overlay.")
                .size(12)
                .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.6))),
            toggle_row(
                "Use fullscreen xdg fallback",
                self.draft.ui_xdg_fullscreen,
                self.defaults.ui_xdg_fullscreen,
                ToggleField::UiXdgFullscreen,
            ),
            toggle_row(
                "Enable context menu",
                self.draft.ui_context_menu_enabled,
                self.defaults.ui_context_menu_enabled,
                ToggleField::UiContextMenuEnabled,
            )
        ]
        .spacing(12);

        column![text("UI Settings").size(20), general, tab_bar, content]
            .spacing(12)
            .into()
    }

    pub(super) fn ui_toolbar_tab(&self) -> Element<'_, Message> {
        let toolbar_layout = pick_list(
            ToolbarLayoutModeOption::list(),
            Some(self.draft.ui_toolbar_layout_mode),
            Message::ToolbarLayoutModeChanged,
        );
        let override_mode_pick = pick_list(
            ToolbarLayoutModeOption::list(),
            Some(self.override_mode),
            Message::ToolbarOverrideModeChanged,
        );
        let overrides = self
            .draft
            .ui_toolbar_mode_overrides
            .for_mode(self.override_mode);

        let column = column![
            text("Toolbar").size(18),
            labeled_control(
                "Layout mode",
                toolbar_layout.width(Length::Fill).into(),
                self.defaults.ui_toolbar_layout_mode.label().to_string(),
                self.draft.ui_toolbar_layout_mode != self.defaults.ui_toolbar_layout_mode,
            ),
            toggle_row(
                "Pin top toolbar",
                self.draft.ui_toolbar_top_pinned,
                self.defaults.ui_toolbar_top_pinned,
                ToggleField::UiToolbarTopPinned,
            ),
            toggle_row(
                "Pin side toolbar",
                self.draft.ui_toolbar_side_pinned,
                self.defaults.ui_toolbar_side_pinned,
                ToggleField::UiToolbarSidePinned,
            ),
            toggle_row(
                "Use icon-only buttons",
                self.draft.ui_toolbar_use_icons,
                self.defaults.ui_toolbar_use_icons,
                ToggleField::UiToolbarUseIcons,
            ),
            toggle_row(
                "Show extended colors",
                self.draft.ui_toolbar_show_more_colors,
                self.defaults.ui_toolbar_show_more_colors,
                ToggleField::UiToolbarShowMoreColors,
            ),
            toggle_row(
                "Show presets",
                self.draft.ui_toolbar_show_presets,
                self.defaults.ui_toolbar_show_presets,
                ToggleField::UiToolbarShowPresets,
            ),
            toggle_row(
                "Show actions (basic)",
                self.draft.ui_toolbar_show_actions_section,
                self.defaults.ui_toolbar_show_actions_section,
                ToggleField::UiToolbarShowActionsSection,
            ),
            toggle_row(
                "Show advanced actions",
                self.draft.ui_toolbar_show_actions_advanced,
                self.defaults.ui_toolbar_show_actions_advanced,
                ToggleField::UiToolbarShowActionsAdvanced,
            ),
            toggle_row(
                "Show pages section",
                self.draft.ui_toolbar_show_pages_section,
                self.defaults.ui_toolbar_show_pages_section,
                ToggleField::UiToolbarShowPagesSection,
            ),
            toggle_row(
                "Show Step Undo/Redo",
                self.draft.ui_toolbar_show_step_section,
                self.defaults.ui_toolbar_show_step_section,
                ToggleField::UiToolbarShowStepSection,
            ),
            toggle_row(
                "Always show text controls",
                self.draft.ui_toolbar_show_text_controls,
                self.defaults.ui_toolbar_show_text_controls,
                ToggleField::UiToolbarShowTextControls,
            ),
            toggle_row(
                "Show settings section",
                self.draft.ui_toolbar_show_settings_section,
                self.defaults.ui_toolbar_show_settings_section,
                ToggleField::UiToolbarShowSettingsSection,
            ),
            toggle_row(
                "Show delay sliders",
                self.draft.ui_toolbar_show_delay_sliders,
                self.defaults.ui_toolbar_show_delay_sliders,
                ToggleField::UiToolbarShowDelaySliders,
            ),
            toggle_row(
                "Show marker opacity controls",
                self.draft.ui_toolbar_show_marker_opacity_section,
                self.defaults.ui_toolbar_show_marker_opacity_section,
                ToggleField::UiToolbarShowMarkerOpacitySection,
            ),
            toggle_row(
                "Show tool preview bubble",
                self.draft.ui_toolbar_show_tool_preview,
                self.defaults.ui_toolbar_show_tool_preview,
                ToggleField::UiToolbarShowToolPreview,
            ),
            toggle_row(
                "Show preset action toasts",
                self.draft.ui_toolbar_show_preset_toasts,
                self.defaults.ui_toolbar_show_preset_toasts,
                ToggleField::UiToolbarPresetToasts,
            ),
            toggle_row(
                "Force inline toolbars",
                self.draft.ui_toolbar_force_inline,
                self.defaults.ui_toolbar_force_inline,
                ToggleField::UiToolbarForceInline,
            ),
            text("Mode overrides").size(16),
            row![text("Edit mode:"), override_mode_pick]
                .spacing(12)
                .align_items(iced::Alignment::Center),
            text("Default keeps the mode preset.").size(12),
            override_row(ToolbarOverrideField::ShowPresets, overrides.show_presets),
            override_row(
                ToolbarOverrideField::ShowActionsSection,
                overrides.show_actions_section,
            ),
            override_row(
                ToolbarOverrideField::ShowActionsAdvanced,
                overrides.show_actions_advanced,
            ),
            override_row(
                ToolbarOverrideField::ShowPagesSection,
                overrides.show_pages_section,
            ),
            override_row(
                ToolbarOverrideField::ShowStepSection,
                overrides.show_step_section
            ),
            override_row(
                ToolbarOverrideField::ShowTextControls,
                overrides.show_text_controls
            ),
            override_row(
                ToolbarOverrideField::ShowSettingsSection,
                overrides.show_settings_section,
            ),
            text("Placement offsets").size(16),
            row![
                labeled_input(
                    "Top offset X (px)",
                    &self.draft.ui_toolbar_top_offset,
                    &self.defaults.ui_toolbar_top_offset,
                    TextField::ToolbarTopOffset,
                ),
                labeled_input(
                    "Top offset Y (px)",
                    &self.draft.ui_toolbar_top_offset_y,
                    &self.defaults.ui_toolbar_top_offset_y,
                    TextField::ToolbarTopOffsetY,
                )
            ]
            .spacing(12),
            row![
                labeled_input(
                    "Side offset Y (px)",
                    &self.draft.ui_toolbar_side_offset,
                    &self.defaults.ui_toolbar_side_offset,
                    TextField::ToolbarSideOffset,
                ),
                labeled_input(
                    "Side offset X (px)",
                    &self.draft.ui_toolbar_side_offset_x,
                    &self.defaults.ui_toolbar_side_offset_x,
                    TextField::ToolbarSideOffsetX,
                )
            ]
            .spacing(12),
        ]
        .spacing(12);

        scrollable(column).into()
    }

    pub(super) fn ui_status_bar_tab(&self) -> Element<'_, Message> {
        let status_position = pick_list(
            StatusPositionOption::list(),
            Some(self.draft.ui_status_position),
            Message::StatusPositionChanged,
        );

        let column = column![
            text("Status Bar").size(18),
            toggle_row(
                "Show status bar",
                self.draft.ui_show_status_bar,
                self.defaults.ui_show_status_bar,
                ToggleField::UiShowStatusBar,
            ),
            toggle_row(
                "Show frozen badge",
                self.draft.ui_show_frozen_badge,
                self.defaults.ui_show_frozen_badge,
                ToggleField::UiShowFrozenBadge,
            ),
            labeled_control(
                "Status bar position",
                status_position.width(Length::Fill).into(),
                self.defaults.ui_status_position.label().to_string(),
                self.draft.ui_status_position != self.defaults.ui_status_position,
            ),
            text("Status Bar Style").size(18),
            color_quad_editor(
                "Background RGBA (0-1)",
                &self.draft.status_bar_bg_color,
                &self.defaults.status_bar_bg_color,
                QuadField::StatusBarBg,
            ),
            color_quad_editor(
                "Text RGBA (0-1)",
                &self.draft.status_bar_text_color,
                &self.defaults.status_bar_text_color,
                QuadField::StatusBarText,
            ),
            row![
                labeled_input(
                    "Font size",
                    &self.draft.status_font_size,
                    &self.defaults.status_font_size,
                    TextField::StatusFontSize,
                ),
                labeled_input(
                    "Padding",
                    &self.draft.status_padding,
                    &self.defaults.status_padding,
                    TextField::StatusPadding,
                ),
                labeled_input(
                    "Dot radius",
                    &self.draft.status_dot_radius,
                    &self.defaults.status_dot_radius,
                    TextField::StatusDotRadius,
                )
            ]
            .spacing(12),
        ]
        .spacing(12);

        scrollable(column).into()
    }

    pub(super) fn ui_help_overlay_tab(&self) -> Element<'_, Message> {
        let column = column![
            text("Help Overlay Style").size(18),
            toggle_row(
                "Filter sections by enabled features",
                self.draft.help_context_filter,
                self.defaults.help_context_filter,
                ToggleField::UiHelpOverlayContextFilter,
            ),
            color_quad_editor(
                "Background RGBA (0-1)",
                &self.draft.help_bg_color,
                &self.defaults.help_bg_color,
                QuadField::HelpBg,
            ),
            color_quad_editor(
                "Border RGBA (0-1)",
                &self.draft.help_border_color,
                &self.defaults.help_border_color,
                QuadField::HelpBorder,
            ),
            color_quad_editor(
                "Text RGBA (0-1)",
                &self.draft.help_text_color,
                &self.defaults.help_text_color,
                QuadField::HelpText,
            ),
            labeled_input(
                "Font family",
                &self.draft.help_font_family,
                &self.defaults.help_font_family,
                TextField::HelpFontFamily,
            ),
            row![
                labeled_input(
                    "Font size",
                    &self.draft.help_font_size,
                    &self.defaults.help_font_size,
                    TextField::HelpFontSize,
                ),
                labeled_input(
                    "Line height",
                    &self.draft.help_line_height,
                    &self.defaults.help_line_height,
                    TextField::HelpLineHeight,
                ),
                labeled_input(
                    "Padding",
                    &self.draft.help_padding,
                    &self.defaults.help_padding,
                    TextField::HelpPadding,
                ),
                labeled_input(
                    "Border width",
                    &self.draft.help_border_width,
                    &self.defaults.help_border_width,
                    TextField::HelpBorderWidth,
                )
            ]
            .spacing(12),
        ]
        .spacing(12);

        scrollable(column).into()
    }

    pub(super) fn ui_click_highlight_tab(&self) -> Element<'_, Message> {
        let column = column![
            text("Click Highlight").size(18),
            toggle_row(
                "Enable click highlight",
                self.draft.click_highlight_enabled,
                self.defaults.click_highlight_enabled,
                ToggleField::UiClickHighlightEnabled,
            ),
            toggle_row(
                "Link highlight color to current pen",
                self.draft.click_highlight_use_pen_color,
                self.defaults.click_highlight_use_pen_color,
                ToggleField::UiClickHighlightUsePenColor,
            ),
            row![
                labeled_input_with_feedback(
                    "Radius",
                    &self.draft.click_highlight_radius,
                    &self.defaults.click_highlight_radius,
                    TextField::HighlightRadius,
                    Some("Range: 16-160"),
                    validate_f64_range(&self.draft.click_highlight_radius, 16.0, 160.0),
                ),
                labeled_input_with_feedback(
                    "Outline thickness",
                    &self.draft.click_highlight_outline_thickness,
                    &self.defaults.click_highlight_outline_thickness,
                    TextField::HighlightOutlineThickness,
                    Some("Range: 1-12"),
                    validate_f64_range(&self.draft.click_highlight_outline_thickness, 1.0, 12.0),
                ),
                labeled_input_with_feedback(
                    "Duration (ms)",
                    &self.draft.click_highlight_duration_ms,
                    &self.defaults.click_highlight_duration_ms,
                    TextField::HighlightDurationMs,
                    Some("Range: 150-1500 ms"),
                    validate_u64_range(&self.draft.click_highlight_duration_ms, 150, 1500),
                )
            ]
            .spacing(12),
            color_quad_editor(
                "Fill RGBA (0-1)",
                &self.draft.click_highlight_fill_color,
                &self.defaults.click_highlight_fill_color,
                QuadField::HighlightFill,
            ),
            color_quad_editor(
                "Outline RGBA (0-1)",
                &self.draft.click_highlight_outline_color,
                &self.defaults.click_highlight_outline_color,
                QuadField::HighlightOutline,
            )
        ]
        .spacing(12);

        scrollable(column).into()
    }
}
