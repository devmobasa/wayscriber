use log::{debug, info};

use crate::backend::wayland::state::{PerfInputSource, WaylandState};
use crate::config::keybindings::{StylusButton, linux};
use crate::input::MouseButton;
use crate::input::state::HelpOverlayPressSource;

fn modal_blocks_stylus_barrel_actions(input_state: &crate::input::InputState) -> bool {
    input_state.modal_owns_pointer_shortcuts()
}

fn stylus_barrel_action(
    input_state: &crate::input::InputState,
    button: u32,
    tablet: &crate::config::TabletInputConfig,
) -> Option<crate::config::Action> {
    let stylus = linux::stylus_button(button)?;
    let trigger = input_state.stylus_trigger(stylus);
    if let Some(action) = input_state.find_trigger_action(&trigger) {
        return Some(action);
    }
    match stylus {
        StylusButton::Primary => tablet.stylus_button.action,
        StylusButton::Secondary => tablet.stylus_button2.action,
    }
}

impl WaylandState {
    /// Queue a tablet motion axis update until the enclosing tablet frame commits.
    pub(super) fn queue_stylus_motion(&mut self, x: f64, y: f64) {
        self.tablet.pending_frame.motion = Some((x, y));
    }

    /// Queue a tablet pressure axis update until the enclosing tablet frame commits.
    pub(super) fn queue_stylus_pressure(&mut self, pressure: u32) {
        self.tablet.pending_frame.pressure = Some(pressure);
    }

    /// Queue logical tip contact until the enclosing tablet frame commits.
    pub(super) fn queue_stylus_down(&mut self) {
        self.tablet.pending_frame.down = true;
    }

    /// Queue logical tip release until the enclosing tablet frame commits.
    pub(super) fn queue_stylus_up(&mut self) {
        self.tablet.pending_frame.up = true;
    }

    /// Queue a tablet tool button press until the enclosing tablet frame commits.
    pub(super) fn queue_stylus_button_press(&mut self, button: u32) {
        self.tablet.pending_frame.button_presses.push(button);
    }

    /// Commit coalesced tablet tool state.
    ///
    /// Invariant: drawing samples are appended only after applying the pressure
    /// update from the same committed tablet frame.
    pub(super) fn commit_pending_stylus_frame(&mut self) {
        let pending = std::mem::take(&mut self.tablet.pending_frame);
        if pending.is_empty() {
            return;
        }
        // Modal ownership is captured at frame entry. A tip-up action may
        // close help during this same frame, but barrel presses queued while
        // help was visible must still not dispatch behind it.
        let modal_blocks_barrel_actions = modal_blocks_stylus_barrel_actions(&self.input_state);

        if let Some(pressure) = pending.pressure {
            self.apply_committed_stylus_pressure(pressure);
        }

        if let Some((x, y)) = pending.motion {
            self.commit_stylus_motion_sample(x, y, pending.pressure.is_some());
        }

        if pending.down {
            self.commit_stylus_down();
        }

        if pending.pressure.is_some()
            && pending.motion.is_none()
            && !pending.down
            && self.tablet.tip_down
        {
            self.commit_stylus_motion_sample_at_current_position(true);
        }

        if pending.up {
            self.commit_stylus_up();
        }

        // Actions like radial menu toggling read the cached pointer position,
        // so button presses run after this frame's motion has been committed.
        if !modal_blocks_barrel_actions {
            for button in pending.button_presses {
                self.dispatch_stylus_button_press(button);
            }
        }
    }

    pub(super) fn current_or_pending_stylus_position(&self) -> (f64, f64) {
        self.tablet
            .pending_frame
            .motion
            .or(self.tablet.last_pos)
            .unwrap_or_else(|| {
                let (x, y) = self.current_mouse();
                (x as f64, y as f64)
            })
    }

    fn apply_committed_stylus_pressure(&mut self, pressure: u32) {
        if pressure == 0 {
            debug!("Stylus pressure reported 0; deferring to peak/base");
            return;
        }

        let first_pressure_sample =
            self.tablet.tip_down && self.tablet.pressure_thickness.is_none();
        let p01 = (pressure as f64) / 65535.0;
        if !crate::input::tablet::try_apply_pressure_to_state(
            p01,
            &mut self.input_state,
            self.tablet.settings,
        ) {
            return;
        }
        if first_pressure_sample {
            self.input_state
                .replace_active_drawing_pressure_samples(self.input_state.current_thickness);
        }
        self.tablet.pressure_thickness = Some(self.input_state.current_thickness);
        self.record_stylus_peak(self.input_state.current_thickness);
    }

    fn commit_stylus_motion_sample(&mut self, x: f64, y: f64, pressure_sample: bool) {
        let previous_hover_cursor_pos = self.stylus_hover_cursor_position();
        self.set_current_mouse(x as i32, y as i32);
        self.tablet.last_pos = Some((x, y));
        let (wx, wy) = self.zoomed_world_coords(x, y);
        self.input_state
            .on_mouse_motion_with_canvas(x.round() as i32, y.round() as i32, wx, wy);
        self.record_perf_input_sample(
            PerfInputSource::Stylus,
            x.round() as i32,
            y.round() as i32,
            wx,
            wy,
            pressure_sample,
        );
        let next_hover_cursor_pos = self.stylus_hover_cursor_position();
        self.mark_stylus_hover_cursor_dirty(previous_hover_cursor_pos, next_hover_cursor_pos);
        if self.tablet.tip_down {
            self.record_stylus_motion_thickness();
        }
    }

    fn commit_stylus_motion_sample_at_current_position(&mut self, pressure_sample: bool) {
        let (x, y) = self.current_stylus_position();
        self.commit_stylus_motion_sample(x, y, pressure_sample);
    }

    fn commit_stylus_down(&mut self) {
        if !self.tablet.on_overlay {
            return;
        }

        if !self.input_state.help_overlay.is_visible() {
            // A new tip-down supersedes any consume-only help ownership left
            // by a sequence whose tip-up was not delivered.
            self.input_state
                .clear_help_overlay_press_for(HelpOverlayPressSource::Stylus);
        }

        if self.input_state.region_is_active() {
            let (x, y) = self.current_stylus_position();
            self.begin_region_selection(crate::input::state::RegionInputSource::Stylus, x, y);
            return;
        }

        if self.input_state.eyedropper_is_active() {
            let (x, y) = self.current_stylus_position();
            self.sample_eyedropper(x, y);
            return;
        }

        if self.input_state.tour.active {
            return;
        }

        // Help owns stylus tip input just as it owns mouse and touch input.
        // Record the press target but do not begin a canvas interaction.
        if self.input_state.help_overlay.is_visible() {
            let (x, y) = self.current_stylus_position();
            self.set_current_mouse(x.round() as i32, y.round() as i32);
            self.input_state.note_help_overlay_press(
                HelpOverlayPressSource::Stylus,
                x.round() as i32,
                y.round() as i32,
            );
            return;
        }

        // Canvas click-away: a pen-down on the canvas with a top popover open
        // (Canvas/Session/Settings) dismisses it and swallows the pen-down,
        // matching the mouse and touch paths — otherwise the pen-down would
        // start a stray stroke instead of closing the popover.
        if self.dismiss_top_toolbar_menus() {
            self.input_state.needs_redraw = true;
            return;
        }

        // Report the pen tip to the input HUD alongside the mouse buttons; the
        // pen is a pointer device, so it gets the same pill chrome.
        self.input_state
            .note_input_hud_mouse("Pen", self.input_state.modifiers);

        let hover_cursor_pos = self.stylus_hover_cursor_position();
        let (x, y) = self.current_stylus_position();
        self.set_current_mouse(x as i32, y as i32);
        self.tablet.tip_down = true;
        self.mark_stylus_hover_cursor_dirty(hover_cursor_pos, None);
        info!(
            "Stylus DOWN at ({}, {})",
            self.current_mouse().0,
            self.current_mouse().1
        );
        let screen_x = self.current_mouse().0;
        let screen_y = self.current_mouse().1;
        let (wx, wy) = self.zoomed_world_coords(x, y);
        self.input_state
            .on_mouse_press_with_canvas(MouseButton::Left, screen_x, screen_y, wx, wy);
        let base_thickness = self.input_state.current_thickness;
        self.tablet.base_thickness = Some(base_thickness);
        self.record_stylus_motion_thickness();
        self.input_state.needs_redraw = true;
    }

    fn commit_stylus_up(&mut self) {
        if !self.tablet.on_overlay {
            return;
        }

        self.tablet.tip_down = false;
        let final_thick = self
            .tablet
            .peak_thickness
            .or(self.tablet.pressure_thickness)
            .or(self.tablet.base_thickness);
        if let Some(thick) = final_thick {
            self.input_state
                .set_pressure_thickness_for_active_tool(thick);
            self.tablet.base_thickness = Some(thick);
        }
        self.tablet.pressure_thickness = None;
        self.tablet.peak_thickness = None;
        info!(
            "Stylus UP at ({}, {})",
            self.current_mouse().0,
            self.current_mouse().1
        );
        let (x, y) = self.current_stylus_position();
        self.set_current_mouse(x as i32, y as i32);
        let screen_x = self.current_mouse().0;
        let screen_y = self.current_mouse().1;
        if self.handle_help_overlay_release(HelpOverlayPressSource::Stylus, screen_x, screen_y) {
            let hover_cursor_pos = self.stylus_hover_cursor_position();
            self.mark_stylus_hover_cursor_dirty(None, hover_cursor_pos);
            self.input_state.needs_redraw = true;
            return;
        }
        let (wx, wy) = self.zoomed_world_coords(x, y);
        self.input_state.on_mouse_release_with_canvas(
            MouseButton::Left,
            screen_x,
            screen_y,
            wx,
            wy,
        );
        let hover_cursor_pos = self.stylus_hover_cursor_position();
        self.mark_stylus_hover_cursor_dirty(None, hover_cursor_pos);
        self.input_state.needs_redraw = true;
    }

    fn current_stylus_position(&self) -> (f64, f64) {
        self.current_or_pending_stylus_position()
    }

    /// Dispatch the configured action for a stylus barrel button press.
    fn dispatch_stylus_button_press(&mut self, button: u32) {
        if let Some(action) = stylus_barrel_action(&self.input_state, button, &self.config.tablet) {
            debug!("Stylus button {}: dispatching {:?}", button, action);
            self.input_state.clear_pending_sequence();
            self.dispatch_input_action(action);
        } else if linux::stylus_button(button).is_none() {
            debug!("Ignoring unknown stylus button {}", button);
        }
    }

    fn record_stylus_motion_thickness(&mut self) {
        if self.tablet.settings.enabled
            && self.tablet.settings.pressure_enabled
            && self.tablet.pressure_thickness.is_none()
        {
            return;
        }

        self.tablet.pressure_thickness = Some(self.input_state.current_thickness);
        self.record_stylus_peak(self.input_state.current_thickness);
    }
}

#[cfg(test)]
mod tests {
    use super::{modal_blocks_stylus_barrel_actions, stylus_barrel_action};
    use crate::input::state::test_support::make_test_input_state;

    #[test]
    fn help_and_tour_block_stylus_barrel_actions() {
        let mut state = make_test_input_state();
        assert!(!modal_blocks_stylus_barrel_actions(&state));

        state.toggle_help_overlay();
        assert!(modal_blocks_stylus_barrel_actions(&state));

        state.toggle_help_overlay();
        state.tour.active = true;
        assert!(modal_blocks_stylus_barrel_actions(&state));
    }

    /// Both screen-region modals swallow pointer and keyboard input, so a
    /// barrel button must not run its bound action on the canvas behind them —
    /// including while they are still waiting on a capture.
    #[test]
    fn screen_region_modals_block_stylus_barrel_actions() {
        use crate::input::state::{EyedropperCaptureSource, RegionPurposeTag, ScreenCaptureSource};

        let mut state = make_test_input_state();
        assert!(!modal_blocks_stylus_barrel_actions(&state));

        state.set_region_pending_capture(RegionPurposeTag::Ocr, 1, ScreenCaptureSource::Frozen);
        assert!(modal_blocks_stylus_barrel_actions(&state));
        state.activate_region(RegionPurposeTag::Ocr, 1);
        assert!(modal_blocks_stylus_barrel_actions(&state));
        state.cancel_region_ui_only();
        assert!(!modal_blocks_stylus_barrel_actions(&state));

        state.set_eyedropper_pending_capture(EyedropperCaptureSource::Frozen);
        assert!(modal_blocks_stylus_barrel_actions(&state));
        state.activate_eyedropper(Some(1));
        assert!(modal_blocks_stylus_barrel_actions(&state));
        state.cancel_eyedropper();
        assert!(!modal_blocks_stylus_barrel_actions(&state));
    }

    #[test]
    fn canonical_stylus_shortcut_wins_over_legacy_tablet_binding() {
        use crate::config::keybindings::linux;
        use crate::config::{Action, KeybindingsConfig};

        let mut keybindings = KeybindingsConfig::default();
        keybindings.core.undo = vec!["StylusPrimary".to_string()];
        let action_map = keybindings.build_action_map().expect("map");
        let action_bindings = keybindings.build_action_bindings().expect("bindings");
        let mut state = crate::input::state::test_support::make_test_input_state();
        state.set_keybinding_maps(action_map, action_bindings);

        let mut tablet = crate::config::TabletInputConfig::default();
        tablet.stylus_button.action = Some(Action::ToggleRadialMenu);

        assert_eq!(
            stylus_barrel_action(&state, linux::BTN_STYLUS, &tablet),
            Some(Action::Undo)
        );
        tablet.stylus_button2.action = Some(Action::Redo);
        assert_eq!(
            stylus_barrel_action(&state, linux::BTN_STYLUS2, &tablet),
            Some(Action::Redo)
        );
    }

    #[test]
    fn unbound_stylus_falls_back_to_legacy_tablet_action() {
        use crate::config::Action;
        use crate::config::keybindings::linux;

        let state = crate::input::state::test_support::make_test_input_state();
        let mut tablet = crate::config::TabletInputConfig::default();
        tablet.stylus_button.action = Some(Action::ToggleRadialMenu);
        tablet.stylus_button2.action = None;

        assert_eq!(
            stylus_barrel_action(&state, linux::BTN_STYLUS, &tablet),
            Some(Action::ToggleRadialMenu)
        );
        assert_eq!(
            stylus_barrel_action(&state, linux::BTN_STYLUS2, &tablet),
            None
        );
        assert_eq!(stylus_barrel_action(&state, 0, &tablet), None);
    }
}
