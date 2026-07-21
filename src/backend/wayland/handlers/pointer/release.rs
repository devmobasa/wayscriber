use log::debug;
use smithay_client_toolkit::seat::pointer::{BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, PointerEvent};

use crate::backend::wayland::state::drag_log;
use crate::input::state::HelpOverlayPressSource;
use crate::input::{HelpOverlayReleaseOutcome, MouseButton};
use crate::ui::ZoomChipPress;

use super::*;

impl WaylandState {
    pub(super) fn handle_pointer_release(
        &mut self,
        event: &PointerEvent,
        on_toolbar: bool,
        inline_active: bool,
        button: u32,
    ) {
        if self.input_state.eyedropper_is_active() {
            return;
        }

        // Swallow releases after modal clicks (e.g., palette dismiss)
        if self.take_suppress_next_release() {
            self.set_pending_toast_press(None);
            self.set_pending_status_hud_press(false);
            self.set_pending_zoom_chip_press(ZoomChipPress::None);
            return;
        }

        // Resolve help ownership even after the overlay has closed: a press
        // swallowed by help must not leak its release into a newly opened
        // popup. Conversely, a press that preceded help has no owner and falls
        // through to finish its original gesture.
        let source = HelpOverlayPressSource::Pointer(button);
        let help_owned_release = if button == BTN_LEFT {
            let screen_position = if on_toolbar {
                self.toolbar_surface_screen_coords(&event.surface, event.position)
            } else {
                Some(event.position)
            };
            match screen_position {
                Some((sx, sy)) => {
                    self.handle_help_overlay_release(source, sx.round() as i32, sy.round() as i32)
                }
                None => self.input_state.clear_help_overlay_press_for(source),
            }
        } else {
            // Non-left help presses are modal-owned but never resolve rows.
            self.input_state.clear_help_overlay_press_for(source)
        };
        if help_owned_release {
            self.set_pending_toast_press(None);
            self.set_pending_status_hud_press(false);
            self.set_pending_zoom_chip_press(ZoomChipPress::None);
            return;
        }

        // Block pointer input when modal overlays are active
        if self.input_state.command_palette_open || self.input_state.tour_active {
            // For command palette, press handles the click - release is a no-op
            self.set_pending_toast_press(None);
            self.set_pending_status_hud_press(false);
            self.set_pending_zoom_chip_press(ZoomChipPress::None);
            return;
        }

        if button == BTN_LEFT
            && let Some(pressed) = self.take_pending_toast_press()
        {
            let screen_position = if on_toolbar {
                self.toolbar_surface_screen_coords(&event.surface, event.position)
            } else {
                Some(event.position)
            };
            if let Some((screen_x, screen_y)) = screen_position {
                let (hit, action) = self.input_state.resolve_toast_release(
                    pressed,
                    screen_x.round() as i32,
                    screen_y.round() as i32,
                );
                if hit && let Some(action) = action {
                    self.dispatch_input_action(action);
                }
            }
            return;
        }

        if button == BTN_LEFT && self.take_pending_status_hud_press() {
            let screen_position = if on_toolbar {
                self.toolbar_surface_screen_coords(&event.surface, event.position)
            } else {
                Some(event.position)
            };
            if let Some((screen_x, screen_y)) = screen_position {
                // Segment activations open their surfaces directly; help
                // comes back as an action to dispatch.
                let (hit, action) = self
                    .input_state
                    .check_status_hud_click(screen_x.round() as i32, screen_y.round() as i32);
                if hit && let Some(action) = action {
                    self.dispatch_input_action(action);
                }
            }
            return;
        }

        if button == BTN_LEFT {
            let pressed = self.take_pending_zoom_chip_press();
            if pressed.is_pending() {
                // Any pending chip press (`Passive` or `Button`) consumes its
                // release here. Only a `Button` resolves to a zoom action; the
                // action fires only when the release lands on the SAME button.
                // dispatch_input_action drains the resulting pending zoom action.
                if let ZoomChipPress::Button(kind) = pressed {
                    let screen_position = if on_toolbar {
                        self.toolbar_surface_screen_coords(&event.surface, event.position)
                    } else {
                        Some(event.position)
                    };
                    if let Some((screen_x, screen_y)) = screen_position {
                        let (_, action) = self.input_state.check_zoom_chip_click(
                            kind,
                            screen_x.round() as i32,
                            screen_y.round() as i32,
                        );
                        if let Some(action) = action {
                            self.dispatch_input_action(action);
                        }
                    }
                }
                return;
            }
        }

        if debug_toolbar_drag_logging_enabled() {
            debug!(
                "pointer release: button={}, on_toolbar={}, inline_active={}, drag_active={}, toolbar_dragging={}, pointer_over_toolbar={}",
                button,
                on_toolbar,
                inline_active,
                self.is_move_dragging(),
                self.toolbar_dragging(),
                self.pointer_over_toolbar()
            );
        }
        // An open radial menu owns pointer releases everywhere on screen: a
        // press-flick-release whose release lands over a toolbar region must
        // still commit (or cancel) instead of being swallowed by the toolbar
        // gates below. The radial release router consumes every button while
        // the menu is open, so nothing leaks through to canvas handling.
        if self.input_state.is_radial_menu_open()
            && !self.is_move_dragging()
            && !self.toolbar_dragging()
        {
            let mb = match button {
                BTN_LEFT => Some(MouseButton::Left),
                BTN_MIDDLE => Some(MouseButton::Middle),
                BTN_RIGHT => Some(MouseButton::Right),
                _ => None,
            };
            let screen_position = if on_toolbar {
                self.toolbar_surface_screen_coords(&event.surface, event.position)
            } else {
                Some(event.position)
            };
            if let (Some(mb), Some((sx, sy))) = (mb, screen_position) {
                let screen_x = sx.round() as i32;
                let screen_y = sy.round() as i32;
                let (wx, wy) = self.zoomed_world_coords(sx, sy);
                self.input_state
                    .on_mouse_release_with_canvas(mb, screen_x, screen_y, wx, wy);
                self.input_state.needs_redraw = true;
                return;
            }
        }
        if inline_active {
            if button == BTN_LEFT && self.inline_toolbar_release(event.position) {
                drag_log(format!(
                    "pointer release: inline handled, pos=({:.3}, {:.3}), drag_active={}, pointer_over_toolbar={}",
                    event.position.0,
                    event.position.1,
                    self.is_move_dragging(),
                    self.pointer_over_toolbar()
                ));
                self.unlock_pointer();
                return;
            }
            if self.pointer_over_toolbar() || self.toolbar_dragging() {
                drag_log(format!(
                    "pointer release: inline end drag, pos=({:.3}, {:.3}), drag_active={}, pointer_over_toolbar={}",
                    event.position.0,
                    event.position.1,
                    self.is_move_dragging(),
                    self.pointer_over_toolbar()
                ));
                self.end_toolbar_move_drag();
                self.unlock_pointer();
                return;
            }
        }
        if on_toolbar || self.pointer_over_toolbar() {
            if button == BTN_LEFT {
                self.set_toolbar_dragging(false);
            }
            drag_log(format!(
                "pointer release: toolbar end drag, pos=({:.3}, {:.3}), drag_active={}, pointer_over_toolbar={}",
                event.position.0,
                event.position.1,
                self.is_move_dragging(),
                self.pointer_over_toolbar()
            ));
            self.end_toolbar_move_drag();
            self.unlock_pointer();
            return;
        }
        // End move drag if released on the main surface
        if button == BTN_LEFT && self.is_move_dragging() {
            self.set_toolbar_dragging(false);
            drag_log(format!(
                "pointer release: main surface end drag, pos=({:.3}, {:.3})",
                event.position.0, event.position.1
            ));
            self.end_toolbar_move_drag();
            self.unlock_pointer();
            return;
        }
        debug!("Button {} released", button);
        if self.zoom.active && button == BTN_MIDDLE {
            if self.zoom.panning {
                self.zoom.stop_pan();
                self.input_state.dirty_tracker.mark_full();
                self.input_state.needs_redraw = true;
            }
            return;
        }
        if button == BTN_LEFT && self.board_panning_active() {
            self.stop_board_pan();
            self.input_state.needs_redraw = true;
            return;
        }

        let mb = match button {
            BTN_LEFT => MouseButton::Left,
            BTN_MIDDLE => MouseButton::Middle,
            BTN_RIGHT => MouseButton::Right,
            _ => return,
        };

        let screen_x = event.position.0.round() as i32;
        let screen_y = event.position.1.round() as i32;
        let (wx, wy) = self.zoomed_world_coords(event.position.0, event.position.1);
        self.input_state
            .on_mouse_release_with_canvas(mb, screen_x, screen_y, wx, wy);
        self.input_state.needs_redraw = true;
    }

    /// Resolve a left release inside the open help overlay against the target
    /// recorded on press, enforcing the same-target contract: run a row only
    /// when press and release land on the SAME row, dismiss only when both fall
    /// outside the box, and otherwise leave the overlay untouched.
    pub(in crate::backend::wayland) fn handle_help_overlay_release(
        &mut self,
        source: HelpOverlayPressSource,
        screen_x: i32,
        screen_y: i32,
    ) -> bool {
        let Some(outcome) = self
            .input_state
            .resolve_help_overlay_release(source, screen_x, screen_y)
        else {
            return false;
        };
        match outcome {
            HelpOverlayReleaseOutcome::Run(action) => {
                self.dispatch_input_action(action);
                // Most actions leave the overlay up; close it so the effect is
                // visible. Actions that already closed it (ToggleHelp,
                // ReplayTour) leave `show_help` false, so this is a no-op.
                if self.input_state.show_help {
                    self.input_state.close_help_overlay();
                }
                self.input_state.needs_redraw = true;
            }
            HelpOverlayReleaseOutcome::Dismiss => {
                self.input_state.close_help_overlay();
            }
            HelpOverlayReleaseOutcome::None => {}
        }
        true
    }
}
