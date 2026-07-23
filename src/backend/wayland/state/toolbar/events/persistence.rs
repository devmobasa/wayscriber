use super::*;
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

pub(super) fn apply_toolbar_ui_config_target(
    config: &mut crate::config::Config,
    input_state: &InputState,
    target: ToolbarUiPersistenceTarget,
) {
    match target {
        ToolbarUiPersistenceTarget::StatusBar => {
            config.ui.show_status_bar = input_state.show_status_bar;
        }
        ToolbarUiPersistenceTarget::StatusBoardBadge => {
            config.ui.show_status_board_badge = input_state.show_status_board_badge;
        }
        ToolbarUiPersistenceTarget::StatusPageBadge => {
            config.ui.show_status_page_badge = input_state.show_status_page_badge;
        }
        ToolbarUiPersistenceTarget::FloatingBadgeAlways => {
            config.ui.show_floating_badge_always = input_state.show_floating_badge_always;
        }
        ToolbarUiPersistenceTarget::FloatingBadge => {
            config.ui.show_floating_badge = input_state.show_floating_badge;
        }
        ToolbarUiPersistenceTarget::ZoomChip => {
            config.ui.toolbar.show_zoom_chip = input_state.show_zoom_chip;
        }
    }
}

fn apply_toolbar_ui_visibility_value(
    config: &mut crate::config::Config,
    target: ToolbarUiPersistenceTarget,
    visible: bool,
) {
    match target {
        ToolbarUiPersistenceTarget::FloatingBadge => {
            config.ui.show_floating_badge = visible;
        }
        ToolbarUiPersistenceTarget::ZoomChip => {
            config.ui.toolbar.show_zoom_chip = visible;
        }
        _ => unreachable!("only master-visibility targets carry authored values"),
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ToolbarPositions {
    pub(super) top_x: f64,
    pub(super) top_y: f64,
    pub(super) side_x: f64,
    pub(super) side_y: f64,
}

fn apply_all_section_compatibility_mirrors(
    config: &mut crate::config::Config,
    input_state: &InputState,
) {
    config.ui.toolbar.show_actions_section = input_state.show_actions_section;
    config.ui.toolbar.show_actions_advanced = input_state.show_actions_advanced;
    config.ui.toolbar.show_zoom_actions = input_state.show_zoom_actions;
    config.ui.toolbar.show_pages_section = input_state.show_pages_section;
    config.ui.toolbar.show_boards_section = input_state.show_boards_section;
    config.ui.toolbar.show_presets = input_state.show_presets;
    config.ui.toolbar.show_step_section = input_state.show_step_section;
    config.ui.toolbar.show_text_controls = input_state.show_text_controls;
    config.ui.toolbar.show_settings_section = input_state.show_settings_section;
}

fn apply_section_compatibility_mirror(
    config: &mut crate::config::Config,
    flag: crate::config::ToolbarSectionFlag,
    visible: bool,
) {
    use crate::config::ToolbarSectionFlag;

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

pub(super) fn apply_toolbar_config_target(
    config: &mut crate::config::Config,
    input_state: &InputState,
    positions: ToolbarPositions,
    target: ToolbarConfigPersistenceTarget,
) {
    use ToolbarConfigPersistenceTarget::*;

    match target {
        LayoutMode => {
            config.ui.toolbar.layout_mode = input_state.toolbar_layout_mode;
            apply_all_section_compatibility_mirrors(config, input_state);
        }
        SectionVisibility(flag) => {
            let id = flag.item_id();
            let setting =
                crate::config::item_visibility_setting(&input_state.resolved_toolbar_items, id);
            config.ui.toolbar.items.set_visibility_setting(id, setting);
            let visible = crate::config::resolve_section_visibility(
                input_state.toolbar_layout_mode,
                &input_state.toolbar_mode_overrides,
                &input_state.resolved_toolbar_items,
            )
            .get(flag);
            apply_section_compatibility_mirror(config, flag, visible);
        }
        TopDisplayMode => {
            config.ui.toolbar.top_display_mode = persisted_top_display_mode_value(
                input_state.toolbar_top_display_mode,
                input_state
                    .presenter_restore
                    .as_ref()
                    .and_then(|restore| restore.toolbar_top_display_mode),
            );
        }
        Icons => config.ui.toolbar.use_icons = input_state.toolbar_use_icons,
        MoreColors => config.ui.toolbar.show_more_colors = input_state.show_more_colors,
        ContextAwareUi => config.ui.toolbar.context_aware_ui = input_state.context_aware_ui,
        PresetToasts => config.ui.toolbar.show_preset_toasts = input_state.show_preset_toasts,
        ToolPreview => {
            config.ui.toolbar.show_tool_preview = persisted_tool_preview_value(
                input_state.show_tool_preview,
                input_state
                    .presenter_restore
                    .as_ref()
                    .and_then(|restore| restore.show_tool_preview),
            );
        }
        DelaySliders => config.ui.toolbar.show_delay_sliders = input_state.show_delay_sliders,
        TopPosition => {
            config.ui.toolbar.top_offset = positions.top_x;
            config.ui.toolbar.top_offset_y = positions.top_y;
        }
        SidePosition => {
            // A side drag can change whether the side palette overlaps the
            // top strip. Drag completion reconciles the top strip's X offset
            // against that new base before saving, so persist the derived X
            // together with the side position. The top Y value is unrelated.
            config.ui.toolbar.top_offset = positions.top_x;
            config.ui.toolbar.side_offset_x = positions.side_x;
            config.ui.toolbar.side_offset = positions.side_y;
        }
    }
}

impl WaylandState {
    pub(super) fn save_toolbar_config(&mut self, target: ToolbarConfigPersistenceTarget) {
        apply_toolbar_config_target(
            &mut self.config,
            &self.input_state,
            ToolbarPositions {
                top_x: self.data.toolbar_top_offset,
                top_y: self.data.toolbar_top_offset_y,
                side_x: self.data.toolbar_side_offset_x,
                side_y: self.data.toolbar_side_offset,
            },
            target,
        );

        if let Err(err) = self.config.save() {
            log::warn!("Failed to save toolbar config: {}", err);
        } else {
            log::debug!("Saved toolbar config");
        }
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
        self.save_toolbar_ui_visibility_config(ToolbarUiPersistenceTarget::FloatingBadge, visible);
    }

    pub(in crate::backend::wayland) fn save_zoom_chip_visibility_config(&mut self, visible: bool) {
        self.save_toolbar_ui_visibility_config(ToolbarUiPersistenceTarget::ZoomChip, visible);
    }

    fn save_toolbar_ui_visibility_config(
        &mut self,
        target: ToolbarUiPersistenceTarget,
        visible: bool,
    ) {
        let save_result = crate::config::Config::update_file(|config| {
            apply_toolbar_ui_visibility_value(config, target, visible);
        });
        match save_result {
            Ok(()) => {
                apply_toolbar_ui_visibility_value(&mut self.config, target, visible);
                log::debug!("Saved toolbar UI visibility config");
            }
            Err(err) => log::warn!("Failed to save toolbar UI visibility config: {}", err),
        }
    }

    pub(super) fn save_toolbar_ui_config(&mut self, target: ToolbarUiPersistenceTarget) {
        let save_result = crate::config::Config::update_file(|config| {
            apply_toolbar_ui_config_target(config, &self.input_state, target);
        });

        match save_result {
            Ok(()) => {
                // Keep the runtime's config baseline aligned only after the
                // durable write succeeds. On failure the live InputState value
                // remains in effect, but cannot hitchhike on a later save.
                apply_toolbar_ui_config_target(&mut self.config, &self.input_state, target);
                log::debug!("Saved toolbar UI config");
            }
            Err(err) => log::warn!("Failed to save toolbar UI config: {}", err),
        }
    }

    pub(super) fn save_toolbar_history_config(&mut self) {
        self.config.history.custom_section_enabled = self.input_state.custom_section_enabled;

        if let Err(err) = self.config.save() {
            log::warn!("Failed to save toolbar history config: {}", err);
        } else {
            log::debug!("Saved toolbar history config");
        }
    }

    pub(in crate::backend::wayland) fn save_click_highlight_preferences(&mut self) {
        if !(self.input_state.presenter_mode
            && self
                .input_state
                .presenter_mode_config
                .enable_click_highlight)
        {
            self.config.ui.click_highlight.enabled = self.input_state.click_highlight_enabled();
        }
        self.config.ui.click_highlight.show_on_highlight_tool =
            self.input_state.highlight_tool_ring_enabled();
        if let Err(err) = self.config.save() {
            log::warn!("Failed to persist click highlight preferences: {}", err);
        }
    }

    pub(in crate::backend::wayland) fn handle_preset_action(
        &mut self,
        action: crate::input::state::PresetAction,
    ) {
        match action {
            crate::input::state::PresetAction::Save { slot, preset } => {
                self.config.presets.set_slot(slot, Some(*preset));
                if let Err(err) = self.config.save() {
                    log::warn!("Failed to save preset slot {}: {}", slot, err);
                }
            }
            crate::input::state::PresetAction::Clear { slot } => {
                self.config.presets.set_slot(slot, None);
                if let Err(err) = self.config.save() {
                    log::warn!("Failed to clear preset slot {}: {}", slot, err);
                }
            }
        }
    }
}
