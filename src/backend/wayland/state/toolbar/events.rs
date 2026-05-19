use super::*;
use crate::{
    input::InputState,
    onboarding::OnboardingState,
    ui::toolbar::model::{
        ToolbarBackendRoute, ToolbarEventPolicy, ToolbarPersistence, ToolbarPersistenceTarget,
        ToolbarPreApplyEffect, ToolbarUiPersistenceTarget,
    },
};

fn persisted_tool_preview_value(current: bool, presenter_restore: Option<bool>) -> bool {
    presenter_restore.unwrap_or(current)
}

fn record_drawer_hint_shown(state: &mut OnboardingState) -> bool {
    if state.drawer_hint_count >= crate::onboarding::DRAWER_HINT_MAX {
        return false;
    }

    state.drawer_hint_count = state.drawer_hint_count.saturating_add(1);
    state.drawer_hint_shown = state.drawer_hint_count >= crate::onboarding::DRAWER_HINT_MAX;
    true
}

fn apply_toolbar_ui_config_target(
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
    }
}

impl WaylandState {
    /// Returns a snapshot of the current input state for toolbar UI consumption.
    pub(in crate::backend::wayland) fn toolbar_snapshot(&self) -> ToolbarSnapshot {
        let hints = ToolbarBindingHints::from_input_state(&self.input_state);
        let hint_max = crate::onboarding::DRAWER_HINT_MAX;
        let show_drawer_hint = self.onboarding.state().drawer_hint_count < hint_max
            && !self.input_state.toolbar_drawer_open;
        ToolbarSnapshot::from_input_with_options(&self.input_state, hints, show_drawer_hint)
    }

    /// Applies an incoming toolbar event and schedules redraws as needed.
    pub(in crate::backend::wayland) fn handle_toolbar_event(&mut self, event: ToolbarEvent) {
        let policy = ToolbarEventPolicy::for_event(&event);
        for effect in &policy.pre_apply_effects {
            match effect {
                ToolbarPreApplyEffect::RecordDrawerHintShown => {
                    if record_drawer_hint_shown(self.onboarding.state_mut()) {
                        self.onboarding.save();
                    }
                }
            }
        }

        match (&policy.backend_route, &event) {
            (ToolbarBackendRoute::MoveTopToolbar, ToolbarEvent::MoveTopToolbar { x, y }) => {
                let inline_active = self.inline_toolbars_active();
                let coord_is_screen = inline_active;
                drag_log(format!(
                    "toolbar move event: kind=Top, coord=({:.3}, {:.3}), coord_is_screen={}, inline_active={}",
                    *x, *y, coord_is_screen, inline_active
                ));
                self.begin_toolbar_move_drag(MoveDragKind::Top, (*x, *y), coord_is_screen);
                if coord_is_screen {
                    self.handle_toolbar_move_screen(MoveDragKind::Top, (*x, *y));
                } else {
                    self.handle_toolbar_move(MoveDragKind::Top, (*x, *y));
                }
                return;
            }
            (ToolbarBackendRoute::MoveSideToolbar, ToolbarEvent::MoveSideToolbar { x, y }) => {
                let inline_active = self.inline_toolbars_active();
                let coord_is_screen = inline_active;
                drag_log(format!(
                    "toolbar move event: kind=Side, coord=({:.3}, {:.3}), coord_is_screen={}, inline_active={}",
                    *x, *y, coord_is_screen, inline_active
                ));
                self.begin_toolbar_move_drag(MoveDragKind::Side, (*x, *y), coord_is_screen);
                if coord_is_screen {
                    self.handle_toolbar_move_screen(MoveDragKind::Side, (*x, *y));
                } else {
                    self.handle_toolbar_move(MoveDragKind::Side, (*x, *y));
                }
                return;
            }
            (ToolbarBackendRoute::ApplyToInput, _)
            | (ToolbarBackendRoute::MoveTopToolbar, _)
            | (ToolbarBackendRoute::MoveSideToolbar, _) => {}
        }

        #[cfg(tablet)]
        let prev_thickness = self.input_state.current_thickness;
        #[cfg(tablet)]
        let thickness_event = policy.tablet_thickness_sensitive;

        if self.input_state.apply_toolbar_event(event) {
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;

            #[cfg(tablet)]
            if thickness_event {
                self.sync_stylus_thickness_cache(prev_thickness);
                if self.stylus_tip_down {
                    self.record_stylus_peak(self.input_state.current_thickness);
                } else {
                    self.stylus_peak_thickness = None;
                }
            }

            match policy.persistence {
                ToolbarPersistence::RuntimeOnly => {}
                ToolbarPersistence::Persist(ToolbarPersistenceTarget::Toolbar) => {
                    self.save_toolbar_pin_config();
                }
                ToolbarPersistence::Persist(ToolbarPersistenceTarget::Ui(target)) => {
                    self.save_toolbar_ui_config(target);
                }
                ToolbarPersistence::Persist(ToolbarPersistenceTarget::History) => {
                    self.save_toolbar_history_config();
                }
                ToolbarPersistence::Persist(ToolbarPersistenceTarget::ClickHighlight) => {
                    self.save_click_highlight_preferences();
                }
            }
        }
        if let Some(action) = self.input_state.take_pending_preset_action() {
            self.handle_preset_action(action);
        }
        if self.input_state.take_pending_copy_hex() {
            self.handle_copy_hex_color();
        }
        if self.input_state.take_pending_paste_hex() {
            self.handle_paste_hex_color();
        }
        self.drain_clipboard_requests();
        self.refresh_keyboard_interactivity();
    }

    #[cfg(tablet)]
    fn sync_stylus_thickness_cache(&mut self, prev: f64) {
        let cur = self.input_state.current_thickness;
        if (cur - prev).abs() > f64::EPSILON {
            self.stylus_base_thickness = Some(cur);
            if self.stylus_tip_down {
                self.stylus_pressure_thickness = Some(cur);
            } else {
                self.stylus_pressure_thickness = None;
            }
        }
    }

    /// Records the maximum stylus thickness seen during the current stroke.
    #[cfg(tablet)]
    pub(in crate::backend::wayland) fn record_stylus_peak(&mut self, thickness: f64) {
        self.stylus_peak_thickness = Some(
            self.stylus_peak_thickness
                .map_or(thickness, |p| p.max(thickness)),
        );
    }

    /// Saves the current toolbar configuration to disk (pinned state, icon mode, section visibility).
    pub(super) fn save_toolbar_pin_config(&mut self) {
        self.config.ui.toolbar.layout_mode = self.input_state.toolbar_layout_mode;
        self.config.ui.toolbar.top_pinned = self.input_state.toolbar_top_pinned;
        self.config.ui.toolbar.side_pinned = self.input_state.toolbar_side_pinned;
        self.config.ui.toolbar.use_icons = self.input_state.toolbar_use_icons;
        self.config.ui.toolbar.show_more_colors = self.input_state.show_more_colors;
        self.config.ui.toolbar.show_actions_section = self.input_state.show_actions_section;
        self.config.ui.toolbar.show_actions_advanced = self.input_state.show_actions_advanced;
        self.config.ui.toolbar.show_zoom_actions = self.input_state.show_zoom_actions;
        self.config.ui.toolbar.show_pages_section = self.input_state.show_pages_section;
        self.config.ui.toolbar.show_boards_section = self.input_state.show_boards_section;
        self.config.ui.toolbar.show_presets = self.input_state.show_presets;
        self.config.ui.toolbar.show_step_section = self.input_state.show_step_section;
        self.config.ui.toolbar.show_text_controls = self.input_state.show_text_controls;
        self.config.ui.toolbar.context_aware_ui = self.input_state.context_aware_ui;
        self.config.ui.toolbar.show_settings_section = self.input_state.show_settings_section;
        self.config.ui.toolbar.show_delay_sliders = self.input_state.show_delay_sliders;
        self.config.ui.toolbar.show_marker_opacity_section =
            self.input_state.show_marker_opacity_section;
        self.config.ui.toolbar.show_preset_toasts = self.input_state.show_preset_toasts;
        self.config.ui.toolbar.show_tool_preview = persisted_tool_preview_value(
            self.input_state.show_tool_preview,
            self.input_state
                .presenter_restore
                .as_ref()
                .and_then(|restore| restore.show_tool_preview),
        );
        self.config.ui.toolbar.top_offset = self.data.toolbar_top_offset;
        self.config.ui.toolbar.top_offset_y = self.data.toolbar_top_offset_y;
        self.config.ui.toolbar.side_offset = self.data.toolbar_side_offset;
        self.config.ui.toolbar.side_offset_x = self.data.toolbar_side_offset_x;

        if let Err(err) = self.config.save() {
            log::warn!("Failed to save toolbar config: {}", err);
        } else {
            log::debug!("Saved toolbar config");
        }
    }

    fn save_toolbar_ui_config(&mut self, target: ToolbarUiPersistenceTarget) {
        apply_toolbar_ui_config_target(&mut self.config, &self.input_state, target);

        if let Err(err) = self.config.save() {
            log::warn!("Failed to save toolbar UI config: {}", err);
        } else {
            log::debug!("Saved toolbar UI config");
        }
    }

    fn save_toolbar_history_config(&mut self) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ToolbarLayoutMode;
    use crate::draw::{Color, FontDescriptor};
    use crate::input::state::test_support::make_test_input_state;
    use crate::input::{EraserMode, Tool};

    fn persistence_for(event: &ToolbarEvent) -> ToolbarPersistence {
        ToolbarEventPolicy::for_event(event).persistence
    }

    #[test]
    fn runtime_toolbar_events_do_not_directly_save_config() {
        let events = vec![
            ToolbarEvent::SelectTool(Tool::Line),
            ToolbarEvent::SetColor(Color {
                r: 0.1,
                g: 0.2,
                b: 0.3,
                a: 1.0,
            }),
            ToolbarEvent::SetThickness(8.0),
            ToolbarEvent::NudgeThickness(1.0),
            ToolbarEvent::SetMarkerOpacity(0.5),
            ToolbarEvent::NudgeMarkerOpacity(0.1),
            ToolbarEvent::SetEraserMode(EraserMode::Stroke),
            ToolbarEvent::SetFont(FontDescriptor::new(
                "Monospace".to_string(),
                "normal".to_string(),
                "italic".to_string(),
            )),
            ToolbarEvent::SetFontSize(44.0),
            ToolbarEvent::ToggleFill(true),
            ToolbarEvent::ApplyPreset(1),
        ];

        for event in events {
            assert_eq!(
                persistence_for(&event),
                ToolbarPersistence::RuntimeOnly,
                "{event:?} should not directly save config"
            );
        }
    }

    #[test]
    fn toolbar_preference_events_save_toolbar_config() {
        let events = vec![
            ToolbarEvent::PinTopToolbar(true),
            ToolbarEvent::PinSideToolbar(true),
            ToolbarEvent::ToggleIconMode(true),
            ToolbarEvent::ToggleMoreColors(true),
            ToolbarEvent::ToggleActionsSection(true),
            ToolbarEvent::ToggleActionsAdvanced(true),
            ToolbarEvent::ToggleZoomActions(true),
            ToolbarEvent::TogglePagesSection(true),
            ToolbarEvent::ToggleBoardsSection(true),
            ToolbarEvent::TogglePresets(true),
            ToolbarEvent::ToggleStepSection(true),
            ToolbarEvent::ToggleTextControls(true),
            ToolbarEvent::ToggleContextAwareUi(true),
            ToolbarEvent::TogglePresetToasts(true),
            ToolbarEvent::ToggleToolPreview(true),
            ToolbarEvent::ToggleDelaySliders(true),
            ToolbarEvent::SetToolbarLayoutMode(ToolbarLayoutMode::Advanced),
        ];

        for event in events {
            assert_eq!(
                persistence_for(&event),
                ToolbarPersistence::Persist(ToolbarPersistenceTarget::Toolbar),
                "{event:?} should save toolbar config"
            );
        }
    }

    #[test]
    fn ui_and_history_preference_events_save_their_own_config_targets() {
        let ui_events = [
            (
                ToolbarEvent::ToggleStatusBar(true),
                ToolbarUiPersistenceTarget::StatusBar,
            ),
            (
                ToolbarEvent::ToggleStatusBoardBadge(true),
                ToolbarUiPersistenceTarget::StatusBoardBadge,
            ),
            (
                ToolbarEvent::ToggleStatusPageBadge(true),
                ToolbarUiPersistenceTarget::StatusPageBadge,
            ),
            (
                ToolbarEvent::ToggleFloatingBadgeAlways(true),
                ToolbarUiPersistenceTarget::FloatingBadgeAlways,
            ),
        ];

        for (event, target) in ui_events {
            assert_eq!(
                persistence_for(&event),
                ToolbarPersistence::Persist(ToolbarPersistenceTarget::Ui(target)),
                "{event:?} should save only its UI config field"
            );
        }

        assert_eq!(
            persistence_for(&ToolbarEvent::ToggleCustomSection(true)),
            ToolbarPersistence::Persist(ToolbarPersistenceTarget::History)
        );
    }

    #[test]
    fn toolbar_ui_config_target_save_leaves_sibling_fields_unchanged() {
        let mut config = crate::config::Config::default();
        config.ui.show_status_bar = true;
        config.ui.show_status_board_badge = false;
        config.ui.show_status_page_badge = true;
        config.ui.show_floating_badge_always = false;

        let mut input_state = make_test_input_state();
        input_state.show_status_bar = false;
        input_state.show_status_board_badge = true;
        input_state.show_status_page_badge = false;
        input_state.show_floating_badge_always = true;

        apply_toolbar_ui_config_target(
            &mut config,
            &input_state,
            ToolbarUiPersistenceTarget::StatusBoardBadge,
        );

        assert!(config.ui.show_status_bar);
        assert!(config.ui.show_status_board_badge);
        assert!(config.ui.show_status_page_badge);
        assert!(!config.ui.show_floating_badge_always);
    }

    #[test]
    fn click_highlight_toolbar_events_are_explicit_config_exceptions() {
        let events = vec![
            ToolbarEvent::ToggleAllHighlight(true),
            ToolbarEvent::SelectTool(Tool::Highlight),
            ToolbarEvent::ToggleHighlightToolRing(true),
        ];

        for event in events {
            assert_eq!(
                persistence_for(&event),
                ToolbarPersistence::Persist(ToolbarPersistenceTarget::ClickHighlight),
                "{event:?} should save click-highlight config"
            );
        }
    }

    #[test]
    fn drawer_hint_pre_apply_effect_is_conditionally_recorded_below_max() {
        let mut state = OnboardingState {
            drawer_hint_count: crate::onboarding::DRAWER_HINT_MAX - 1,
            drawer_hint_shown: false,
            ..OnboardingState::default()
        };

        assert!(record_drawer_hint_shown(&mut state));
        assert_eq!(state.drawer_hint_count, crate::onboarding::DRAWER_HINT_MAX);
        assert!(state.drawer_hint_shown);
    }

    #[test]
    fn drawer_hint_pre_apply_effect_is_ignored_at_max() {
        let mut state = OnboardingState {
            drawer_hint_count: crate::onboarding::DRAWER_HINT_MAX,
            drawer_hint_shown: true,
            ..OnboardingState::default()
        };

        assert!(!record_drawer_hint_shown(&mut state));
        assert_eq!(state.drawer_hint_count, crate::onboarding::DRAWER_HINT_MAX);
        assert!(state.drawer_hint_shown);
    }

    #[test]
    fn tool_preview_config_preserves_presenter_mode_restore_value() {
        assert!(persisted_tool_preview_value(false, Some(true)));
        assert!(!persisted_tool_preview_value(false, Some(false)));
        assert!(persisted_tool_preview_value(true, None));
        assert!(!persisted_tool_preview_value(false, None));
    }
}
