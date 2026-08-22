use log::debug;
use smithay_client_toolkit::seat::pointer::{BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, PointerEvent};
use wayland_client::QueueHandle;

use crate::backend::wayland::state::drag_log;
use crate::backend::wayland::toolbar_intent::intent_to_event;
use crate::input::MouseButton;
use crate::input::state::HelpOverlayPressSource;
use crate::ui::ZoomChipPress;
use crate::ui::toolbar::ToolbarEvent;

use super::*;

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

        let help_press_source = HelpOverlayPressSource::Pointer(button);
        if !self.input_state.show_help {
            // A new press proves any older help-owned sequence for this button
            // has ended, even if its release was lost with a surface/device.
            self.input_state
                .clear_help_overlay_press_for(help_press_source);
        }

        if self.input_state.region_is_active() {
            if on_toolbar || self.pointer_over_toolbar() {
                // A toolbar interaction ends the region first, then runs
                // normally; the click never lands on the selector.
                self.cancel_region_for_toolbar_interaction();
            } else {
                match button {
                    BTN_LEFT => {
                        self.begin_region_selection(
                            RegionInputSource::Pointer,
                            event.position.0,
                            event.position.1,
                        );
                    }
                    BTN_RIGHT => {
                        self.cancel_active_region_selector();
                        self.set_suppress_next_release(true);
                    }
                    _ => {}
                }
                return;
            }
        }

        if self.input_state.eyedropper_is_active() {
            if on_toolbar || self.pointer_over_toolbar() {
                self.cancel_eyedropper();
            } else {
                match button {
                    BTN_LEFT => {
                        self.sample_eyedropper(event.position.0, event.position.1);
                        self.set_suppress_next_release(true);
                    }
                    BTN_RIGHT => {
                        self.cancel_eyedropper();
                        self.set_suppress_next_release(true);
                    }
                    _ => {}
                }
                return;
            }
        }

        // Block pointer input when tour is active
        if self.input_state.tour_active {
            return;
        }

        // The help overlay is modal for pointer input: swallow the press so a
        // click never starts a stroke on the canvas beneath it, but record the
        // help target under the press. The release enforces a same-target
        // contract before it runs a row (or dismisses), so a press that starts
        // on bare chrome (or outside) and drags onto a clickable row — notably
        // the destructive Clear row — never executes it. Toolbar surfaces report
        // toolbar-local coordinates, so convert to screen space just like the
        // toast/status-HUD press guards below.
        if self.input_state.show_help {
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
            return;
        }

        // Handle command palette clicks
        if self.input_state.command_palette_is_engaged() {
            if button == BTN_LEFT {
                let screen_width = self.surface.width();
                let screen_height = self.surface.height();
                if self.input_state.handle_command_palette_click(
                    event.position.0 as i32,
                    event.position.1 as i32,
                    screen_width,
                    screen_height,
                ) {
                    self.set_suppress_next_release(true);
                }
            }
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
        if inline_active {
            // Inline bars share the canvas surface, so their swatch recolor
            // gesture is resolved here rather than in the `on_toolbar` branch.
            if button == BTN_RIGHT
                && self.inline_toolbar_secondary_press(event.position, Some(conn), Some(qh))
            {
                self.refresh_keyboard_interactivity();
                return;
            }
            if button == BTN_LEFT && self.inline_toolbar_press(event.position, Some(conn), Some(qh))
            {
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
                return;
            }
            if self.pointer_over_toolbar() {
                if button == BTN_LEFT {
                    self.dismiss_top_toolbar_menus();
                }
                return;
            }
        }
        if on_toolbar {
            // Secondary click on a quick-color swatch opens the picker bound to
            // that palette slot. Every other toolbar control ignores the button,
            // so the press is consumed only when it lands on a swatch.
            if button == BTN_RIGHT
                && let Some(index) = self
                    .toolbar
                    .quick_color_slot_at(&event.surface, event.position)
            {
                self.handle_toolbar_event(
                    ToolbarEvent::EditQuickColor { index },
                    Some(conn),
                    Some(qh),
                );
                self.toolbar.mark_dirty();
                self.input_state.needs_redraw = true;
                self.refresh_keyboard_interactivity();
                return;
            }
            let mut handled = false;
            if button == BTN_LEFT
                && let Some((intent, drag)) =
                    self.toolbar.pointer_press(&event.surface, event.position)
            {
                handled = true;
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
            }
            if button == BTN_LEFT && !handled {
                self.dismiss_top_toolbar_menus();
            }
            return;
        } else if self.pointer_over_toolbar() {
            self.finish_toolbar_item_drag(false);
            self.set_toolbar_dragging(false);
            return;
        }

        if button == BTN_LEFT && self.dismiss_top_toolbar_menus() {
            return;
        }

        if button == BTN_LEFT {
            let screen_x = event.position.0.round() as i32;
            let screen_y = event.position.1.round() as i32;
            self.set_pending_toast_press(None);
            if let Some(pressed) = self.input_state.toast_press_at(screen_x, screen_y) {
                self.set_pending_toast_press(Some(pressed));
                return;
            }
            // Interactive status HUD: report the hit on press without side
            // effects; the matching surface opens on release-inside.
            self.set_pending_status_hud_press(false);
            if self.input_state.status_hud_contains(screen_x, screen_y) {
                self.set_pending_status_hud_press(true);
                return;
            }
            // Interactive zoom chip: same press→release contract. Any press
            // inside the pill is swallowed here and recorded as `Passive` (the
            // `NN%` readout / inter-piece gap) or `Button(kind)`, so its release
            // stays consumed either way; a `Button` release dispatches its zoom
            // action only when it lands on the SAME button. The cached layout
            // only exists while the chip is visible, so `zoom_chip_contains` is
            // false when it is hidden and the press falls through.
            self.set_pending_zoom_chip_press(ZoomChipPress::None);
            if self.input_state.zoom_chip_contains(screen_x, screen_y) {
                let pressed = self.input_state.zoom_chip_press_at(screen_x, screen_y);
                self.set_pending_zoom_chip_press(pressed);
                return;
            }
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
