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

        let mut toggles = vec![
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

        toggles.retain(|toggle| control_visible(snapshot, toggle.id));
        if customizing {
            toggles.clear();
        }

        let notices = if customizing {
            Vec::new()
        } else {
            runtime_persistence_notices(snapshot)
        };
        let buttons = if customizing {
            customize_buttons(snapshot)
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
    push_wrapped_notice(&mut notices, summary, severity);
    if let Some(detail) = &runtime.detail {
        push_wrapped_notice(&mut notices, detail, severity);
    }
    push_wrapped_notice(
        &mut notices,
        &format!("Runtime state: {}", runtime.path.display()),
        ToolbarSettingsNoticeSeverity::Info,
    );
    for path in &runtime.recovery_artifacts {
        push_wrapped_notice(
            &mut notices,
            &format!("Preserved recovery file: {}", path.display()),
            ToolbarSettingsNoticeSeverity::Warning,
        );
    }
    notices
}

fn push_wrapped_notice(
    notices: &mut Vec<ToolbarSettingsNotice>,
    text: &str,
    severity: ToolbarSettingsNoticeSeverity,
) {
    // The Cairo settings renderers ellipsize individual rows. Conservative
    // fixed-size character chunks keep all diagnostic and artifact path text
    // visible across rows instead of silently dropping the tail.
    const MAX_NOTICE_CHARS: usize = 20;
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        notices.push(ToolbarSettingsNotice {
            text: Cow::Borrowed(""),
            severity,
        });
        return;
    }
    for chunk in chars.chunks(MAX_NOTICE_CHARS) {
        notices.push(ToolbarSettingsNotice {
            text: Cow::Owned(chunk.iter().collect()),
            severity,
        });
    }
}
