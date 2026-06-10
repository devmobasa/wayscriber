use std::borrow::Cow;

use crate::config::{
    Action, ToolbarItemId, ToolbarLayoutMode, action_label, action_short_label,
};

use super::super::{ToolbarEvent, ToolbarSideSection, ToolbarSnapshot};
use super::activation::{ToolbarActivation, ToolbarControlId};
use super::control::{ToolbarIcon, ToolbarTooltip};

#[derive(Debug, Clone)]
pub(crate) struct ToolbarSettingsModel {
    toggles: Vec<ToolbarSettingsToggle>,
    buttons: Vec<ToolbarSettingsButton>,
}

impl ToolbarSettingsModel {
    pub(crate) fn from_snapshot(snapshot: &ToolbarSnapshot) -> Option<Self> {
        if snapshot.side_section_hidden(ToolbarSideSection::Settings)
            || !snapshot.show_settings_section
            || !snapshot.drawer_open
            || snapshot.drawer_tab != crate::input::ToolbarDrawerTab::App
        {
            return None;
        }

        let mut toggles = vec![
            ToolbarSettingsToggle::new(
                ToolbarControlId::SettingsContextAwareUi,
                "Context UI",
                snapshot.context_aware_ui,
                ToolbarEvent::ToggleContextAwareUi(!snapshot.context_aware_ui),
                "Show/hide controls based on active tool.",
            ),
            ToolbarSettingsToggle::new(
                ToolbarControlId::SettingsTextControls,
                "Text controls",
                snapshot.show_text_controls,
                ToolbarEvent::ToggleTextControls(!snapshot.show_text_controls),
                "Text: font size/family.",
            ),
            ToolbarSettingsToggle::new(
                ToolbarControlId::SettingsStatusBar,
                "Status bar",
                snapshot.show_status_bar,
                ToolbarEvent::ToggleStatusBar(!snapshot.show_status_bar),
                "Status bar: color/tool readout.",
            ),
            ToolbarSettingsToggle::new(
                ToolbarControlId::SettingsStatusBoardBadge,
                "Status board",
                snapshot.show_status_board_badge,
                ToolbarEvent::ToggleStatusBoardBadge(!snapshot.show_status_board_badge),
                "Status bar: board label.",
            ),
            ToolbarSettingsToggle::new(
                ToolbarControlId::SettingsStatusPageBadge,
                "Status page",
                snapshot.show_status_page_badge,
                ToolbarEvent::ToggleStatusPageBadge(!snapshot.show_status_page_badge),
                "Status bar: page counter.",
            ),
            ToolbarSettingsToggle::new(
                ToolbarControlId::SettingsFloatingBadgeAlways,
                "Overlay badge",
                snapshot.show_floating_badge_always,
                ToolbarEvent::ToggleFloatingBadgeAlways(!snapshot.show_floating_badge_always),
                "Board/page badge when status bar is visible.",
            ),
            ToolbarSettingsToggle::new(
                ToolbarControlId::SettingsPresetToasts,
                "Preset toasts",
                snapshot.show_preset_toasts,
                ToolbarEvent::TogglePresetToasts(!snapshot.show_preset_toasts),
                "Preset toasts: apply/save/clear.",
            ),
        ];

        if snapshot.layout_mode != ToolbarLayoutMode::Simple {
            toggles.extend([
                ToolbarSettingsToggle::new(
                    ToolbarControlId::SettingsPresets,
                    "Show presets",
                    snapshot.show_presets,
                    ToolbarEvent::TogglePresets(!snapshot.show_presets),
                    "Presets: quick slots.",
                ),
                ToolbarSettingsToggle::new(
                    ToolbarControlId::SettingsActions,
                    "Show actions",
                    snapshot.show_actions_section,
                    ToolbarEvent::ToggleActionsSection(!snapshot.show_actions_section),
                    "Actions: undo/redo/clear.",
                ),
                ToolbarSettingsToggle::new(
                    ToolbarControlId::SettingsZoomActions,
                    "Zoom actions",
                    snapshot.show_zoom_actions,
                    ToolbarEvent::ToggleZoomActions(!snapshot.show_zoom_actions),
                    "Zoom: in/out/reset/lock.",
                ),
                ToolbarSettingsToggle::new(
                    ToolbarControlId::SettingsAdvancedActions,
                    "Adv. Actions",
                    snapshot.show_actions_advanced,
                    ToolbarEvent::ToggleActionsAdvanced(!snapshot.show_actions_advanced),
                    "Advanced: undo-all/delay/freeze.",
                ),
                ToolbarSettingsToggle::new(
                    ToolbarControlId::SettingsBoards,
                    "Boards",
                    snapshot.show_boards_section,
                    ToolbarEvent::ToggleBoardsSection(!snapshot.show_boards_section),
                    "Boards: prev/next/new/del.",
                ),
                ToolbarSettingsToggle::new(
                    ToolbarControlId::SettingsPages,
                    "Pages",
                    snapshot.show_pages_section,
                    ToolbarEvent::TogglePagesSection(!snapshot.show_pages_section),
                    "Pages: prev/next/new/dup/del.",
                ),
                ToolbarSettingsToggle::new(
                    ToolbarControlId::SettingsStepControls,
                    "Step controls",
                    snapshot.show_step_section,
                    ToolbarEvent::ToggleStepSection(!snapshot.show_step_section),
                    "Step: step undo/redo.",
                ),
            ]);
        }

        toggles.retain(|toggle| control_visible(snapshot, toggle.id));
        let buttons: Vec<_> = vec![
            ToolbarSettingsButton {
                id: ToolbarControlId::OpenConfigurator,
                label: Cow::Borrowed(action_short_label(Action::OpenConfigurator)),
                event: ToolbarEvent::OpenConfigurator,
                icon: ToolbarIcon::Settings,
                tooltip: ToolbarTooltip::Binding {
                    label: Cow::Borrowed(action_label(Action::OpenConfigurator)),
                    binding: snapshot
                        .binding_hints
                        .binding_for_action(Action::OpenConfigurator)
                        .map(str::to_string),
                },
            },
            ToolbarSettingsButton {
                id: ToolbarControlId::OpenConfigFile,
                label: Cow::Borrowed("Config file"),
                event: ToolbarEvent::OpenConfigFile,
                icon: ToolbarIcon::File,
                tooltip: ToolbarTooltip::text("Config file"),
            },
        ]
        .into_iter()
        .filter(|button| control_visible(snapshot, button.id))
        .collect();

        (!toggles.is_empty() || !buttons.is_empty()).then_some(Self { toggles, buttons })
    }

    pub(crate) fn toggles(&self) -> &[ToolbarSettingsToggle] {
        &self.toggles
    }

    pub(crate) fn buttons(&self) -> &[ToolbarSettingsButton] {
        &self.buttons
    }
}

fn control_visible(snapshot: &ToolbarSnapshot, id: ToolbarControlId) -> bool {
    control_item_id(id).is_none_or(|item| !snapshot.toolbar_item_hidden(item))
}

fn control_item_id(id: ToolbarControlId) -> Option<ToolbarItemId> {
    Some(ToolbarItemId::from_known(match id {
        ToolbarControlId::SettingsContextAwareUi => "side.settings.context-aware-ui",
        ToolbarControlId::SettingsTextControls => "side.settings.text-controls",
        ToolbarControlId::SettingsStatusBar => "side.settings.status-bar",
        ToolbarControlId::SettingsStatusBoardBadge => "side.settings.status-board-badge",
        ToolbarControlId::SettingsStatusPageBadge => "side.settings.status-page-badge",
        ToolbarControlId::SettingsFloatingBadgeAlways => "side.settings.floating-badge-always",
        ToolbarControlId::SettingsPresetToasts => "side.settings.preset-toasts",
        ToolbarControlId::SettingsPresets => "side.settings.presets",
        ToolbarControlId::SettingsActions => "side.settings.actions",
        ToolbarControlId::SettingsZoomActions => "side.settings.zoom-actions",
        ToolbarControlId::SettingsAdvancedActions => "side.settings.advanced-actions",
        ToolbarControlId::SettingsBoards => "side.settings.boards",
        ToolbarControlId::SettingsPages => "side.settings.pages",
        ToolbarControlId::SettingsStepControls => "side.settings.step-controls",
        ToolbarControlId::OpenConfigurator => "side.settings.configurator",
        ToolbarControlId::OpenConfigFile => "side.settings.config-file",
        _ => return None,
    }))
}

#[derive(Debug, Clone)]
pub(crate) struct ToolbarSettingsToggle {
    pub(crate) id: ToolbarControlId,
    pub(crate) label: Cow<'static, str>,
    pub(crate) checked: bool,
    pub(crate) activation: ToolbarActivation,
    pub(crate) tooltip: ToolbarTooltip,
}

impl ToolbarSettingsToggle {
    fn new(
        id: ToolbarControlId,
        label: &'static str,
        checked: bool,
        event: ToolbarEvent,
        tooltip: &'static str,
    ) -> Self {
        Self {
            id,
            label: Cow::Borrowed(label),
            checked,
            activation: ToolbarActivation::Click(event),
            tooltip: ToolbarTooltip::text(tooltip),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ToolbarSettingsButton {
    pub(crate) id: ToolbarControlId,
    pub(crate) label: Cow<'static, str>,
    pub(crate) event: ToolbarEvent,
    pub(crate) icon: ToolbarIcon,
    pub(crate) tooltip: ToolbarTooltip,
}
