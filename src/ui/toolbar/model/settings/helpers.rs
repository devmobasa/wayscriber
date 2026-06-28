use super::*;

pub(super) fn settings_buttons(snapshot: &ToolbarSnapshot) -> Vec<ToolbarSettingsButton> {
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

pub(super) fn section_buttons(snapshot: &ToolbarSnapshot) -> Vec<ToolbarSettingsButton> {
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

pub(super) fn customize_buttons(snapshot: &ToolbarSnapshot) -> Vec<ToolbarSettingsButton> {
    let back_event = if snapshot.customize_items_group.is_some() {
        ToolbarEvent::SetToolbarItemCustomizationGroup(None)
    } else if snapshot.drawer_tab == crate::input::ToolbarDrawerTab::Customize {
        ToolbarEvent::SetDrawerTab(crate::input::ToolbarDrawerTab::App)
    } else {
        ToolbarEvent::SetToolbarItemCustomizationOpen(false)
    };
    let mut buttons = vec![
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
    ];
    if let Some(group) = snapshot
        .customize_items_group
        .and_then(customize_order_group)
        .filter(|group| order_is_customized(snapshot, *group))
    {
        buttons.push(ToolbarSettingsButton {
            id: ToolbarControlId::ResetToolbarItemOrder,
            label: Cow::Borrowed("Reset order"),
            event: ToolbarEvent::ResetToolbarItemOrder(group),
            icon: ToolbarIcon::Back,
            tooltip: ToolbarTooltip::text("Restore default order for this group"),
        });
    }
    buttons
        .into_iter()
        .filter(|button| reset_button_visible(snapshot, button.id))
        .collect()
}

fn reset_button_visible(snapshot: &ToolbarSnapshot, id: ToolbarControlId) -> bool {
    id != ToolbarControlId::ResetToolbarHiddenItems
        || !snapshot.resolved_toolbar_items.hidden.is_empty()
}

pub(super) fn is_section_toggle_id(id: ToolbarControlId) -> bool {
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

pub(super) fn customize_groups() -> Vec<ToolbarSettingsCustomizeGroup> {
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

pub(super) fn customize_group_contains(
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

pub(super) fn sort_customize_definitions(
    snapshot: &ToolbarSnapshot,
    group: ToolbarItemCustomizeGroup,
    definitions: &mut Vec<&ToolbarItemDefinition>,
) {
    let Some(order_group) = customize_order_group(group) else {
        return;
    };
    definitions.sort_by_key(|definition| {
        if overlay_order_group_for_definition(definition) == Some(order_group) {
            snapshot
                .resolved_toolbar_items
                .order
                .index_of(order_group, definition.id)
                .unwrap_or(usize::MAX)
        } else {
            usize::MAX
        }
    });
}

fn customize_order_group(group: ToolbarItemCustomizeGroup) -> Option<ToolbarItemOrderGroup> {
    match group {
        ToolbarItemCustomizeGroup::TopTools => Some(ToolbarItemOrderGroup::TopTools),
        ToolbarItemCustomizeGroup::TopControls => Some(ToolbarItemOrderGroup::TopControls),
        ToolbarItemCustomizeGroup::SideSections => Some(ToolbarItemOrderGroup::SideSections),
        _ => None,
    }
}

pub(super) fn definition_order_group_for_customize(
    group: ToolbarItemCustomizeGroup,
    definition: &ToolbarItemDefinition,
) -> Option<ToolbarItemOrderGroup> {
    let order_group = customize_order_group(group)?;
    (overlay_order_group_for_definition(definition) == Some(order_group)).then_some(order_group)
}

fn overlay_order_group_for_definition(
    definition: &ToolbarItemDefinition,
) -> Option<ToolbarItemOrderGroup> {
    toolbar_item_order_group(definition)
}

fn order_is_customized(snapshot: &ToolbarSnapshot, group: ToolbarItemOrderGroup) -> bool {
    let current = snapshot.resolved_toolbar_items.order.ordered_ids(group);
    let default_order = ToolbarItemOrderConfig::default().resolved();
    current != default_order.ordered_ids(group)
}

pub(super) fn control_visible(snapshot: &ToolbarSnapshot, id: ToolbarControlId) -> bool {
    control_item_id(id).is_none_or(|item| !snapshot.toolbar_item_hidden(item))
}

fn control_item_id(id: ToolbarControlId) -> Option<ToolbarItemId> {
    Some(match id {
        ToolbarControlId::SettingsContextAwareUi => ids::SIDE_SETTINGS_CONTEXT_AWARE_UI,
        ToolbarControlId::SettingsTextControls => ids::SIDE_SETTINGS_TEXT_CONTROLS,
        ToolbarControlId::SettingsStatusBar => ids::SIDE_SETTINGS_STATUS_BAR,
        ToolbarControlId::SettingsStatusBoardBadge => ids::SIDE_SETTINGS_STATUS_BOARD_BADGE,
        ToolbarControlId::SettingsStatusPageBadge => ids::SIDE_SETTINGS_STATUS_PAGE_BADGE,
        ToolbarControlId::SettingsFloatingBadgeAlways => ids::SIDE_SETTINGS_FLOATING_BADGE_ALWAYS,
        ToolbarControlId::SettingsPresetToasts => ids::SIDE_SETTINGS_PRESET_TOASTS,
        ToolbarControlId::SettingsPresets => ids::SIDE_SETTINGS_PRESETS,
        ToolbarControlId::SettingsActions => ids::SIDE_SETTINGS_ACTIONS,
        ToolbarControlId::SettingsZoomActions => ids::SIDE_SETTINGS_ZOOM_ACTIONS,
        ToolbarControlId::SettingsAdvancedActions => ids::SIDE_SETTINGS_ADVANCED_ACTIONS,
        ToolbarControlId::SettingsBoards => ids::SIDE_SETTINGS_BOARDS,
        ToolbarControlId::SettingsPages => ids::SIDE_SETTINGS_PAGES,
        ToolbarControlId::SettingsStepControls => ids::SIDE_SETTINGS_STEP_CONTROLS,
        ToolbarControlId::OpenConfigurator => ids::SIDE_SETTINGS_CONFIGURATOR,
        ToolbarControlId::OpenConfigFile => ids::SIDE_SETTINGS_CONFIG_FILE,
        _ => return None,
    })
}
