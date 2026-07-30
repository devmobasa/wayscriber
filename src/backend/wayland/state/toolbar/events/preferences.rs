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
use crate::config::{
    Config, StatusBarItem, ToolbarItemVisibilitySetting, ToolbarSectionFlag,
    item_visibility_setting, resolve_section_visibility, section_flag_for_item,
};

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
    LayoutMode,
    SectionVisibility(ToolbarSectionFlag),
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

impl ToolbarPreference {
    /// Whether changing this preference also moves a runtime-UI seed.
    ///
    /// `runtime_seeds_from_config` derives its seeds from the toolbar item
    /// resolution, which folds `layout_mode`, the legacy section flags, and
    /// `ui.toolbar.items`. Changing any of those has to refresh the seed
    /// registry, or override reconciliation keeps comparing against the
    /// pre-toggle baseline until an unrelated event refreshes it.
    ///
    /// This is deliberately the same superset the write path declared: the
    /// seed registry excludes named section rows today
    /// (`toolbar_section_visibility_is_not_seeded_into_runtime_state`), so
    /// these two only reach the fold's inputs. A redundant reseed is
    /// idempotent; a missing one is not, and the fields feeding the fold are
    /// exactly these.
    pub(super) fn affects_runtime_ui_seeds(self) -> bool {
        match self {
            Self::Toolbar(
                ToolbarPreferenceField::LayoutMode | ToolbarPreferenceField::SectionVisibility(_),
            ) => true,
            Self::Toolbar(_)
            | Self::Ui(_)
            | Self::HistoryCustomSection
            | Self::ClickHighlight
            | Self::InputHud => false,
        }
    }
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
        ToolbarEvent::ToggleActionsSection(_) => {
            Toolbar(SectionVisibility(ToolbarSectionFlag::Actions))
        }
        ToolbarEvent::ToggleActionsAdvanced(_) => {
            Toolbar(SectionVisibility(ToolbarSectionFlag::ActionsAdvanced))
        }
        ToolbarEvent::ToggleZoomActions(_) => {
            Toolbar(SectionVisibility(ToolbarSectionFlag::ZoomActions))
        }
        ToolbarEvent::TogglePagesSection(_) => {
            Toolbar(SectionVisibility(ToolbarSectionFlag::Pages))
        }
        ToolbarEvent::ToggleBoardsSection(_) => {
            Toolbar(SectionVisibility(ToolbarSectionFlag::Boards))
        }
        ToolbarEvent::TogglePresets(_) => Toolbar(SectionVisibility(ToolbarSectionFlag::Presets)),
        ToolbarEvent::ToggleStepSection(_) => {
            Toolbar(SectionVisibility(ToolbarSectionFlag::StepSection))
        }
        ToolbarEvent::ToggleTextControls(_) => {
            Toolbar(SectionVisibility(ToolbarSectionFlag::TextControls))
        }
        // A section row hidden from the customization list is that section's
        // visibility, not a runtime-UI item override (see
        // `persistence_for_event`).
        ToolbarEvent::SetToolbarItemHidden(id, _) => {
            Toolbar(SectionVisibility(section_flag_for_item(*id)?))
        }
        ToolbarEvent::ToggleContextAwareUi(_) => Toolbar(ContextAwareUi),
        ToolbarEvent::TogglePresetToasts(_) => Toolbar(PresetToasts),
        ToolbarEvent::ToggleToolPreview(_) => Toolbar(ToolPreview),
        ToolbarEvent::ToggleDelaySliders(_) => Toolbar(DelaySliders),
        ToolbarEvent::SetToolbarLayoutMode(_) => Toolbar(LayoutMode),
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
        LayoutMode => {
            let mode_changed = store(
                &mut config.ui.toolbar.layout_mode,
                input_state.toolbar_layout_mode,
            );
            let mirrors_changed = rebaseline_legacy_section_flags(config);
            mode_changed || mirrors_changed
        }
        SectionVisibility(flag) => apply_section_visibility(config, input_state, flag),
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

/// Section visibility lives in the canonical item override plus the legacy
/// `show_*` mirror the load fold reads back, so both move together.
fn apply_section_visibility(
    config: &mut Config,
    input_state: &InputState,
    flag: ToolbarSectionFlag,
) -> bool {
    let id = flag.item_id();
    let setting = item_visibility_setting(&input_state.resolved_toolbar_items, id);
    let visible = resolve_section_visibility(
        input_state.toolbar_layout_mode,
        &input_state.toolbar_mode_overrides,
        &input_state.resolved_toolbar_items,
    )
    .get(flag);

    let setting_changed =
        item_visibility_setting(&config.ui.toolbar.items.resolved(), id) != setting;
    config.ui.toolbar.items.set_visibility_setting(id, setting);
    let mirror_changed = section_compatibility_mirror(config, flag) != visible;
    apply_section_compatibility_mirror(config, flag, visible);
    setting_changed || mirror_changed
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

/// Re-baseline the legacy `show_*` mirrors a layout switch leaves behind.
///
/// Loading folds a legacy flag into an explicit item override wherever it
/// disagrees with the active mode's baseline, so a switch that left the old
/// mode's values in place would come back as sections pinned to the mode the
/// user just left — and the seed refresh this run performs reads through that
/// same fold. Only sections without an explicit override need the mirror: the
/// fold skips the rest, and `show_settings_section` is authored-only input the
/// resolver ignores.
///
/// Returns whether any mirror moved, so a switch that only re-baselines is
/// still reported as an effective-config change.
fn rebaseline_legacy_section_flags(config: &mut Config) -> bool {
    let toolbar = &config.ui.toolbar;
    let mode = toolbar.layout_mode;
    let resolved = toolbar.items.resolved();
    let mirrors = ToolbarSectionFlag::ALL.map(|flag| {
        let baseline = matches!(
            item_visibility_setting(&resolved, flag.item_id()),
            ToolbarItemVisibilitySetting::Default
        )
        .then(|| flag.baseline(mode, &toolbar.mode_overrides));
        (flag, baseline)
    });
    let mut changed = false;
    for (flag, baseline) in mirrors {
        if let Some(visible) = baseline {
            changed |= section_compatibility_mirror(config, flag) != visible;
            apply_section_compatibility_mirror(config, flag, visible);
        }
    }
    changed
}

fn section_compatibility_mirror(config: &Config, flag: ToolbarSectionFlag) -> bool {
    let toolbar = &config.ui.toolbar;
    match flag {
        ToolbarSectionFlag::Actions => toolbar.show_actions_section,
        ToolbarSectionFlag::ActionsAdvanced => toolbar.show_actions_advanced,
        ToolbarSectionFlag::ZoomActions => toolbar.show_zoom_actions,
        ToolbarSectionFlag::Pages => toolbar.show_pages_section,
        ToolbarSectionFlag::Boards => toolbar.show_boards_section,
        ToolbarSectionFlag::Presets => toolbar.show_presets,
        ToolbarSectionFlag::StepSection => toolbar.show_step_section,
        ToolbarSectionFlag::TextControls => toolbar.show_text_controls,
    }
}

fn apply_section_compatibility_mirror(
    config: &mut Config,
    flag: ToolbarSectionFlag,
    visible: bool,
) {
    match flag {
        ToolbarSectionFlag::Actions => config.ui.toolbar.show_actions_section = visible,
        ToolbarSectionFlag::ActionsAdvanced => {
            config.ui.toolbar.show_actions_advanced = visible;
        }
        ToolbarSectionFlag::ZoomActions => config.ui.toolbar.show_zoom_actions = visible,
        ToolbarSectionFlag::Pages => config.ui.toolbar.show_pages_section = visible,
        ToolbarSectionFlag::Boards => config.ui.toolbar.show_boards_section = visible,
        ToolbarSectionFlag::Presets => config.ui.toolbar.show_presets = visible,
        ToolbarSectionFlag::StepSection => config.ui.toolbar.show_step_section = visible,
        ToolbarSectionFlag::TextControls => config.ui.toolbar.show_text_controls = visible,
    }
}

impl WaylandState {
    /// Apply one authored preference to the effective config for this run and
    /// announce, once per session, that it is not a durable edit.
    pub(super) fn apply_effective_toolbar_preference(&mut self, preference: ToolbarPreference) {
        if !apply_toolbar_preference(&mut self.config, &self.input_state, preference) {
            return;
        }
        // Seeds derive from the effective config this toggle just changed, so
        // reseed here rather than waiting for an unrelated session, board, or
        // keybinding event: until then, override reconciliation and redundant
        // override deletion would key off the pre-toggle baseline.
        if preference.affects_runtime_ui_seeds() {
            self.refresh_runtime_ui_config_seeds();
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
