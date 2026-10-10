use log::debug;
use smithay_client_toolkit::seat::pointer::{BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, PointerEvent};
use wayland_client::QueueHandle;

use crate::backend::wayland::state::{ContactOwner, RegionReviewPress, drag_log};
use crate::backend::wayland::toolbar_intent::intent_to_event;
use crate::input::MouseButton;
use crate::input::state::HelpOverlayPressSource;
use crate::ui::toolbar::ToolbarEvent;

use super::*;

#[cfg(test)]
fn review_action_suppresses_next_release(action: crate::ui::RegionAction) -> bool {
    action.is_terminal()
}

impl WaylandState {
    pub(super) fn handle_pointer_press(
        &mut self,
        conn: &wayland_client::Connection,
        qh: &QueueHandle<Self>,
        event: &PointerEvent,
        routed: RoutedInput,
        button: u32,
    ) {
        self.pointer.reconcile_contacts(
            &self.input_state,
            self.zoom.panning,
            self.toolbar_drag.is_moving() || self.toolbar_drag.item_dragging(),
        );

        let on_toolbar = routed.surface == InputSurface::Toolbar;
        let inline_active = routed.inline_toolbars;
        // Report the physical button to the input HUD before any modal or
        // toolbar routing consumes it. GTK toolbar surfaces are separate
        // windows and never reach this handler, so their clicks only show in
        // system mode (documented in docs/CONFIG.md).
        if self.input_state.input_hud_enabled() {
            self.input_state
                .note_input_hud_mouse(&input_hud_button_label(button), self.input_state.modifiers);
        }

        // A finished scan card is transient chrome: the next interaction of any
        // kind takes it away rather than making the user wait it out.
        self.input_state.dismiss_ocr_scan_result();

        let help_press_source = HelpOverlayPressSource::Pointer(button);
        if !self.input_state.help_overlay.is_visible() {
            // A new press proves any older help-owned sequence for this button
            // has ended, even if its release was lost with a surface/device.
            self.input_state
                .clear_help_overlay_press_for(help_press_source);
        }

        self.pointer.take_contact(button, &self.input_state);

        if routed.surface == InputSurface::Foreign {
            return;
        }

        if self.handle_region_pointer_press(event, on_toolbar, button) {
            return;
        }

        if self.handle_eyedropper_pointer_press(event, on_toolbar, button) {
            return;
        }

        if self.handle_modal_pointer_press(event, routed, button, help_press_source) {
            return;
        }

        if self.handle_onboarding_card_pointer_press(routed, button) {
            return;
        }

        if !self.input_state.modal_owns_pointer_shortcuts()
            && self.try_dispatch_pointer_shortcut(button)
        {
            return;
        }

        if self.route_toolbar_pointer_press(conn, qh, event, button, on_toolbar, inline_active) {
            return;
        }

        if button == BTN_LEFT
            && self.press_overlay_chrome(
                event.position.0.round() as i32,
                event.position.1.round() as i32,
            )
        {
            return;
        }

        debug!(
            "Button {} pressed at ({}, {})",
            button, event.position.0, event.position.1
        );
        if self.start_pointer_pan(event, button) {
            return;
        }

        let mb = match button {
            BTN_LEFT => MouseButton::Left,
            BTN_MIDDLE => MouseButton::Middle,
            BTN_RIGHT => MouseButton::Right,
            _ => return,
        };

        if inline_active {
            self.clear_inline_contact_hover();
        }

        let screen_x = event.position.0.round() as i32;
        let screen_y = event.position.1.round() as i32;
        let (wx, wy) = self.canvas_world_coords_precise(event.position.0, event.position.1);
        self.pointer.route_canvas_press(
            &mut self.input_state,
            crate::input::state::InputTextResources {
                measurer: self.render.text_measurer(),
                ui_engine: self.render.ui_text(),
            },
            button,
            mb,
            (screen_x, screen_y),
            (wx, wy),
        );
        self.pointer.reconcile_contacts(
            &self.input_state,
            self.zoom.panning,
            self.toolbar_drag.is_moving() || self.toolbar_drag.item_dragging(),
        );
        self.input_state.needs_redraw = true;
    }

    /// A middle press pans an unlocked zoom, and a left press with the pan key
    /// held pans the board.
    fn start_pointer_pan(&mut self, event: &PointerEvent, button: u32) -> bool {
        if self.zoom.active && button == BTN_MIDDLE && !self.zoom.locked {
            self.zoom.start_pan(event.position.0, event.position.1);
            self.pointer.bind_contact(button, ContactOwner::ZoomPan);
            self.input_state.dirty_tracker.mark_full();
            self.input_state.needs_redraw = true;
            return true;
        }
        if button == BTN_LEFT && self.pointer.board_pan_key_held() && self.can_start_board_pan() {
            self.pointer
                .start_board_pan((event.position.0, event.position.1));
            self.pointer.bind_contact(button, ContactOwner::BoardPan);
            self.input_state.needs_redraw = true;
            return true;
        }
        false
    }

    fn handle_region_pointer_press(
        &mut self,
        event: &PointerEvent,
        on_toolbar: bool,
        button: u32,
    ) -> bool {
        if !self.input_state.region_is_active() {
            return false;
        }
        if on_toolbar || self.toolbar_chrome.pointer_over_toolbar() {
            // A toolbar interaction ends the region first, then runs normally;
            // the click never lands on the selector.
            self.cancel_region_for_toolbar_interaction();
            return false;
        }
        match button {
            BTN_LEFT => {
                match self.consume_region_review_press(RegionInputSource::Pointer, event.position) {
                    RegionReviewPress::NotReview | RegionReviewPress::Fallthrough => {
                        self.begin_region_selection(
                            RegionInputSource::Pointer,
                            event.position.0,
                            event.position.1,
                        );
                    }
                    RegionReviewPress::Consumed { suppress_release } => {
                        if suppress_release {
                            self.pointer.suppress_release(RegionInputSource::Pointer);
                        }
                    }
                }
            }
            BTN_RIGHT => {
                self.cancel_active_region_selector();
                self.pointer.suppress_release(RegionInputSource::Pointer);
            }
            _ => {}
        }
        true
    }

    fn handle_eyedropper_pointer_press(
        &mut self,
        event: &PointerEvent,
        on_toolbar: bool,
        button: u32,
    ) -> bool {
        if !self.input_state.eyedropper_is_active() {
            return false;
        }
        if on_toolbar || self.toolbar_chrome.pointer_over_toolbar() {
            self.cancel_eyedropper();
            return false;
        }
        match button {
            BTN_LEFT => {
                self.sample_eyedropper(event.position.0, event.position.1);
                self.pointer.suppress_release(RegionInputSource::Pointer);
            }
            BTN_RIGHT => {
                self.cancel_eyedropper();
                self.pointer.suppress_release(RegionInputSource::Pointer);
            }
            _ => {}
        }
        true
    }

    fn handle_modal_pointer_press(
        &mut self,
        event: &PointerEvent,
        routed: RoutedInput,
        button: u32,
        help_press_source: HelpOverlayPressSource,
    ) -> bool {
        // Help is modal: remember the target so release can require the same row.
        if self.input_state.help_overlay.is_visible() {
            match routed.screen {
                Some((sx, sy)) => self.input_state.note_help_overlay_press(
                    help_press_source,
                    sx.round() as i32,
                    sy.round() as i32,
                ),
                None => {
                    self.input_state
                        .clear_help_overlay_press_for(help_press_source);
                }
            }
            return true;
        }
        if !self.input_state.command_palette_is_engaged() {
            return false;
        }
        if button == BTN_LEFT {
            let handled = self
                .input_state
                .handle_command_palette_click_with_resources(
                    crate::input::state::InputTextResources {
                        measurer: self.render.text_measurer(),
                        ui_engine: self.render.ui_text(),
                    },
                    event.position.0 as i32,
                    event.position.1 as i32,
                    self.surface.width(),
                    self.surface.height(),
                );
            if handled {
                self.pointer.suppress_release(RegionInputSource::Pointer);
            }
        }
        true
    }

    /// The onboarding card paints above the canvas and the inline bars, so it
    /// owns every button pressed on it: nothing draws, no pointer shortcut
    /// fires, and a click never counts as the stroke the card asks for.
    fn handle_onboarding_card_pointer_press(&mut self, routed: RoutedInput, button: u32) -> bool {
        if routed.surface != InputSurface::Canvas {
            return false;
        }
        let Some(press) = routed
            .screen
            .and_then(|(x, y)| self.onboarding_card_press_at(x, y))
        else {
            return false;
        };

        self.pointer.clear_chrome_press();
        if button == BTN_LEFT {
            self.pointer.arm_onboarding_card_press(press);
        } else {
            self.pointer.suppress_release(RegionInputSource::Pointer);
        }
        true
    }

    pub(in crate::backend::wayland) fn press_overlay_chrome(
        &mut self,
        screen_x: i32,
        screen_y: i32,
    ) -> bool {
        self.pointer.clear_chrome_press();
        // The card paints above toasts; touch reaches it only through here.
        if let Some(press) = self.onboarding_card_press_at(f64::from(screen_x), f64::from(screen_y))
        {
            return self.pointer.arm_onboarding_card_press(press);
        }
        if let Some(pressed) = self.input_state.toast_press_at(screen_x, screen_y) {
            return self.pointer.arm_toast_press(pressed);
        }
        if self.input_state.status_hud_contains(screen_x, screen_y) {
            return self.pointer.arm_status_hud_press();
        }
        if !self.input_state.zoom_chip_contains(screen_x, screen_y) {
            return false;
        }
        self.pointer
            .arm_zoom_chip_press(self.input_state.zoom_chip_press_at(screen_x, screen_y))
    }

    fn try_dispatch_pointer_shortcut(&mut self, button: u32) -> bool {
        let Some(pointer) = crate::config::keybindings::linux::pointer_button(button) else {
            return false;
        };
        if self.dispatch_pointer_shortcut(pointer) {
            self.input_state.consume_pointer_shortcut_button(button);
            true
        } else {
            false
        }
    }

    pub(in crate::backend::wayland) fn try_dispatch_gdk_pointer_shortcut(
        &mut self,
        button: u32,
        ctrl: bool,
        shift: bool,
        alt: bool,
        logo: bool,
    ) -> bool {
        let Some(pointer) = crate::config::keybindings::gdk::pointer_button(button) else {
            return false;
        };
        self.dispatch_pointer_trigger(crate::config::PointerTrigger {
            button: pointer,
            ctrl,
            shift,
            alt,
            logo,
        })
    }

    fn dispatch_pointer_shortcut(&mut self, pointer: crate::config::PointerButton) -> bool {
        match self.input_state.pointer_trigger(pointer) {
            crate::config::ShortcutTrigger::Pointer(trigger) => {
                self.dispatch_pointer_trigger(trigger)
            }
            _ => false,
        }
    }

    fn dispatch_pointer_trigger(&mut self, trigger: crate::config::PointerTrigger) -> bool {
        let shortcut = crate::config::ShortcutTrigger::Pointer(trigger);
        let Some(action) = self.input_state.find_trigger_action(&shortcut) else {
            return false;
        };
        self.input_state.clear_pending_sequence();
        debug!("Pointer shortcut {shortcut}: dispatching {action:?}");
        self.dispatch_input_action(action);
        true
    }
}

/// Input HUD label for a raw pointer button code. The three primary buttons
/// get their spoken names; auxiliary buttons use the same semantic names as
/// the shortcut parser; anything else reports its evdev code.
fn input_hud_button_label(button: u32) -> String {
    match button {
        BTN_LEFT => "Click".to_string(),
        BTN_RIGHT => "Right Click".to_string(),
        BTN_MIDDLE => "Middle Click".to_string(),
        other => crate::config::keybindings::linux::pointer_button(other)
            .map(|button| button.name())
            .unwrap_or_else(|| format!("Button {other}")),
    }
}

mod toolbar;

#[cfg(test)]
mod tests;
