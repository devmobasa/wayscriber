use super::*;

impl WaylandState {
    /// Returns a snapshot of the current input state for toolbar UI consumption.
    pub(in crate::backend::wayland) fn toolbar_snapshot(&self) -> ToolbarSnapshot {
        let hints = ToolbarBindingHints::from_input_state(&self.input_state);
        let show_drawer_hint =
            !self.onboarding.state().drawer_hint_shown && !self.input_state.toolbar_drawer_open;
        ToolbarSnapshot::from_input_with_options(&self.input_state, hints, show_drawer_hint)
    }

    /// Applies an incoming toolbar event and schedules redraws as needed.
    pub(in crate::backend::wayland) fn handle_toolbar_event(&mut self, event: ToolbarEvent) {
        // Mark drawer hint as shown when user opens the drawer
        if matches!(event, ToolbarEvent::ToggleDrawer(true))
            && !self.onboarding.state().drawer_hint_shown
        {
            self.onboarding.state_mut().drawer_hint_shown = true;
            self.onboarding.save();
        }

        match event {
            ToolbarEvent::MoveTopToolbar { x, y } => {
                let inline_active = self.inline_toolbars_active();
                let coord_is_screen = inline_active;
                drag_log(format!(
                    "toolbar move event: kind=Top, coord=({:.3}, {:.3}), coord_is_screen={}, inline_active={}",
                    x, y, coord_is_screen, inline_active
                ));
                self.begin_toolbar_move_drag(MoveDragKind::Top, (x, y), coord_is_screen);
                if coord_is_screen {
                    self.handle_toolbar_move_screen(MoveDragKind::Top, (x, y));
                } else {
                    self.handle_toolbar_move(MoveDragKind::Top, (x, y));
                }
                return;
            }
            ToolbarEvent::MoveSideToolbar { x, y } => {
                let inline_active = self.inline_toolbars_active();
                let coord_is_screen = inline_active;
                drag_log(format!(
                    "toolbar move event: kind=Side, coord=({:.3}, {:.3}), coord_is_screen={}, inline_active={}",
                    x, y, coord_is_screen, inline_active
                ));
                self.begin_toolbar_move_drag(MoveDragKind::Side, (x, y), coord_is_screen);
                if coord_is_screen {
                    self.handle_toolbar_move_screen(MoveDragKind::Side, (x, y));
                } else {
                    self.handle_toolbar_move(MoveDragKind::Side, (x, y));
                }
                return;
            }
            _ => {}
        }

        #[cfg(tablet)]
        let prev_thickness = self.input_state.current_thickness;
        #[cfg(tablet)]
        let thickness_event = matches!(
            event,
            ToolbarEvent::SetThickness(_) | ToolbarEvent::NudgeThickness(_)
        );

        // Check if this is a toolbar config event that needs saving
        let needs_config_save = matches!(
            event,
            ToolbarEvent::PinTopToolbar(_)
                | ToolbarEvent::PinSideToolbar(_)
                | ToolbarEvent::ToggleIconMode(_)
                | ToolbarEvent::ToggleMoreColors(_)
                | ToolbarEvent::ToggleActionsSection(_)
                | ToolbarEvent::ToggleActionsAdvanced(_)
                | ToolbarEvent::ToggleZoomActions(_)
                | ToolbarEvent::TogglePagesSection(_)
                | ToolbarEvent::ToggleBoardsSection(_)
                | ToolbarEvent::TogglePresets(_)
                | ToolbarEvent::ToggleStepSection(_)
                | ToolbarEvent::ToggleTextControls(_)
                | ToolbarEvent::TogglePresetToasts(_)
                | ToolbarEvent::ToggleToolPreview(_)
                | ToolbarEvent::ToggleDelaySliders(_)
                | ToolbarEvent::ToggleCustomSection(_)
                | ToolbarEvent::SetToolbarLayoutMode(_)
        );

        let persist_drawing = matches!(
            event,
            ToolbarEvent::SetColor(_)
                | ToolbarEvent::SetThickness(_)
                | ToolbarEvent::SetMarkerOpacity(_)
                | ToolbarEvent::SetEraserMode(_)
                | ToolbarEvent::SetFont(_)
                | ToolbarEvent::SetFontSize(_)
                | ToolbarEvent::ToggleFill(_)
                | ToolbarEvent::ApplyPreset(_)
        );

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

            // Save config when pin state changes
            if needs_config_save {
                self.save_toolbar_pin_config();
            }

            if persist_drawing {
                self.save_drawing_preferences();
            }
        }
        if let Some(action) = self.input_state.take_pending_preset_action() {
            self.handle_preset_action(action);
        }
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
        self.config.ui.toolbar.show_settings_section = self.input_state.show_settings_section;
        self.config.ui.toolbar.show_delay_sliders = self.input_state.show_delay_sliders;
        self.config.ui.toolbar.show_marker_opacity_section =
            self.input_state.show_marker_opacity_section;
        self.config.ui.toolbar.show_preset_toasts = self.input_state.show_preset_toasts;
        self.config.ui.toolbar.top_offset = self.data.toolbar_top_offset;
        self.config.ui.toolbar.top_offset_y = self.data.toolbar_top_offset_y;
        self.config.ui.toolbar.side_offset = self.data.toolbar_side_offset;
        self.config.ui.toolbar.side_offset_x = self.data.toolbar_side_offset_x;
        // Step controls toggle is in history config
        self.config.history.custom_section_enabled = self.input_state.custom_section_enabled;

        if let Err(err) = self.config.save() {
            log::warn!("Failed to save toolbar config: {}", err);
        } else {
            log::debug!("Saved toolbar config");
        }
    }

    pub(in crate::backend::wayland) fn save_drawing_preferences(&mut self) {
        self.config.drawing.default_color = ColorSpec::from(self.input_state.current_color);
        self.config.drawing.default_thickness = self.input_state.current_thickness;
        self.config.drawing.default_eraser_mode = self.input_state.eraser_mode;
        self.config.drawing.default_fill_enabled = self.input_state.fill_enabled;
        self.config.drawing.default_font_size = self.input_state.current_font_size;
        self.config.drawing.font_family = self.input_state.font_descriptor.family.clone();
        self.config.drawing.font_weight = self.input_state.font_descriptor.weight.clone();
        self.config.drawing.font_style = self.input_state.font_descriptor.style.clone();
        self.config.drawing.marker_opacity = self.input_state.marker_opacity;

        if let Err(err) = self.config.save() {
            log::warn!("Failed to persist drawing preferences: {}", err);
        }
    }

    pub(in crate::backend::wayland) fn handle_preset_action(
        &mut self,
        action: crate::input::state::PresetAction,
    ) {
        match action {
            crate::input::state::PresetAction::Save { slot, preset } => {
                self.config.presets.set_slot(slot, Some(preset));
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
