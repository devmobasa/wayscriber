//! Authored preferences an overlay control changes for the current run.
//!
//! `config.toml` is an authored input, changed only by an explicit user edit
//! action: the configurator's Save, or one of the overlay's three scoped edits
//! — a shortcut rebind, a preset slot, a quick-color swatch. Flipping a
//! preference is not one of them, so these toggles never reach disk. They
//! update the effective config the running overlay reads — the toolbar seed
//! derivation is the live consumer — and the next start reads the configured
//! value back.

use super::*;
use crate::config::{Config, StatusBarItem};

/// One authored preference reachable from an overlay control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ToolbarPreference {
    Toolbar(ToolbarPreferenceField),
    Ui(UiPreferenceField),
    HistoryCustomSection,
    ClickHighlight,
    InputHud,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ToolbarPreferenceField {
    Icons,
    MoreColors,
    ContextAwareUi,
    PresetToasts,
    ToolPreview,
    DelaySliders,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UiPreferenceField {
    StatusBar,
    StatusBarInteractive,
    StatusBarItem(StatusBarItem),
    StatusBoardBadge,
    StatusPageBadge,
    FloatingBadgeAlways,
}

/// The authored preference a toolbar event changes, if any.
///
/// Events absent from this match own no authored field; the exhaustive
/// classification of what an event persists lives in `persistence_for_event`,
/// where every one of these is `Ephemeral`.
pub(super) fn preference_for_event(event: &ToolbarEvent) -> Option<ToolbarPreference> {
    use ToolbarPreference::{ClickHighlight, HistoryCustomSection, InputHud, Toolbar, Ui};
    use ToolbarPreferenceField::*;
    use UiPreferenceField as UiField;

    let preference = match event {
        ToolbarEvent::ToggleIconMode(_) => Toolbar(Icons),
        ToolbarEvent::ToggleMoreColors(_) => Toolbar(MoreColors),
        ToolbarEvent::ToggleContextAwareUi(_) => Toolbar(ContextAwareUi),
        ToolbarEvent::TogglePresetToasts(_) => Toolbar(PresetToasts),
        ToolbarEvent::ToggleToolPreview(_) => Toolbar(ToolPreview),
        ToolbarEvent::ToggleDelaySliders(_) => Toolbar(DelaySliders),
        ToolbarEvent::ToggleCustomSection(_) => HistoryCustomSection,
        ToolbarEvent::ToggleStatusBar(_) => Ui(UiField::StatusBar),
        ToolbarEvent::SetStatusBarInteractive(_) => Ui(UiField::StatusBarInteractive),
        ToolbarEvent::SetStatusBarItemVisible(item, _) => Ui(UiField::StatusBarItem(*item)),
        ToolbarEvent::ToggleStatusBoardBadge(_) => Ui(UiField::StatusBoardBadge),
        ToolbarEvent::ToggleStatusPageBadge(_) => Ui(UiField::StatusPageBadge),
        ToolbarEvent::ToggleFloatingBadgeAlways(_) => Ui(UiField::FloatingBadgeAlways),
        ToolbarEvent::SelectTool(crate::input::Tool::Highlight)
        | ToolbarEvent::ToggleAllHighlight(_)
        | ToolbarEvent::ToggleHighlightToolRing(_) => ClickHighlight,
        ToolbarEvent::ToggleInputHud(_) => InputHud,
        _ => return None,
    };
    Some(preference)
}

/// The tool-preview value the effective config keeps while presenter mode
/// holds the runtime one: the user's, not the mode's.
pub(super) fn effective_tool_preview_value(current: bool, presenter_restore: Option<bool>) -> bool {
    presenter_restore.unwrap_or(current)
}

/// Copy one authored preference from the runtime state into the effective
/// config. Returns whether the config value actually moved.
pub(super) fn apply_toolbar_preference(
    config: &mut Config,
    input_state: &InputState,
    preference: ToolbarPreference,
) -> bool {
    match preference {
        ToolbarPreference::Toolbar(field) => apply_toolbar_field(config, input_state, field),
        ToolbarPreference::Ui(field) => apply_ui_field(config, input_state, field),
        ToolbarPreference::HistoryCustomSection => store(
            &mut config.history.custom_section_enabled,
            input_state.custom_section_enabled,
        ),
        ToolbarPreference::ClickHighlight => apply_click_highlight(config, input_state),
        ToolbarPreference::InputHud => apply_input_hud(config, input_state),
    }
}

fn apply_toolbar_field(
    config: &mut Config,
    input_state: &InputState,
    field: ToolbarPreferenceField,
) -> bool {
    use ToolbarPreferenceField::*;

    match field {
        Icons => store(
            &mut config.ui.toolbar.use_icons,
            input_state.toolbar_use_icons,
        ),
        MoreColors => store(
            &mut config.ui.toolbar.show_more_colors,
            input_state.show_more_colors,
        ),
        ContextAwareUi => store(
            &mut config.ui.toolbar.context_aware_ui,
            input_state.context_aware_ui,
        ),
        PresetToasts => store(
            &mut config.ui.toolbar.show_preset_toasts,
            input_state.show_preset_toasts,
        ),
        ToolPreview => store(
            &mut config.ui.toolbar.show_tool_preview,
            effective_tool_preview_value(
                input_state.show_tool_preview,
                input_state
                    .presenter_restore
                    .as_ref()
                    .and_then(|restore| restore.show_tool_preview),
            ),
        ),
        DelaySliders => store(
            &mut config.ui.toolbar.show_delay_sliders,
            input_state.show_delay_sliders,
        ),
    }
}

fn apply_ui_field(config: &mut Config, input_state: &InputState, field: UiPreferenceField) -> bool {
    match field {
        UiPreferenceField::StatusBar => {
            store(&mut config.ui.show_status_bar, input_state.show_status_bar)
        }
        UiPreferenceField::StatusBarInteractive => store(
            &mut config.ui.status_bar_interactive,
            input_state.status_bar_interactive,
        ),
        UiPreferenceField::StatusBarItem(item) => {
            let visible = input_state.status_bar_item_visible(item);
            let changed = config.ui.status_bar_item_visible(item) != visible;
            config.ui.set_status_bar_item_visible(item, visible);
            changed
        }
        UiPreferenceField::StatusBoardBadge => store(
            &mut config.ui.show_status_board_badge,
            input_state.show_status_board_badge,
        ),
        UiPreferenceField::StatusPageBadge => store(
            &mut config.ui.show_status_page_badge,
            input_state.show_status_page_badge,
        ),
        UiPreferenceField::FloatingBadgeAlways => store(
            &mut config.ui.show_floating_badge_always,
            input_state.show_floating_badge_always,
        ),
    }
}

/// Presenter mode forces the click highlight on while it runs, so the value
/// the effective config keeps is the user's own, not the mode's.
fn apply_click_highlight(config: &mut Config, input_state: &InputState) -> bool {
    let mut changed = store(
        &mut config.ui.click_highlight.show_on_highlight_tool,
        input_state.highlight_tool_ring_enabled(),
    );
    if !(input_state.presenter_mode && input_state.presenter_mode_config.enable_click_highlight) {
        changed |= store(
            &mut config.ui.click_highlight.enabled,
            input_state.click_highlight_enabled(),
        );
    }
    changed
}

/// While presenter mode forces the HUD on, the runtime value is the mode's,
/// not the user's, so the effective config keeps the user's until the mode
/// releases it (the same contract `apply_click_highlight` follows).
fn apply_input_hud(config: &mut Config, input_state: &InputState) -> bool {
    if input_state.presenter_mode && input_state.presenter_mode_config.enable_input_hud {
        return false;
    }
    store(
        &mut config.ui.input_hud.enabled,
        input_state.input_hud_enabled(),
    )
}

fn store<T: PartialEq>(field: &mut T, value: T) -> bool {
    let changed = *field != value;
    *field = value;
    changed
}

impl WaylandState {
    /// Apply one authored preference to the effective config for this run and
    /// announce, once per session, that it is not a durable edit.
    pub(super) fn apply_effective_toolbar_preference(&mut self, preference: ToolbarPreference) {
        if !apply_toolbar_preference(&mut self.config, &self.input_state, preference) {
            return;
        }
        self.input_state.notify_process_only_preference();
    }

    /// Follow a click-highlight change the user made outside the toolbar
    /// (keyboard action or command palette).
    pub(in crate::backend::wayland) fn apply_click_highlight_preferences(&mut self) {
        self.apply_effective_toolbar_preference(ToolbarPreference::ClickHighlight);
    }

    /// Follow an input-HUD change the user made outside the toolbar.
    pub(in crate::backend::wayland) fn apply_input_hud_preferences(&mut self) {
        self.apply_effective_toolbar_preference(ToolbarPreference::InputHud);
    }
}
