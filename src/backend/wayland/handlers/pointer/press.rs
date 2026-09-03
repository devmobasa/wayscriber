use log::debug;
use smithay_client_toolkit::seat::pointer::{BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, PointerEvent};
use wayland_client::QueueHandle;

use crate::backend::wayland::state::{RegionReviewPress, drag_log};
use crate::backend::wayland::toolbar_intent::intent_to_event;
use crate::input::MouseButton;
use crate::input::state::HelpOverlayPressSource;
use crate::ui::ZoomChipPress;
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
        on_toolbar: bool,
        inline_active: bool,
        button: u32,
    ) {
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

        if self.handle_region_pointer_press(event, on_toolbar, button) {
            return;
        }

        if self.handle_eyedropper_pointer_press(event, on_toolbar, button) {
            return;
        }

        if self.handle_modal_pointer_press(event, on_toolbar, button, help_press_source) {
            return;
        }

        if !self.input_state.modal_owns_pointer_shortcuts()
            && self.try_dispatch_pointer_shortcut(button)
        {
            return;
        }

        if debug_toolbar_drag_logging_enabled() {
            debug!(
                "pointer press: button={}, on_toolbar={}, inline_active={}, drag_active={}",
                button,
                on_toolbar,
                inline_active,
                self.is_move_dragging()
            );
        }
        if inline_active && self.handle_inline_pointer_press(conn, qh, event, button) {
            return;
        }
        if on_toolbar {
            self.handle_toolbar_pointer_press(conn, qh, event, button);
            return;
        } else if self.pointer_over_toolbar() {
            self.finish_toolbar_item_drag(false);
            self.set_toolbar_dragging(false);
            return;
        }

        if button == BTN_LEFT && self.dismiss_top_toolbar_menus() {
            return;
        }

        if button == BTN_LEFT && self.handle_overlay_pointer_press(event.position) {
            return;
        }

        debug!(
            "Button {} pressed at ({}, {})",
            button, event.position.0, event.position.1
        );
        if self.zoom.active && button == BTN_MIDDLE && !self.zoom.locked {
            self.zoom.start_pan(event.position.0, event.position.1);
            self.input_state.dirty_tracker.mark_full();
            self.input_state.needs_redraw = true;
            return;
        }
        if button == BTN_LEFT && self.board_pan_key_held() && self.can_start_board_pan() {
            self.start_board_pan(event.position.0, event.position.1);
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
            .on_mouse_press_with_canvas(mb, screen_x, screen_y, wx, wy);
        self.input_state.needs_redraw = true;
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
        if on_toolbar || self.pointer_over_toolbar() {
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
                            self.suppress_next_release_from(RegionInputSource::Pointer);
                        }
                    }
                }
            }
            BTN_RIGHT => {
                self.cancel_active_region_selector();
                self.suppress_next_release_from(RegionInputSource::Pointer);
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
        if on_toolbar || self.pointer_over_toolbar() {
            self.cancel_eyedropper();
            return false;
        }
        match button {
            BTN_LEFT => {
                self.sample_eyedropper(event.position.0, event.position.1);
                self.suppress_next_release_from(RegionInputSource::Pointer);
            }
            BTN_RIGHT => {
                self.cancel_eyedropper();
                self.suppress_next_release_from(RegionInputSource::Pointer);
            }
            _ => {}
        }
        true
    }

    fn handle_modal_pointer_press(
        &mut self,
        event: &PointerEvent,
        on_toolbar: bool,
        button: u32,
        help_press_source: HelpOverlayPressSource,
    ) -> bool {
        if self.input_state.tour_active {
            return true;
        }
        // Help is modal: remember the target so release can require the same row.
        if self.input_state.help_overlay.is_visible() {
            let screen_position = if on_toolbar {
                self.toolbar_surface_screen_coords(&event.surface, event.position)
            } else {
                Some(event.position)
            };
            match screen_position {
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
            let handled = self.input_state.handle_command_palette_click(
                event.position.0 as i32,
                event.position.1 as i32,
                self.surface.width(),
                self.surface.height(),
            );
            if handled {
                self.suppress_next_release_from(RegionInputSource::Pointer);
            }
        }
        true
    }

    fn handle_inline_pointer_press(
        &mut self,
        conn: &wayland_client::Connection,
        qh: &QueueHandle<Self>,
        event: &PointerEvent,
        button: u32,
    ) -> bool {
        if button == BTN_RIGHT
            && self.inline_toolbar_secondary_press(event.position, Some(conn), Some(qh))
        {
            self.refresh_keyboard_interactivity();
            return true;
        }
        if button == BTN_LEFT && self.inline_toolbar_press(event.position, Some(conn), Some(qh)) {
            drag_log(|| {
                format!(
                    "pointer press: inline handled, drag_active={}, pos=({:.3}, {:.3}), surface={}",
                    self.toolbar_dragging(),
                    event.position.0,
                    event.position.1,
                    surface_id(&event.surface)
                )
            });
            if self.is_move_dragging() {
                self.lock_pointer_for_drag(qh, &event.surface);
            }
            return true;
        }
        if !self.pointer_over_toolbar() {
            return false;
        }
        if button == BTN_LEFT {
            self.dismiss_top_toolbar_menus();
        }
        true
    }

    fn handle_toolbar_pointer_press(
        &mut self,
        conn: &wayland_client::Connection,
        qh: &QueueHandle<Self>,
        event: &PointerEvent,
        button: u32,
    ) {
        if button == BTN_RIGHT
            && let Some(index) = self
                .toolbar
                .quick_color_slot_at(&event.surface, event.position)
        {
            self.handle_toolbar_event(ToolbarEvent::EditQuickColor { index }, Some(conn), Some(qh));
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
            self.refresh_keyboard_interactivity();
            return;
        }
        let handled = if button == BTN_LEFT {
            self.handle_primary_toolbar_pointer_press(conn, qh, event)
        } else {
            false
        };
        if button == BTN_LEFT && !handled {
            self.dismiss_top_toolbar_menus();
        }
    }

    fn handle_primary_toolbar_pointer_press(
        &mut self,
        conn: &wayland_client::Connection,
        qh: &QueueHandle<Self>,
        event: &PointerEvent,
    ) -> bool {
        let Some((intent, drag)) = self.toolbar.pointer_press(&event.surface, event.position)
        else {
            return false;
        };
        let toolbar_event = intent_to_event(intent, self.toolbar.last_snapshot());
        if matches!(toolbar_event, ToolbarEvent::MoveTopToolbar { .. }) && drag {
            self.lock_pointer_for_drag(qh, &event.surface);
        }
        log::info!(
            "toolbar press: drag_start={}, surface={}, seat={:?}, inline_active={}",
            drag,
            surface_id(&event.surface),
            self.current_seat_id(),
            self.inline_toolbars_active()
        );
        self.set_toolbar_dragging(drag);
        self.handle_toolbar_event(toolbar_event, Some(conn), Some(qh));
        self.toolbar.mark_dirty();
        self.input_state.needs_redraw = true;
        self.refresh_keyboard_interactivity();
        true
    }

    fn handle_overlay_pointer_press(&mut self, position: (f64, f64)) -> bool {
        let screen_x = position.0.round() as i32;
        let screen_y = position.1.round() as i32;
        self.set_pending_toast_press(None);
        if let Some(pressed) = self.input_state.toast_press_at(screen_x, screen_y) {
            self.set_pending_toast_press(Some(pressed));
            return true;
        }
        self.set_pending_status_hud_press(false);
        if self.input_state.status_hud_contains(screen_x, screen_y) {
            self.set_pending_status_hud_press(true);
            return true;
        }
        self.set_pending_zoom_chip_press(ZoomChipPress::None);
        if !self.input_state.zoom_chip_contains(screen_x, screen_y) {
            return false;
        }
        let pressed = self.input_state.zoom_chip_press_at(screen_x, screen_y);
        self.set_pending_zoom_chip_press(pressed);
        true
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

    /// Click-away dismissal for the top-strip menus/popovers. Defers to the
    /// canonical [`InputState::close_top_toolbar_menus`] so the click-away set
    /// stays in lockstep with the keyboard Escape route and the apply-action
    /// callers — the Canvas popover in particular must dismiss here exactly
    /// like the Session/Settings popovers, else a canvas click would leak
    /// through and start a stray stroke. Returns whether a menu was open so the
    /// press handler early-returns instead of drawing.
    ///
    /// Shared with the touch-down and tablet pen-down paths so every canvas
    /// down modality dismisses the Canvas (and Session/Settings) popover and
    /// swallows the interaction identically.
    pub(in crate::backend::wayland) fn dismiss_top_toolbar_menus(&mut self) -> bool {
        let changed = self.input_state.close_top_toolbar_menus();
        if changed {
            if self.inline_toolbars_active() {
                self.mark_inline_toolbar_full_damage();
            } else {
                self.toolbar.mark_dirty();
            }
        }
        changed
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

#[cfg(test)]
mod tests {
    use super::review_action_suppresses_next_release;
    use crate::ui::RegionAction;

    #[test]
    fn retained_review_toggle_does_not_arm_the_post_modal_release_latch() {
        assert!(!review_action_suppresses_next_release(
            RegionAction::ToggleIncludeDrawings
        ));
        for terminal in [
            RegionAction::Copy,
            RegionAction::Save,
            RegionAction::Both,
            RegionAction::Board,
        ] {
            assert!(review_action_suppresses_next_release(terminal));
        }
    }
}
