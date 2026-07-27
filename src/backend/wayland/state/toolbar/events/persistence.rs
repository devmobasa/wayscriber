use super::*;
use crate::backend::wayland::config_writer::ConfigMutation;
use crate::ui::toolbar::model::{ToolbarConfigPersistenceTarget, ToolbarUiPersistenceTarget};

pub(super) fn persisted_tool_preview_value(current: bool, presenter_restore: Option<bool>) -> bool {
    presenter_restore.unwrap_or(current)
}

/// While presenter mode owns the top strip, persist its saved pre-presenter
/// display mode rather than the temporary live mapping.
pub(super) fn persisted_top_display_mode_value(
    current: crate::config::TopDisplayMode,
    presenter_restore: Option<crate::config::TopDisplayMode>,
) -> crate::config::TopDisplayMode {
    // Hidden persists as Full: like the F9 visibility toggle, a hidden
    // strip is runtime-only and `top_pinned` governs startup.
    presenter_restore.unwrap_or(current).persisted()
}

#[cfg(test)]
pub(super) fn apply_toolbar_ui_config_target(
    config: &mut crate::config::Config,
    input_state: &InputState,
    target: ToolbarUiPersistenceTarget,
) {
    let _ = toolbar_ui_config_mutation(input_state, target).apply(config);
}

fn toolbar_ui_config_mutation(
    input_state: &InputState,
    target: ToolbarUiPersistenceTarget,
) -> ConfigMutation {
    match target {
        ToolbarUiPersistenceTarget::StatusBar => {
            ConfigMutation::ShowStatusBar(input_state.show_status_bar)
        }
        ToolbarUiPersistenceTarget::StatusBarInteractive => {
            ConfigMutation::StatusBarInteractive(input_state.status_bar_interactive)
        }
        ToolbarUiPersistenceTarget::StatusBarItem(item) => ConfigMutation::StatusBarItem {
            item,
            visible: input_state.status_bar_item_visible(item),
        },
        ToolbarUiPersistenceTarget::StatusBoardBadge => {
            ConfigMutation::StatusBoardBadge(input_state.show_status_board_badge)
        }
        ToolbarUiPersistenceTarget::StatusPageBadge => {
            ConfigMutation::StatusPageBadge(input_state.show_status_page_badge)
        }
        ToolbarUiPersistenceTarget::FloatingBadgeAlways => {
            ConfigMutation::FloatingBadgeAlways(input_state.show_floating_badge_always)
        }
        ToolbarUiPersistenceTarget::FloatingBadge => {
            ConfigMutation::FloatingBadge(input_state.show_floating_badge)
        }
        ToolbarUiPersistenceTarget::ZoomChip => {
            ConfigMutation::ZoomChip(input_state.show_zoom_chip)
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ToolbarPositions {
    pub(super) top_x: f64,
    pub(super) top_y: f64,
    pub(super) side_x: f64,
    pub(super) side_y: f64,
}

#[cfg(test)]
pub(super) fn apply_toolbar_config_target(
    config: &mut crate::config::Config,
    input_state: &InputState,
    positions: ToolbarPositions,
    target: ToolbarConfigPersistenceTarget,
) {
    let _ = toolbar_config_mutation(input_state, positions, target).apply(config);
}

fn toolbar_config_mutation(
    input_state: &InputState,
    positions: ToolbarPositions,
    target: ToolbarConfigPersistenceTarget,
) -> ConfigMutation {
    use ToolbarConfigPersistenceTarget::*;

    match target {
        LayoutMode => ConfigMutation::ToolbarLayout {
            mode: input_state.toolbar_layout_mode,
            sections: crate::config::ToolbarSectionVisibility {
                show_actions_section: input_state.show_actions_section,
                show_actions_advanced: input_state.show_actions_advanced,
                show_zoom_actions: input_state.show_zoom_actions,
                show_pages_section: input_state.show_pages_section,
                show_boards_section: input_state.show_boards_section,
                show_presets: input_state.show_presets,
                show_step_section: input_state.show_step_section,
                show_text_controls: input_state.show_text_controls,
                show_settings_section: input_state.show_settings_section,
            },
        },
        SectionVisibility(flag) => {
            let id = flag.item_id();
            let setting =
                crate::config::item_visibility_setting(&input_state.resolved_toolbar_items, id);
            let visible = crate::config::resolve_section_visibility(
                input_state.toolbar_layout_mode,
                &input_state.toolbar_mode_overrides,
                &input_state.resolved_toolbar_items,
            )
            .get(flag);
            ConfigMutation::ToolbarSectionVisibility {
                id,
                setting,
                flag,
                visible,
            }
        }
        TopDisplayMode => ConfigMutation::ToolbarTopDisplayMode(persisted_top_display_mode_value(
            input_state.toolbar_top_display_mode,
            input_state
                .presenter_restore
                .as_ref()
                .and_then(|restore| restore.toolbar_top_display_mode),
        )),
        Icons => ConfigMutation::ToolbarUseIcons(input_state.toolbar_use_icons),
        MoreColors => ConfigMutation::ToolbarShowMoreColors(input_state.show_more_colors),
        ContextAwareUi => ConfigMutation::ToolbarContextAwareUi(input_state.context_aware_ui),
        PresetToasts => ConfigMutation::ToolbarPresetToasts(input_state.show_preset_toasts),
        ToolPreview => ConfigMutation::ToolbarToolPreview(persisted_tool_preview_value(
            input_state.show_tool_preview,
            input_state
                .presenter_restore
                .as_ref()
                .and_then(|restore| restore.show_tool_preview),
        )),
        DelaySliders => ConfigMutation::ToolbarDelaySliders(input_state.show_delay_sliders),
        TopPosition => ConfigMutation::ToolbarTopPosition {
            x: positions.top_x,
            y: positions.top_y,
        },
        SidePosition => {
            // A side drag can change whether the side palette overlaps the
            // top strip. Drag completion reconciles the top strip's X offset
            // against that new base before saving, so persist the derived X
            // together with the side position. The top Y value is unrelated.
            ConfigMutation::ToolbarSidePosition {
                top_x: positions.top_x,
                side_x: positions.side_x,
                side_y: positions.side_y,
            }
        }
    }
}

impl WaylandState {
    pub(in crate::backend::wayland) fn queue_config_mutation(
        &mut self,
        mutation: ConfigMutation,
        description: &str,
    ) -> bool {
        if self.config_writer.request(&mutation) {
            // Keep the runtime baseline aligned with accepted writes. The
            // worker owns retrying the same mutation, so a later config edit
            // cannot reintroduce the stale pre-request value while it waits.
            let _ = mutation.apply(&mut self.config);
            log::debug!("Queued {description}");
            true
        } else {
            log::warn!("Failed to queue {description}; runtime value remains session-only");
            false
        }
    }

    pub(super) fn save_toolbar_config(&mut self, target: ToolbarConfigPersistenceTarget) {
        let mutation = toolbar_config_mutation(
            &self.input_state,
            ToolbarPositions {
                top_x: self.data.toolbar_top_offset,
                top_y: self.data.toolbar_top_offset_y,
                side_x: self.data.toolbar_side_offset_x,
                side_y: self.data.toolbar_side_offset,
            },
            target,
        );
        self.queue_config_mutation(mutation, "toolbar config persistence");
    }

    pub(in crate::backend::wayland) fn save_toolbar_position_config(&mut self, kind: MoveDragKind) {
        let target = match kind {
            MoveDragKind::Top => ToolbarConfigPersistenceTarget::TopPosition,
            MoveDragKind::Side => ToolbarConfigPersistenceTarget::SidePosition,
        };
        self.save_toolbar_config(target);
    }

    pub(in crate::backend::wayland) fn save_toolbar_display_config(&mut self) {
        self.save_toolbar_config(ToolbarConfigPersistenceTarget::TopDisplayMode);
    }

    pub(in crate::backend::wayland) fn save_floating_badge_visibility_config(
        &mut self,
        visible: bool,
    ) {
        self.queue_config_mutation(
            ConfigMutation::FloatingBadge(visible),
            "floating badge visibility persistence",
        );
    }

    pub(in crate::backend::wayland) fn save_zoom_chip_visibility_config(&mut self, visible: bool) {
        self.queue_config_mutation(
            ConfigMutation::ZoomChip(visible),
            "zoom chip visibility persistence",
        );
    }

    pub(super) fn save_toolbar_ui_config(&mut self, target: ToolbarUiPersistenceTarget) {
        let mutation = toolbar_ui_config_mutation(&self.input_state, target);
        self.queue_config_mutation(mutation, "toolbar UI config persistence");
    }

    pub(super) fn save_toolbar_history_config(&mut self) {
        self.queue_config_mutation(
            ConfigMutation::HistoryCustomSection(self.input_state.custom_section_enabled),
            "toolbar history config persistence",
        );
    }

    pub(in crate::backend::wayland) fn save_click_highlight_preferences(&mut self) {
        let enabled = if self.input_state.presenter_mode
            && self
                .input_state
                .presenter_mode_config
                .enable_click_highlight
        {
            None
        } else {
            Some(self.input_state.click_highlight_enabled())
        };
        self.queue_config_mutation(
            ConfigMutation::ClickHighlight {
                enabled,
                show_on_highlight_tool: self.input_state.highlight_tool_ring_enabled(),
            },
            "click highlight preference persistence",
        );
    }

    /// Persist the input HUD's enabled preference.
    ///
    /// While presenter mode forces the HUD on, the runtime value is the mode's,
    /// not the user's, so nothing is written until the mode releases it (the
    /// same contract `save_click_highlight_preferences` follows).
    pub(in crate::backend::wayland) fn save_input_hud_preferences(&mut self) {
        if self.input_state.presenter_mode
            && self.input_state.presenter_mode_config.enable_input_hud
        {
            return;
        }
        self.queue_config_mutation(
            ConfigMutation::InputHud(self.input_state.input_hud_enabled()),
            "input HUD preference persistence",
        );
    }

    pub(in crate::backend::wayland) fn shutdown_config_writer(&mut self) {
        self.config_writer.shutdown();
    }

    /// Flush durable edits that are queued but not yet written. Pointer-driven
    /// accepts (the color picker's OK button) queue their config write from the
    /// release handler, which is not itself a drain site, so the exit path calls
    /// this before the process goes away.
    pub(in crate::backend::wayland) fn persist_pending_config_edits(&mut self) {
        if let Some(edit) = self.input_state.take_pending_quick_color_edit() {
            self.handle_quick_color_edit(edit);
        }
    }

    /// Persist an accepted quick-color recolor. The runtime palette already
    /// shows the color; this writes `drawing.quick_colors` through the same
    /// reload-and-save guard the other runtime-owned config edits use, so a
    /// long-lived snapshot cannot overwrite newer edits from the configurator
    /// or the file itself.
    pub(in crate::backend::wayland) fn handle_quick_color_edit(
        &mut self,
        edit: crate::input::state::QuickColorEdit,
    ) {
        let crate::input::state::QuickColorEdit { index, color } = edit;
        self.queue_config_mutation(
            ConfigMutation::QuickColor { index, color },
            "quick color persistence",
        );
    }

    pub(in crate::backend::wayland) fn handle_preset_action(
        &mut self,
        action: crate::input::state::PresetAction,
    ) {
        let mutation = match action {
            crate::input::state::PresetAction::Save { slot, preset } => {
                ConfigMutation::PresetSlot {
                    slot,
                    preset: Some(preset),
                }
            }
            crate::input::state::PresetAction::Clear { slot } => {
                ConfigMutation::PresetSlot { slot, preset: None }
            }
        };
        self.queue_config_mutation(mutation, "preset persistence");
    }
}
