use super::*;
use crate::{
    input::InputState,
    onboarding::OnboardingState,
    ui::toolbar::model::{
        ToolbarBackendRoute, ToolbarEventPolicy, ToolbarPersistence, ToolbarPersistenceTarget,
        ToolbarPreApplyEffect, ToolbarUiPersistenceTarget,
    },
};
use wayland_client::{Connection, QueueHandle};

mod session;
pub(in crate::backend::wayland::state) use session::SessionFileDialogController;

use session::populate_session_snapshot;

fn persisted_tool_preview_value(current: bool, presenter_restore: Option<bool>) -> bool {
    presenter_restore.unwrap_or(current)
}

/// While presenter mode owns the top strip (`[presenter_mode] toolbar_mode =
/// "micro"`), the live display mode and minimized flag hold presenter-mapped
/// values (`Micro`, force-cleared `false`), not user preferences. Any
/// `Persist(Toolbar)` event fired during presenter mode — a pin toggle, an
/// icon-mode switch, ... — must therefore write the saved pre-presenter
/// values from `PresenterRestore`. Outside presenter mode (and under the
/// `"hidden"` mapping, which leaves both fields untouched) the restore slots
/// are `None` and the live values persist as usual.
fn persisted_top_minimized_value(current: bool, presenter_restore: Option<bool>) -> bool {
    presenter_restore.unwrap_or(current)
}

fn persisted_top_display_mode_value(
    current: crate::config::TopDisplayMode,
    presenter_restore: Option<crate::config::TopDisplayMode>,
) -> crate::config::TopDisplayMode {
    // Hidden persists as Full: like the F9 visibility toggle, a hidden
    // strip is runtime-only and `top_pinned` governs startup.
    presenter_restore.unwrap_or(current).persisted()
}

fn record_drawer_hint_shown(state: &mut OnboardingState) -> bool {
    if state.drawer_hint_count >= crate::onboarding::DRAWER_HINT_MAX {
        return false;
    }

    state.drawer_hint_count = state.drawer_hint_count.saturating_add(1);
    state.drawer_hint_shown = state.drawer_hint_count >= crate::onboarding::DRAWER_HINT_MAX;
    true
}

fn toolbar_event_blocked_by_modal(input_state: &InputState) -> bool {
    input_state.command_palette_is_engaged()
}

/// The top overflow menu is a plain flyout: any event other than the two menu
/// toggles dismisses it (selecting a dropped tool, an unrelated keybinding, etc.).
fn event_dismisses_top_overflow(event: &ToolbarEvent) -> bool {
    !matches!(
        event,
        ToolbarEvent::ToggleShapePicker(_) | ToolbarEvent::ToggleTopOverflow(_)
    )
}

/// The precise-entry popup dismisses on any toolbar interaction other
/// than its own open/commit/cancel events (mirroring the overflow flyout).
fn event_dismisses_precision_entry(event: &ToolbarEvent) -> bool {
    !matches!(
        event,
        ToolbarEvent::OpenPrecisionEntry(_)
            | ToolbarEvent::CommitPrecisionEntry { .. }
            | ToolbarEvent::CancelPrecisionEntry
    )
}

/// The Shapes popover dismisses on everything the overflow does *except* its own
/// inline options: the Fill checkbox and the polygon-sides stepper live inside
/// the popover, so using them must not close it out from under the pointer.
fn event_dismisses_shape_picker(event: &ToolbarEvent) -> bool {
    event_dismisses_top_overflow(event)
        && !matches!(
            event,
            ToolbarEvent::ToggleFill(_) | ToolbarEvent::NudgePolygonSides(_)
        )
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
            && self.input_state.toolbar_side_pane == crate::ui::toolbar::SidePane::Draw;
        let mut snapshot =
            ToolbarSnapshot::from_input_with_options(&self.input_state, hints, show_drawer_hint);
        populate_session_snapshot(&mut snapshot, self.session.options());
        snapshot.side_viewport_max = self.side_pane_viewport_max(&snapshot);
        snapshot.top_viewport_max = self.top_strip_viewport_max(&snapshot);
        snapshot.top_fade = self.data.top_strip_fade.value();
        snapshot
    }

    /// Width available to the top strip in pre-scale spec units; content
    /// past this degrades into the overflow menu instead of clipping off
    /// the screen. Both inline and layer-shell placement use the pushed top
    /// base X, so budgeting must subtract that same position.
    fn top_strip_viewport_max(&self, snapshot: &ToolbarSnapshot) -> Option<f64> {
        let screen_width = self.surface.width() as f64;
        let scale = if snapshot.toolbar_scale.is_finite() {
            snapshot.toolbar_scale.clamp(0.5, 3.0)
        } else {
            1.0
        };
        let base_x = self.inline_top_base_x(snapshot);
        super::geometry::remaining_top_width(screen_width, base_x, Self::TOP_MARGIN_RIGHT, scale)
    }

    /// Height available to the side palette in pre-scale spec units: the
    /// screen space below the palette's top edge, floored so a pathological
    /// drag offset cannot collapse the pane entirely.
    fn side_pane_viewport_max(&self, snapshot: &ToolbarSnapshot) -> Option<f64> {
        const MIN_SIDE_VIEWPORT: f64 = 180.0;
        let screen_height = self.surface.height() as f64;
        if screen_height <= 0.0 {
            return None;
        }
        let scale = if snapshot.toolbar_scale.is_finite() {
            snapshot.toolbar_scale.clamp(0.5, 3.0)
        } else {
            1.0
        };
        let side_top = Self::SIDE_BASE_MARGIN_TOP + self.data.toolbar_side_offset;
        let available = screen_height - side_top - Self::SIDE_MARGIN_BOTTOM;
        Some((available / scale).max(MIN_SIDE_VIEWPORT))
    }

    /// Applies an incoming toolbar event and schedules redraws as needed.
    pub(in crate::backend::wayland) fn handle_toolbar_event(
        &mut self,
        event: ToolbarEvent,
        conn: Option<&Connection>,
        qh: Option<&QueueHandle<Self>>,
    ) {
        let rebind_requested = self.config.ui.toolbar.rebind_modifier.matches(
            self.input_state.modifiers.ctrl,
            self.input_state.modifiers.shift,
            self.input_state.modifiers.alt,
        );
        // Built-in press resolution: Shift+click on Clear skips the undo
        // toast. (GTK resolves the same upgrade from its own click-time
        // modifier capture before the event reaches the bridge.)
        let event = match event {
            ToolbarEvent::ClearCanvas { instant } => ToolbarEvent::ClearCanvas {
                instant: instant || self.input_state.modifiers.shift,
            },
            other => other,
        };
        self.handle_toolbar_event_with_rebind(event, rebind_requested, conn, qh);
    }

    pub(in crate::backend::wayland) fn handle_toolbar_event_with_rebind(
        &mut self,
        event: ToolbarEvent,
        rebind_requested: bool,
        conn: Option<&Connection>,
        qh: Option<&QueueHandle<Self>>,
    ) {
        // GTK toolbar feedback bypasses the built-in pointer modal gate, so
        // enforce the same rule in the shared event path as well.
        if toolbar_event_blocked_by_modal(&self.input_state) {
            return;
        }
        // A toolbar interaction replaces the modal sampler. Do this before
        // shortcut capture so the capture modal owns subsequent keys.
        self.cancel_eyedropper();
        if rebind_requested
            && let Some(action) = crate::ui::toolbar::model::action_for_event(&event)
        {
            self.input_state.begin_keybinding_capture(action);
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
            return;
        }
        // Toolbar actions win over the modal sampler: cancel without sampling,
        // then apply the requested toolbar event normally.
        if self.input_state.is_precision_entry_open()
            && event_dismisses_precision_entry(&event)
            && self.input_state.cancel_precision_entry()
        {
            self.toolbar.mark_dirty();
        }
        let dismiss_overflow =
            self.input_state.toolbar_top_overflow_open && event_dismisses_top_overflow(&event);
        let dismiss_shapes =
            self.input_state.toolbar_shapes_expanded && event_dismisses_shape_picker(&event);
        if dismiss_overflow || dismiss_shapes {
            if dismiss_overflow {
                self.input_state.toolbar_top_overflow_open = false;
            }
            if dismiss_shapes {
                self.input_state.toolbar_shapes_expanded = false;
            }
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
        }
        if self.handle_toolbar_session_event(&event, conn, qh) {
            return;
        }

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

        #[cfg(feature = "tablet-input")]
        let prev_thickness = self.input_state.current_thickness;
        #[cfg(feature = "tablet-input")]
        let thickness_event = policy.tablet_thickness_sensitive;

        let pane_switch = matches!(event, ToolbarEvent::SetSidePane(_));

        if self.input_state.apply_toolbar_event(event) {
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
            if pane_switch {
                self.reset_side_toolbar_focus();
            }

            #[cfg(feature = "tablet-input")]
            if thickness_event && self.sync_stylus_thickness_cache(prev_thickness) {
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

    #[cfg(feature = "tablet-input")]
    pub(in crate::backend::wayland) fn sync_stylus_thickness_cache(&mut self, prev: f64) -> bool {
        let cur = self.input_state.current_thickness;
        if (cur - prev).abs() <= f64::EPSILON {
            return false;
        }

        self.stylus_base_thickness = Some(cur);
        if self.stylus_tip_down {
            self.stylus_pressure_thickness = Some(cur);
        } else {
            self.stylus_pressure_thickness = None;
        }
        true
    }

    /// Records the maximum stylus thickness seen during the current stroke.
    #[cfg(feature = "tablet-input")]
    pub(in crate::backend::wayland) fn record_stylus_peak(&mut self, thickness: f64) {
        self.stylus_peak_thickness = Some(
            self.stylus_peak_thickness
                .map_or(thickness, |p| p.max(thickness)),
        );
    }

    /// Saves the current toolbar configuration to disk (pinned state, icon mode, section visibility).
    pub(in crate::backend::wayland) fn save_toolbar_pin_config(&mut self) {
        self.config.ui.toolbar.layout_mode = self.input_state.toolbar_layout_mode;
        self.config.ui.toolbar.items = self.input_state.toolbar_items.clone();
        self.config.ui.toolbar.top_pinned = self.input_state.toolbar_top_pinned;
        self.config.ui.toolbar.side_pinned = self.input_state.toolbar_side_pinned;
        self.config.ui.toolbar.top_minimized = persisted_top_minimized_value(
            self.input_state.toolbar_top_minimized,
            self.input_state
                .presenter_restore
                .as_ref()
                .and_then(|restore| restore.toolbar_top_minimized),
        );
        self.config.ui.toolbar.top_display_mode = persisted_top_display_mode_value(
            self.input_state.toolbar_top_display_mode,
            self.input_state
                .presenter_restore
                .as_ref()
                .and_then(|restore| restore.toolbar_top_display_mode),
        );
        self.config.ui.toolbar.side_minimized = self.input_state.toolbar_side_minimized;
        self.config.ui.toolbar.side_active_pane =
            self.input_state.toolbar_side_pane.config_id().to_string();
        // Keep unknown ids (written by newer versions) so a round trip
        // through this build does not drop them.
        let mut collapsed: Vec<String> = self
            .config
            .ui
            .toolbar
            .collapsed_sections
            .iter()
            .filter(|id| crate::ui::toolbar::ToolbarSideSection::from_config_id(id).is_none())
            .cloned()
            .collect();
        collapsed.extend(
            self.input_state
                .toolbar_collapsed_side_sections
                .iter()
                .map(|section| section.config_id().to_string()),
        );
        self.config.ui.toolbar.collapsed_sections = collapsed;
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
mod tests;
