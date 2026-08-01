use std::borrow::Cow;

use crate::config::{
    Action, ToolbarGroupId, ToolbarItemCategory, ToolbarItemDefinition, ToolbarItemId,
    ToolbarItemOrderConfig, ToolbarItemOrderGroup, ToolbarItemSurface, ToolbarLayoutMode,
    action_label, action_short_label, factory_individual_toolbar_item_visibility_settings,
    item_visibility_setting, toolbar_item_definitions, toolbar_item_ids as ids,
    toolbar_item_order_group, toolbar_item_visibility_override_allowed,
};

use super::super::{ToolbarEvent, ToolbarItemCustomizeGroup, ToolbarSnapshot};
use super::activation::{ToolbarActivation, ToolbarControlId};
use super::control::{ToolbarIcon, ToolbarTooltip};

mod helpers;
use helpers::{
    control_visible, customize_buttons, customize_group_contains, customize_groups,
    definition_order_group_for_customize, settings_buttons, sort_customize_definitions,
    status_bar_content_buttons,
};

#[derive(Debug, Clone)]
pub(crate) struct ToolbarSettingsModel {
    toggles: Vec<ToolbarSettingsToggle>,
    notices: Vec<ToolbarSettingsNotice>,
    buttons: Vec<ToolbarSettingsButton>,
    groups: Vec<ToolbarSettingsCustomizeGroup>,
    item_overrides: Vec<ToolbarSettingsItemOverride>,
}

impl ToolbarSettingsModel {
    pub(crate) fn from_snapshot(snapshot: &ToolbarSnapshot) -> Option<Self> {
        // The Settings pane is navigation, not a hideable section: it is the
        // single customization surface, so it must always be reachable.
        if snapshot.active_side_pane != crate::ui::toolbar::SidePane::Settings {
            return None;
        }
        Self::build(snapshot)
    }

    /// The same model for the top strip's Settings popover, which ignores
    /// the side palette's pane selection (under `side_layout = "pill"` the
    /// popover is the only Settings surface).
    pub(crate) fn for_popover(snapshot: &ToolbarSnapshot) -> Option<Self> {
        Self::build(snapshot)
    }

    fn build(snapshot: &ToolbarSnapshot) -> Option<Self> {
        let customizing = snapshot.customize_items_open;
        let status_bar_contents = snapshot.status_bar_contents_open;

        let mut toggles = if status_bar_contents {
            status_bar_content_toggles(snapshot)
        } else {
            vec![
                ToolbarSettingsToggle::new(
                    ToolbarControlId::SettingsContextAwareUi,
                    "Adapt to tool",
                    snapshot.context_aware_ui,
                    ToolbarEvent::ToggleContextAwareUi(!snapshot.context_aware_ui),
                    "Show only the active tool's controls.",
                ),
                ToolbarSettingsToggle::new(
                    ToolbarControlId::SettingsIconMode,
                    "Icon buttons",
                    snapshot.use_icons,
                    ToolbarEvent::ToggleIconMode(!snapshot.use_icons),
                    "Icons instead of text labels.",
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
                    "Show the status bar and its configured contents.",
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
                ToolbarSettingsToggle::new(
                    ToolbarControlId::SettingsInputHud,
                    "Input HUD",
                    snapshot.input_hud_enabled,
                    ToolbarEvent::ToggleInputHud(!snapshot.input_hud_enabled),
                    "Show keystrokes and clicks on screen.",
                ),
            ]
        };

        if !status_bar_contents && snapshot.layout_mode != ToolbarLayoutMode::Simple {
            toggles.extend([
                ToolbarSettingsToggle::new(
                    ToolbarControlId::SettingsPresets,
                    "Presets",
                    snapshot.show_presets,
                    ToolbarEvent::TogglePresets(!snapshot.show_presets),
                    "Presets: quick slots.",
                ),
                ToolbarSettingsToggle::new(
                    ToolbarControlId::SettingsActions,
                    "Actions",
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
                    "Advanced actions",
                    snapshot.show_actions_advanced,
                    ToolbarEvent::ToggleActionsAdvanced(!snapshot.show_actions_advanced),
                    "Undo all, delayed undo, freeze.",
                )
                .wide(),
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
                    "Multi-step undo/redo",
                    snapshot.show_step_section,
                    ToolbarEvent::ToggleStepSection(!snapshot.show_step_section),
                    "Undo/redo several strokes at once.",
                )
                .wide(),
            ]);
        }

        // Content preferences are the recovery/customization surface for the
        // status bar itself, so toolbar-item visibility overrides must not
        // hide entries from this nested panel (the legacy Board/Page control
        // IDs are also used by the main Settings grid).
        if !status_bar_contents {
            toggles.retain(|toggle| control_visible(snapshot, toggle.id));
        }
        if customizing {
            toggles.clear();
        }

        let notices = if customizing || status_bar_contents {
            Vec::new()
        } else {
            runtime_persistence_notices(snapshot)
        };
        let buttons = if customizing {
            customize_buttons(snapshot)
        } else if status_bar_contents {
            status_bar_content_buttons()
        } else {
            settings_buttons(snapshot)
        };

        let groups = if customizing && snapshot.customize_items_group.is_none() {
            customize_groups()
        } else {
            Vec::new()
        };

        let item_overrides: Vec<_> = if let Some(group) = snapshot.customize_items_group {
            let mut definitions: Vec<_> = toolbar_item_definitions()
                .iter()
                .filter(|definition| customize_group_contains(group, definition))
                .collect();
            sort_customize_definitions(snapshot, group, &mut definitions);
            definitions
                .into_iter()
                .map(|definition| ToolbarSettingsItemOverride::new(snapshot, group, definition))
                .collect()
        } else {
            Vec::new()
        };

        (!toggles.is_empty()
            || !notices.is_empty()
            || !buttons.is_empty()
            || !item_overrides.is_empty())
        .then_some(Self {
            toggles,
            notices,
            buttons,
            groups,
            item_overrides,
        })
    }

    /// Read by the GTK settings pane when it refreshes a toggle in place;
    /// the built-in frontend rebuilds from the spec instead.
    #[cfg_attr(not(feature = "toolbar-gtk"), allow(dead_code))]
    pub(crate) fn toggles(&self) -> &[ToolbarSettingsToggle] {
        &self.toggles
    }

    /// Toggle rows for the two-column grid: wide toggles take a full row,
    /// the rest pair up in order. The section height math and the renderer
    /// both consume this packing so they can never disagree.
    pub(crate) fn toggle_rows(&self) -> Vec<Vec<&ToolbarSettingsToggle>> {
        let mut rows: Vec<Vec<&ToolbarSettingsToggle>> = Vec::new();
        let mut pending: Option<&ToolbarSettingsToggle> = None;
        for toggle in &self.toggles {
            if toggle.wide {
                if let Some(narrow) = pending.take() {
                    rows.push(vec![narrow]);
                }
                rows.push(vec![toggle]);
            } else if let Some(narrow) = pending.take() {
                rows.push(vec![narrow, toggle]);
            } else {
                pending = Some(toggle);
            }
        }
        if let Some(narrow) = pending.take() {
            rows.push(vec![narrow]);
        }
        rows
    }

    pub(crate) fn buttons(&self) -> &[ToolbarSettingsButton] {
        &self.buttons
    }

    pub(crate) fn notices(&self) -> &[ToolbarSettingsNotice] {
        &self.notices
    }

    pub(crate) fn groups(&self) -> &[ToolbarSettingsCustomizeGroup] {
        &self.groups
    }

    pub(crate) fn item_overrides(&self) -> &[ToolbarSettingsItemOverride] {
        &self.item_overrides
    }
}

fn status_bar_content_toggles(snapshot: &ToolbarSnapshot) -> Vec<ToolbarSettingsToggle> {
    let mut toggles = vec![
        ToolbarSettingsToggle::new(
            ToolbarControlId::SettingsStatusBarInteractive,
            "Clickable segments",
            snapshot.status_bar_interactive,
            ToolbarEvent::SetStatusBarInteractive(!snapshot.status_bar_interactive),
            "Allow status-bar segments to open their related controls.",
        )
        .wide(),
    ];
    toggles.extend(crate::config::StatusBarItem::ALL.map(|item| {
        let (id, label, tooltip) = match item {
            crate::config::StatusBarItem::ActiveOutput => (
                ToolbarControlId::SettingsStatusActiveOutput,
                "Active output",
                "Status bar: active monitor/output.",
            ),
            crate::config::StatusBarItem::SelectionInfo => (
                ToolbarControlId::SettingsStatusSelectionInfo,
                "Selection size",
                "Status bar: selected item dimensions.",
            ),
            crate::config::StatusBarItem::Board => (
                ToolbarControlId::SettingsStatusBoardBadge,
                "Board",
                "Status bar: board label.",
            ),
            crate::config::StatusBarItem::Page => (
                ToolbarControlId::SettingsStatusPageBadge,
                "Page",
                "Status bar: page counter.",
            ),
            crate::config::StatusBarItem::Color => (
                ToolbarControlId::SettingsStatusColor,
                "Current color",
                "Status bar: active color dot.",
            ),
            crate::config::StatusBarItem::Tool => (
                ToolbarControlId::SettingsStatusTool,
                "Active tool",
                "Status bar: active tool name.",
            ),
            crate::config::StatusBarItem::Size => (
                ToolbarControlId::SettingsStatusSize,
                "Tool size",
                "Status bar: active tool size.",
            ),
            crate::config::StatusBarItem::ContextIndicators => (
                ToolbarControlId::SettingsStatusContextIndicators,
                "Context indicators",
                "Status bar: text and highlight state.",
            ),
            crate::config::StatusBarItem::ToolbarHint => (
                ToolbarControlId::SettingsStatusToolbarHint,
                "Toolbar hint",
                "Status bar: recovery hint while toolbars are hidden.",
            ),
            crate::config::StatusBarItem::Help => (
                ToolbarControlId::SettingsStatusHelp,
                "Help shortcut",
                "Status bar: Help shortcut.",
            ),
            crate::config::StatusBarItem::About => (
                ToolbarControlId::SettingsStatusAbout,
                "About and version",
                "Status bar: About/version chip.",
            ),
        };
        ToolbarSettingsToggle::new(
            id,
            label,
            snapshot.status_bar_item_visible(item),
            ToolbarEvent::SetStatusBarItemVisible(item, !snapshot.status_bar_item_visible(item)),
            tooltip,
        )
    }));
    toggles
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
    pub(crate) order: Option<ToolbarSettingsItemOrder>,
}

impl ToolbarSettingsItemOverride {
    fn new(
        snapshot: &ToolbarSnapshot,
        group: ToolbarItemCustomizeGroup,
        definition: &ToolbarItemDefinition,
    ) -> Self {
        let id = definition.id;
        let hidden = snapshot.toolbar_item_hidden(id);
        let order =
            definition_order_group_for_customize(group, definition).and_then(|order_group| {
                let index = snapshot
                    .resolved_toolbar_items
                    .order
                    .index_of(order_group, id)?;
                let len = snapshot
                    .resolved_toolbar_items
                    .order
                    .ordered_ids(order_group)
                    .len();
                Some(ToolbarSettingsItemOrder {
                    group: order_group,
                    index,
                    can_move_up: index > 0,
                    can_move_down: index + 1 < len,
                    move_up: ToolbarActivation::Click(ToolbarEvent::MoveToolbarItem {
                        group: order_group,
                        id,
                        delta: -1,
                    }),
                    move_down: ToolbarActivation::Click(ToolbarEvent::MoveToolbarItem {
                        group: order_group,
                        id,
                        delta: 1,
                    }),
                })
            });
        Self {
            id,
            label: Cow::Borrowed(definition.label),
            shown: !hidden,
            activation: ToolbarActivation::Click(ToolbarEvent::SetToolbarItemHidden(id, !hidden)),
            tooltip: ToolbarTooltip::text(format!("{}: uncheck to hide", definition.label)),
            order,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ToolbarSettingsItemOrder {
    pub(crate) group: ToolbarItemOrderGroup,
    pub(crate) index: usize,
    pub(crate) can_move_up: bool,
    pub(crate) can_move_down: bool,
    pub(crate) move_up: ToolbarActivation,
    pub(crate) move_down: ToolbarActivation,
}

#[derive(Debug, Clone)]
pub(crate) struct ToolbarSettingsToggle {
    pub(crate) id: ToolbarControlId,
    pub(crate) label: Cow<'static, str>,
    pub(crate) checked: bool,
    pub(crate) activation: ToolbarActivation,
    pub(crate) tooltip: ToolbarTooltip,
    /// Label too long for a half-width cell: the toggle takes a full row.
    pub(crate) wide: bool,
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
            wide: false,
        }
    }

    fn wide(mut self) -> Self {
        self.wide = true;
        self
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ToolbarSettingsNotice {
    pub(crate) text: Cow<'static, str>,
    pub(crate) severity: ToolbarSettingsNoticeSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarSettingsNoticeSeverity {
    Info,
    Warning,
    Error,
}

fn runtime_persistence_notices(snapshot: &ToolbarSnapshot) -> Vec<ToolbarSettingsNotice> {
    use crate::ui::toolbar::RuntimeUiPersistenceMode as Mode;

    let Some(runtime) = &snapshot.runtime_ui_persistence else {
        return Vec::new();
    };
    let (summary, severity) = match &runtime.mode {
        Mode::Unavailable => (
            "Runtime preference persistence is unavailable",
            ToolbarSettingsNoticeSeverity::Error,
        ),
        Mode::Missing => (
            "Runtime preferences use configured defaults",
            ToolbarSettingsNoticeSeverity::Info,
        ),
        Mode::Supported => (
            "Runtime preferences are saved separately",
            ToolbarSettingsNoticeSeverity::Info,
        ),
        Mode::UnsupportedReadOnly { .. } => (
            "Runtime preferences are read-only (newer format)",
            ToolbarSettingsNoticeSeverity::Warning,
        ),
        Mode::Resetting => (
            "Resetting runtime preferences…",
            ToolbarSettingsNoticeSeverity::Info,
        ),
        Mode::AwaitingUnsupportedResetConfirmation { .. } => (
            "Confirm reset of newer runtime-state data",
            ToolbarSettingsNoticeSeverity::Warning,
        ),
        Mode::Unhealthy => (
            "Runtime preference persistence is blocked",
            ToolbarSettingsNoticeSeverity::Error,
        ),
        Mode::Recovering => (
            "Recovering runtime preference persistence…",
            ToolbarSettingsNoticeSeverity::Warning,
        ),
        Mode::CancellingRecovery => (
            "Waiting for the active recovery write…",
            ToolbarSettingsNoticeSeverity::Warning,
        ),
        Mode::AwaitingInvalidResetConfirmation => (
            "Confirm preservation and reset of invalid data",
            ToolbarSettingsNoticeSeverity::Warning,
        ),
    };
    let mut notices = Vec::new();
    push_notice(&mut notices, summary, severity);
    if let Some(detail) = &runtime.detail {
        push_notice(&mut notices, detail, severity);
    }
    push_notice(
        &mut notices,
        &format!("Runtime state: {}", runtime.path.display()),
        ToolbarSettingsNoticeSeverity::Info,
    );
    for path in &runtime.recovery_artifacts {
        push_notice(
            &mut notices,
            &format!("Preserved recovery file: {}", path.display()),
            ToolbarSettingsNoticeSeverity::Warning,
        );
    }
    notices
}

fn push_notice(
    notices: &mut Vec<ToolbarSettingsNotice>,
    text: &str,
    severity: ToolbarSettingsNoticeSeverity,
) {
    notices.push(ToolbarSettingsNotice {
        text: Cow::Owned(text.to_string()),
        severity,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StatusBarItem;
    use crate::input::state::test_support::make_test_input_state;
    use crate::ui::toolbar::ToolbarBindingHints;

    fn snapshot() -> ToolbarSnapshot {
        let state = make_test_input_state();
        ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default())
    }

    /// The Settings popover is the app-level lane (palette, configurator,
    /// config file), so About belongs there and must carry the same
    /// action-derived label and event as every other entry.
    #[test]
    fn settings_buttons_offer_about_after_the_configurator_entries() {
        let model = ToolbarSettingsModel::build(&snapshot()).expect("settings model");
        let ids: Vec<ToolbarControlId> = model.buttons().iter().map(|button| button.id).collect();

        let configurator = ids
            .iter()
            .position(|id| *id == ToolbarControlId::OpenConfigurator)
            .expect("configurator entry");
        let about = ids
            .iter()
            .position(|id| *id == ToolbarControlId::OpenAbout)
            .expect("about entry");
        assert!(
            about > configurator,
            "About follows the configurator entries: {ids:?}"
        );

        let button = &model.buttons()[about];
        assert_eq!(button.event, ToolbarEvent::OpenAbout);
        assert_eq!(button.label, action_short_label(Action::OpenAbout));
        assert_eq!(button.icon, ToolbarIcon::Info);
    }

    #[test]
    fn status_bar_contents_subpanel_exposes_every_authored_content_toggle() {
        let mut state = make_test_input_state();
        assert!(state.set_toolbar_item_hidden(
            crate::config::toolbar_item_ids::SIDE_SETTINGS_STATUS_BOARD_BADGE,
            true,
        ));
        assert!(state.set_toolbar_item_hidden(
            crate::config::toolbar_item_ids::SIDE_SETTINGS_STATUS_PAGE_BADGE,
            true,
        ));
        state.toolbar_status_bar_contents_open = true;
        let snapshot =
            ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default());
        let model = ToolbarSettingsModel::build(&snapshot).expect("settings model");

        assert_eq!(model.toggles().len(), StatusBarItem::ALL.len() + 1);
        assert_eq!(
            model.toggles()[0].id,
            ToolbarControlId::SettingsStatusBarInteractive
        );
        let items: Vec<_> = model
            .toggles()
            .iter()
            .skip(1)
            .filter_map(|toggle| match &toggle.activation {
                ToolbarActivation::Click(ToolbarEvent::SetStatusBarItemVisible(item, _)) => {
                    Some(*item)
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            items.len(),
            StatusBarItem::ALL.len(),
            "every status content row uses an item-visibility activation"
        );
        assert_eq!(items, StatusBarItem::ALL);
        assert!(model.notices().is_empty());
        assert_eq!(model.buttons().len(), 1);
        assert_eq!(
            model.buttons()[0].id,
            ToolbarControlId::BackStatusBarContents
        );
    }
}
