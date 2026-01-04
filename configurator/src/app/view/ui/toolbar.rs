use iced::widget::{column, pick_list, row, scrollable, text};
use iced::{Element, Length};

use crate::app::state::ConfiguratorApp;
use crate::messages::Message;
use crate::models::{TextField, ToggleField, ToolbarLayoutModeOption, ToolbarOverrideField};

use super::super::widgets::{labeled_control, labeled_input, override_row, toggle_row};

impl ConfiguratorApp {
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
                "Show zoom actions",
                self.draft.ui_toolbar_show_zoom_actions,
                self.defaults.ui_toolbar_show_zoom_actions,
                ToggleField::UiToolbarShowZoomActions,
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
            override_row(ToolbarOverrideField::ShowZoomActions, overrides.show_zoom_actions),
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
}
