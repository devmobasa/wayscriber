use log::{debug, info};

use crate::backend::wayland::state::{
    ContactOwner, PerfInputSource, StylusDownAdmission, WaylandState,
};
use crate::config::keybindings::{StylusButton, linux};
use crate::input::state::HelpOverlayPressSource;
use crate::input::{DrawingState, MouseButton};

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
    /// The real press router admits a contact before pressure mutates it. The
    /// first drawing sample is then replaced with this frame's pressure.
    pub(super) fn commit_pending_stylus_frame(&mut self) {
        let pending = std::mem::take(&mut self.tablet.pending_frame);
        if pending.is_empty() {
            return;
        }
        // Modal ownership is captured at frame entry. A tip-up action may
        // close help during this same frame, but barrel presses queued while
        // help was visible must still not dispatch behind it.
        let modal_blocks_barrel_actions = modal_blocks_stylus_barrel_actions(&self.input_state);

        if pending.down
            && let Some((x, y)) = pending.motion
        {
            self.tablet.last_pos = Some((x, y));
            self.pointer
                .set_position((x.round() as i32, y.round() as i32));
        }
        let canvas_down = pending.down && self.prepare_stylus_down();

        if let Some(pressure) = pending.pressure
            && !pending.down
        {
            self.tablet.apply_canvas_pressure(
                self.render.text_measurer(),
                &mut self.input_state,
                pressure,
            );
        }

        if let Some((x, y)) = pending.motion
            && (!pending.down || canvas_down)
        {
            self.commit_stylus_motion_sample(x, y, pending.pressure.is_some());
        }

        if canvas_down {
            self.commit_stylus_canvas_down();
            if let Some(pressure) = pending.pressure {
                self.tablet.apply_canvas_pressure(
                    self.render.text_measurer(),
                    &mut self.input_state,
                    pressure,
                );
            }
        }

        if pending.pressure.is_some()
            && pending.motion.is_none()
            && !pending.down
            && self.tablet.is_canvas_gesture()
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
            .current_or_pending_position(self.pointer.position())
    }

    fn commit_stylus_motion_sample(&mut self, x: f64, y: f64, pressure_sample: bool) {
        let previous_hover_cursor_pos = self.stylus_hover_cursor_position();
        self.pointer.set_position((x as i32, y as i32));
        self.tablet.last_pos = Some((x, y));
        let (wx, wy) = self.zoomed_world_coords(x, y);
        let (canvas_x, canvas_y) = self.canvas_world_coords_precise(x, y);
        self.input_state.on_mouse_motion_with_canvas_and_resources(
            crate::input::state::InputTextResources {
                measurer: self.render.text_measurer(),
                ui_engine: self.render.ui_text(),
            },
            x.round() as i32,
            y.round() as i32,
            canvas_x,
            canvas_y,
        );
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
        if self.tablet.is_canvas_gesture() {
            self.record_stylus_motion_thickness();
        }
    }

    fn commit_stylus_motion_sample_at_current_position(&mut self, pressure_sample: bool) {
        let (x, y) = self.current_stylus_position();
        self.commit_stylus_motion_sample(x, y, pressure_sample);
    }

    fn prepare_stylus_down(&mut self) -> bool {
        if !self.tablet.on_overlay {
            return false;
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
            return false;
        }

        if self.input_state.eyedropper_is_active() {
            let (x, y) = self.current_stylus_position();
            self.sample_eyedropper(x, y);
            return false;
        }

        // Help owns stylus tip input just as it owns mouse and touch input.
        // Record the press target but do not begin a canvas interaction.
        if self.input_state.help_overlay.is_visible() {
            let (x, y) = self.current_stylus_position();
            self.pointer
                .set_position((x.round() as i32, y.round() as i32));
            self.input_state.note_help_overlay_press(
                HelpOverlayPressSource::Stylus,
                x.round() as i32,
                y.round() as i32,
            );
            return false;
        }

        let position = self.current_stylus_position();
        let card_visible = self.first_run_onboarding_card_visible();
        match self.tablet.prepare_chrome_down(
            &mut self.input_state,
            &mut self.onboarding_card,
            position,
            card_visible,
        ) {
            StylusDownAdmission::Canvas => {}
            StylusDownAdmission::Onboarding | StylusDownAdmission::Toast => return false,
            StylusDownAdmission::Popover => {
                if self.toolbar_chrome.inline_toolbars() {
                    self.mark_inline_toolbar_full_damage();
                } else {
                    self.toolbar.mark_dirty();
                }
                self.input_state.needs_redraw = true;
                return false;
            }
        }
        true
    }

    fn commit_stylus_canvas_down(&mut self) {
        // Report the pen tip to the input HUD alongside the mouse buttons; the
        // pen is a pointer device, so it gets the same pill chrome.
        self.input_state
            .note_input_hud_mouse("Pen", self.input_state.modifiers);

        let hover_cursor_pos = self.stylus_hover_cursor_position();
        let (x, y) = self.current_stylus_position();
        self.pointer.set_position((x as i32, y as i32));
        self.tablet.bind_tip(ContactOwner::Canvas);
        self.mark_stylus_hover_cursor_dirty(hover_cursor_pos, None);
        info!(
            "Stylus DOWN at ({}, {})",
            self.pointer.position().0,
            self.pointer.position().1
        );
        let screen_x = self.pointer.position().0;
        let screen_y = self.pointer.position().1;
        let (canvas_x, canvas_y) = self.canvas_world_coords_precise(x, y);
        self.input_state.on_mouse_press_with_canvas_and_resources(
            crate::input::state::InputTextResources {
                measurer: self.render.text_measurer(),
                ui_engine: self.render.ui_text(),
            },
            MouseButton::Left,
            screen_x,
            screen_y,
            canvas_x,
            canvas_y,
        );
        self.record_stylus_motion_thickness();
        self.input_state.needs_redraw = true;
    }

    fn commit_stylus_up(&mut self) {
        if !self.tablet.on_overlay {
            return;
        }

        if let Some(press) = self.onboarding_card.take_stylus_press() {
            // The tap never became a contact, so there is no stroke to end
            // and no pressure thickness to commit.
            self.tablet.lift_tip();
            self.tablet.pressure_thickness = None;
            self.tablet.peak_thickness = None;
            let (x, y) = self.current_stylus_position();
            self.tablet
                .set_over_inline_strip(self.inline_toolbar_contains((x, y)));
            self.release_onboarding_card_press(press, x, y);
            self.input_state.needs_redraw = true;
            return;
        }

        if let Some(pressed) = self.tablet.toast_press.take() {
            // Like a card tap, a toast tap never became a stroke.
            self.tablet.lift_tip();
            self.tablet.pressure_thickness = None;
            self.tablet.peak_thickness = None;

            let (x, y) = self.current_stylus_position();
            let (hit, action) =
                self.input_state
                    .resolve_toast_release(pressed, x.round() as i32, y.round() as i32);
            if hit && let Some(command) = action {
                self.handle_toast_command(command);
            }
            return;
        }

        self.tablet.lift_tip();
        // Only a stroke keeps the thickness its pressure reached. A contact
        // that did not draw, such as a size-ring drag or a tap that closed a
        // menu, leaves the thickness as that contact set it.
        let drawing = matches!(self.input_state.state, DrawingState::Drawing { .. });
        let final_thick = self
            .tablet
            .peak_thickness
            .or(self.tablet.pressure_thickness);
        if let Some(thick) = final_thick.filter(|_| drawing) {
            self.input_state
                .set_pressure_thickness_for_active_tool_with(self.render.text_measurer(), thick);
        }
        self.tablet.pressure_thickness = None;
        self.tablet.peak_thickness = None;
        info!(
            "Stylus UP at ({}, {})",
            self.pointer.position().0,
            self.pointer.position().1
        );
        let (x, y) = self.current_stylus_position();
        self.pointer.set_position((x as i32, y as i32));
        self.tablet
            .set_over_inline_strip(self.inline_toolbar_contains((x, y)));
        let screen_x = self.pointer.position().0;
        let screen_y = self.pointer.position().1;
        if self.handle_help_overlay_release(HelpOverlayPressSource::Stylus, screen_x, screen_y) {
            let hover_cursor_pos = self.stylus_hover_cursor_position();
            self.mark_stylus_hover_cursor_dirty(None, hover_cursor_pos);
            self.input_state.needs_redraw = true;
            return;
        }
        let (canvas_x, canvas_y) = self.canvas_world_coords_precise(x, y);
        self.input_state.on_mouse_release_with_canvas_and_resources(
            crate::input::state::InputTextResources {
                measurer: self.render.text_measurer(),
                ui_engine: self.render.ui_text(),
            },
            MouseButton::Left,
            screen_x,
            screen_y,
            canvas_x,
            canvas_y,
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

        self.tablet.pressure_thickness = Some(self.input_state.style.current_thickness);
        self.record_stylus_peak(self.input_state.style.current_thickness);
    }
}

#[cfg(test)]
mod tests;
