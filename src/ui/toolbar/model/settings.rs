use std::borrow::Cow;

use crate::config::{
    Action, ToolbarGroupId, ToolbarItemCategory, ToolbarItemDefinition, ToolbarItemId,
    ToolbarItemSurface, ToolbarLayoutMode, action_label, action_short_label,
    toolbar_item_definitions,
};

use super::super::{ToolbarEvent, ToolbarItemCustomizeGroup, ToolbarSideSection, ToolbarSnapshot};
use super::activation::{ToolbarActivation, ToolbarControlId};
use super::control::{ToolbarIcon, ToolbarTooltip};

#[derive(Debug, Clone)]
pub(crate) struct ToolbarSettingsModel {
    toggles: Vec<ToolbarSettingsToggle>,
    buttons: Vec<ToolbarSettingsButton>,
    groups: Vec<ToolbarSettingsCustomizeGroup>,
    item_overrides: Vec<ToolbarSettingsItemOverride>,
}

impl ToolbarSettingsModel {
    pub(crate) fn from_snapshot(snapshot: &ToolbarSnapshot) -> Option<Self> {
        let customize_shortcut = snapshot.drawer_tab == crate::input::ToolbarDrawerTab::Customize;
        let sections_tab = snapshot.drawer_tab == crate::input::ToolbarDrawerTab::Sections;
        let customizing = snapshot.customize_items_open || customize_shortcut;
        if !snapshot.drawer_open
            || (!customize_shortcut
                && !sections_tab
                && (snapshot.side_section_hidden(ToolbarSideSection::Settings)
                    || !snapshot.show_settings_section
                    || snapshot.drawer_tab != crate::input::ToolbarDrawerTab::App))
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

        if sections_tab {
            toggles.retain(|toggle| is_section_toggle_id(toggle.id));
        } else {
            toggles.retain(|toggle| !is_section_toggle_id(toggle.id));
        }
        toggles.retain(|toggle| control_visible(snapshot, toggle.id));
        if customizing {
            toggles.clear();
        }

        let buttons = if customizing {
            customize_buttons(snapshot)
        } else if sections_tab {
            section_buttons(snapshot)
        } else {
            settings_buttons(snapshot)
        };

        let groups = if customizing && snapshot.customize_items_group.is_none() {
            customize_groups()
        } else {
            Vec::new()
        };

        let item_overrides: Vec<_> = if let Some(group) = snapshot.customize_items_group {
            toolbar_item_definitions()
                .iter()
                .filter(|definition| customize_group_contains(group, definition))
                .map(|definition| ToolbarSettingsItemOverride::new(snapshot, definition))
                .collect()
        } else {
            Vec::new()
        };

        (!toggles.is_empty() || !buttons.is_empty() || !item_overrides.is_empty()).then_some(Self {
            toggles,
            buttons,
            groups,
            item_overrides,
        })
    }

    pub(crate) fn toggles(&self) -> &[ToolbarSettingsToggle] {
        &self.toggles
    }

    pub(crate) fn buttons(&self) -> &[ToolbarSettingsButton] {
        &self.buttons
    }

    pub(crate) fn groups(&self) -> &[ToolbarSettingsCustomizeGroup] {
        &self.groups
    }

    pub(crate) fn item_overrides(&self) -> &[ToolbarSettingsItemOverride] {
        &self.item_overrides
    }
}

fn settings_buttons(snapshot: &ToolbarSnapshot) -> Vec<ToolbarSettingsButton> {
    vec![
        ToolbarSettingsButton {
            id: ToolbarControlId::CustomizeToolbarItems,
            label: Cow::Borrowed("Customize toolbar"),
            event: ToolbarEvent::SetToolbarItemCustomizationOpen(true),
            icon: ToolbarIcon::Visibility,
            tooltip: ToolbarTooltip::text("Customize toolbar item visibility"),
        },
        ToolbarSettingsButton {
            id: ToolbarControlId::ResetToolbarHiddenItems,
            label: Cow::Borrowed("Reset hidden"),
            event: ToolbarEvent::ResetToolbarItemHiddenOverrides,
            icon: ToolbarIcon::Visibility,
            tooltip: ToolbarTooltip::text("Restore default hidden items"),
        },
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
    .filter(|button| reset_button_visible(snapshot, button.id))
    .filter(|button| control_visible(snapshot, button.id))
    .collect()
}

fn section_buttons(snapshot: &ToolbarSnapshot) -> Vec<ToolbarSettingsButton> {
    vec![ToolbarSettingsButton {
        id: ToolbarControlId::ResetToolbarHiddenItems,
        label: Cow::Borrowed("Reset hidden"),
        event: ToolbarEvent::ResetToolbarItemHiddenOverrides,
        icon: ToolbarIcon::Visibility,
        tooltip: ToolbarTooltip::text("Restore default hidden items"),
    }]
    .into_iter()
    .filter(|button| reset_button_visible(snapshot, button.id))
    .collect()
}

fn customize_buttons(snapshot: &ToolbarSnapshot) -> Vec<ToolbarSettingsButton> {
    let back_event = if snapshot.customize_items_group.is_some() {
        ToolbarEvent::SetToolbarItemCustomizationGroup(None)
    } else if snapshot.drawer_tab == crate::input::ToolbarDrawerTab::Customize {
        ToolbarEvent::SetDrawerTab(crate::input::ToolbarDrawerTab::App)
    } else {
        ToolbarEvent::SetToolbarItemCustomizationOpen(false)
    };
    vec![
        ToolbarSettingsButton {
            id: ToolbarControlId::BackToolbarSettings,
            label: Cow::Borrowed("Back"),
            event: back_event,
            icon: ToolbarIcon::Back,
            tooltip: ToolbarTooltip::text("Back to settings"),
        },
        ToolbarSettingsButton {
            id: ToolbarControlId::ResetToolbarHiddenItems,
            label: Cow::Borrowed("Reset hidden"),
            event: ToolbarEvent::ResetToolbarItemHiddenOverrides,
            icon: ToolbarIcon::Visibility,
            tooltip: ToolbarTooltip::text("Restore default hidden items"),
        },
    ]
    .into_iter()
    .filter(|button| reset_button_visible(snapshot, button.id))
    .collect()
}

fn reset_button_visible(snapshot: &ToolbarSnapshot, id: ToolbarControlId) -> bool {
    id != ToolbarControlId::ResetToolbarHiddenItems
        || !snapshot.resolved_toolbar_items.hidden.is_empty()
}

fn is_section_toggle_id(id: ToolbarControlId) -> bool {
    matches!(
        id,
        ToolbarControlId::SettingsPresets
            | ToolbarControlId::SettingsActions
            | ToolbarControlId::SettingsZoomActions
            | ToolbarControlId::SettingsAdvancedActions
            | ToolbarControlId::SettingsBoards
            | ToolbarControlId::SettingsPages
            | ToolbarControlId::SettingsStepControls
    )
}

fn overlay_item_override_allowed(definition: &ToolbarItemDefinition) -> bool {
    definition.group != Some(ToolbarGroupId::Settings)
}

fn customize_groups() -> Vec<ToolbarSettingsCustomizeGroup> {
    [
        ToolbarItemCustomizeGroup::TopTools,
        ToolbarItemCustomizeGroup::TopControls,
        ToolbarItemCustomizeGroup::SideSections,
        ToolbarItemCustomizeGroup::Actions,
        ToolbarItemCustomizeGroup::Pages,
        ToolbarItemCustomizeGroup::Boards,
        ToolbarItemCustomizeGroup::Presets,
        ToolbarItemCustomizeGroup::ToolOptions,
        ToolbarItemCustomizeGroup::Sessions,
    ]
    .into_iter()
    .map(ToolbarSettingsCustomizeGroup::new)
    .collect()
}

fn customize_group_contains(
    group: ToolbarItemCustomizeGroup,
    definition: &ToolbarItemDefinition,
) -> bool {
    if !overlay_item_override_allowed(definition) {
        return false;
    }

    match group {
        ToolbarItemCustomizeGroup::TopTools => {
            definition.surface == ToolbarItemSurface::Top
                && definition.category == ToolbarItemCategory::Tool
        }
        ToolbarItemCustomizeGroup::TopControls => {
            definition.surface == ToolbarItemSurface::Top
                && definition.category != ToolbarItemCategory::Tool
        }
        ToolbarItemCustomizeGroup::SideSections => {
            definition.category == ToolbarItemCategory::Group
        }
        ToolbarItemCustomizeGroup::Actions => definition.category == ToolbarItemCategory::Action,
        ToolbarItemCustomizeGroup::Pages => definition.category == ToolbarItemCategory::Page,
        ToolbarItemCustomizeGroup::Boards => definition.category == ToolbarItemCategory::Board,
        ToolbarItemCustomizeGroup::Presets => definition.group == Some(ToolbarGroupId::Presets),
        ToolbarItemCustomizeGroup::ToolOptions => {
            definition.category == ToolbarItemCategory::ToolOption
        }
        ToolbarItemCustomizeGroup::Sessions => definition.category == ToolbarItemCategory::Session,
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
pub(crate) struct ToolbarSettingsCustomizeGroup {
    pub(crate) label: Cow<'static, str>,
    pub(crate) event: ToolbarEvent,
    pub(crate) tooltip: ToolbarTooltip,
}

impl ToolbarSettingsCustomizeGroup {
    fn new(group: ToolbarItemCustomizeGroup) -> Self {
        Self {
            label: Cow::Borrowed(group.label()),
            event: ToolbarEvent::SetToolbarItemCustomizationGroup(Some(group)),
            tooltip: ToolbarTooltip::text(format!("Customize {}", group.label())),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ToolbarSettingsItemOverride {
    pub(crate) id: ToolbarItemId,
    pub(crate) label: Cow<'static, str>,
    pub(crate) shown: bool,
    pub(crate) activation: ToolbarActivation,
    pub(crate) tooltip: ToolbarTooltip,
}

impl ToolbarSettingsItemOverride {
    fn new(snapshot: &ToolbarSnapshot, definition: &ToolbarItemDefinition) -> Self {
        let id = definition.id;
        let hidden = snapshot.toolbar_item_hidden(id);
        Self {
            id,
            label: Cow::Borrowed(definition.label),
            shown: !hidden,
            activation: ToolbarActivation::Click(ToolbarEvent::SetToolbarItemHidden(id, !hidden)),
            tooltip: ToolbarTooltip::text(format!("{}: uncheck to hide", definition.label)),
        }
    }
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
