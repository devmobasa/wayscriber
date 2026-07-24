// Bridges Wayland key events into our `InputState`, including capture-action plumbing.
mod translate;

use log::{debug, warn};
use smithay_client_toolkit::seat::keyboard::{KeyEvent, KeyboardHandler, Modifiers, RawModifiers};
use std::time::{Duration, Instant};
use wayland_client::{
    Connection, QueueHandle,
    protocol::{wl_keyboard, wl_surface},
};

use crate::{config::Action, input::Key, notification};

use super::super::state::WaylandState;
use translate::keysym_to_key;

impl KeyboardHandler for WaylandState {
    fn enter(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        serial: u32,
        _raw: &[u32],
        _keysyms: &[smithay_client_toolkit::seat::keyboard::Keysym],
    ) {
        debug!("Keyboard focus entered");
        self.set_keyboard_focus(true);
        self.clear_focus_exit_suppression();
        self.clear_xdg_close_guard();
        self.set_last_activation_serial(Some(serial));
        self.maybe_retry_activation(qh);
        if let Some(target) = self.toolbar.focus_target_for_surface(surface) {
            self.set_toolbar_focus_target(Some(target));
        } else {
            self.clear_toolbar_focus();
        }
        // Mark overlay as ready once we have focus and surface is configured
        if self.surface.is_configured() {
            self.set_overlay_ready(true);
            debug!("Overlay ready for keybinds");
        }
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
        debug!("Keyboard focus left");
        self.set_keyboard_focus(false);
        self.set_overlay_ready(false);
        self.clear_toolbar_focus();

        // When the compositor moves focus away from our surface (e.g. to a portal
        // dialog, another layer surface, or a different window), it's possible for
        // us to miss some key release events. To avoid leaving modifiers "stuck"
        // and breaking shortcuts/tools, aggressively reset our modifier state on
        // focus loss.
        self.input_state.reset_modifiers();
        self.input_state.clear_command_palette_repeat();
        self.clear_key_repeat();
        self.set_board_pan_key_held(false);
        self.stop_board_pan();

        if self.surface.is_xdg_window() && self.focus_exit_suppressed() {
            warn!("Keyboard focus lost in xdg fallback; suppressing exit after clipboard action");
            self.set_xdg_close_guard_for(Duration::from_millis(2500));
            self.request_xdg_activation(qh);
            return;
        }

        if self.surface.is_xdg_window() {
            if !self.xdg_focus_loss_exits_overlay() {
                warn!(
                    "Keyboard focus lost in xdg fallback; keeping overlay open without auto-reactivation (ui.xdg_focus_loss_behavior=stay)"
                );
                self.set_xdg_close_guard_for(Duration::from_millis(2500));
                return;
            }
            warn!("Keyboard focus lost in xdg fallback; exiting overlay");
            notification::send_notification_async(
                &self.tokio_handle,
                "Wayscriber lost focus".to_string(),
                "The desktop could not keep the overlay focused, so Wayscriber closed it."
                    .to_string(),
                Some("dialog-warning".to_string()),
            );
            self.input_state.should_exit = true;
        }
    }

    fn press_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        // Block keybinds until overlay is fully ready (prevents Ctrl+W leaking to apps)
        if !self.is_overlay_ready() {
            debug!("Ignoring key press before overlay ready");
            return;
        }
        let key = keysym_to_key(event.keysym);
        // Any fresh key press ends the previous auto-repeat; a repeatable one
        // re-arms it at the end of this handler.
        self.clear_key_repeat();
        if self.input_state.eyedropper_is_engaged() {
            if matches!(key, Key::Escape)
                || self.input_state.action_for_key(key) == Some(Action::PickScreenColor)
            {
                self.cancel_eyedropper();
            }
            return;
        }
        if matches!(key, Key::Escape)
            && self.input_state.modifiers.shift
            && self.try_skip_first_run_onboarding()
        {
            return;
        }
        if self.try_handle_first_run_background_mode_choice(key) {
            return;
        }
        if matches!(key, Key::Space) && self.should_capture_space_for_board_pan() {
            self.set_board_pan_key_held(true);
            self.input_state.needs_redraw = true;
            return;
        }
        if self.zoom.is_engaged() {
            match key {
                Key::Escape => {
                    self.exit_zoom();
                    return;
                }
                Key::Up | Key::Down | Key::Left | Key::Right => {
                    if !self.zoom.active {
                        return;
                    }
                    if self.zoom.locked {
                        return;
                    }
                    let step = if self.input_state.modifiers.shift {
                        WaylandState::ZOOM_PAN_STEP_LARGE
                    } else {
                        WaylandState::ZOOM_PAN_STEP
                    };
                    let (dx, dy) = match key {
                        Key::Up => (0.0, step),
                        Key::Down => (0.0, -step),
                        Key::Left => (step, 0.0),
                        Key::Right => (-step, 0.0),
                        _ => (0.0, 0.0),
                    };
                    self.zoom.pan_by_screen_delta(
                        dx,
                        dy,
                        self.surface.width(),
                        self.surface.height(),
                    );
                    self.sync_input_zoom_state();
                    self.input_state.dirty_tracker.mark_full();
                    self.input_state.needs_redraw = true;
                    return;
                }
                _ => {}
            }
        }
        debug!("Key pressed: {:?}", key);
        let modal_capture = self.input_state.modal_owns_text_input();
        let modal_blocks_repeat = self.input_state.modal_blocks_canvas_key_repeat();
        if should_try_toolbar_key(key, modal_capture)
            && self.handle_toolbar_key(key, Some(conn), Some(qh))
        {
            return;
        }

        self.apply_input_key(key);

        // Arm auto-repeat for editing/navigation keys that reached normal
        // dispatch. Some dedicated entry modals manage or intentionally block
        // repeat themselves; other routed overlays (for example Help search)
        // still use this timer even though they disable the canvas IME.
        if !modal_blocks_repeat && Self::is_repeatable_key(key) && self.has_keyboard_focus() {
            self.key_repeat_key = Some(key);
            self.key_repeat_next_tick = Some(Instant::now() + Self::KEY_REPEAT_INITIAL_DELAY);
        }
    }

    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        let key = keysym_to_key(event.keysym);
        debug!("Key released: {:?}", key);
        // Stop auto-repeat once the held key comes up.
        if self.key_repeat_key == Some(key) {
            self.clear_key_repeat();
        }
        if self.input_state.eyedropper_is_engaged() {
            return;
        }
        if matches!(key, Key::Space) && self.board_pan_key_held() {
            self.set_board_pan_key_held(false);
            self.input_state.needs_redraw = true;
            return;
        }
        self.input_state.on_key_release(key);
    }

    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        modifiers: Modifiers,
        _layout: RawModifiers,
        _group: u32,
    ) {
        debug!(
            "Modifiers: ctrl={} alt={} shift={}",
            modifiers.ctrl, modifiers.alt, modifiers.shift
        );
        // Trust compositor-reported modifier state to reconcile any missed key release
        // events and avoid "stuck" modifiers.
        self.input_state
            .sync_modifiers(modifiers.shift, modifiers.ctrl, modifiers.alt);
    }

    fn repeat_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        // sctk only calls this with a calloop-driven repeat keyboard, which
        // this manual poll loop does not use; the loop's `tick_key_repeat`
        // drives repeats through the same path instead. Kept for parity.
        self.dispatch_key_repeat(keysym_to_key(event.keysym), conn, qh);
    }
}

/// Delay before a held key begins repeating.
const KEY_REPEAT_INITIAL_DELAY: Duration = Duration::from_millis(400);
/// Interval between repeats once repeating (≈25/s).
const KEY_REPEAT_INTERVAL: Duration = Duration::from_millis(40);

impl WaylandState {
    pub(in crate::backend::wayland) const KEY_REPEAT_INITIAL_DELAY: Duration =
        KEY_REPEAT_INITIAL_DELAY;

    /// Keys that auto-repeat while held: text entry, deletion, and
    /// navigation. Action/toggle keys (Return, Escape, Tab, F-keys) are left
    /// out so holding them never spams their one-shot effect.
    fn is_repeatable_key(key: Key) -> bool {
        matches!(
            key,
            Key::Char(_)
                | Key::Backspace
                | Key::Delete
                | Key::Space
                | Key::Left
                | Key::Right
                | Key::Up
                | Key::Down
                | Key::Home
                | Key::End
                | Key::PageUp
                | Key::PageDown
        )
    }

    pub(in crate::backend::wayland) fn clear_key_repeat(&mut self) {
        self.key_repeat_key = None;
        self.key_repeat_next_tick = None;
    }

    /// Duration until the next repeat fires, for the event-loop timeout. The
    /// loop otherwise sleeps until a real event and would never wake to
    /// repeat a held key.
    pub(in crate::backend::wayland) fn key_repeat_timeout(&self, now: Instant) -> Option<Duration> {
        if !self.has_keyboard_focus() {
            return None;
        }
        self.key_repeat_next_tick
            .map(|next| next.saturating_duration_since(now))
    }

    /// Fire a repeat if one is due, then reschedule from `now` (so a long
    /// block does not burst-catch-up). Called once per event-loop iteration.
    pub(in crate::backend::wayland) fn tick_key_repeat(
        &mut self,
        now: Instant,
        conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if !self.has_keyboard_focus() {
            self.clear_key_repeat();
            return;
        }
        if self.input_state.modal_blocks_canvas_key_repeat() {
            // A modal can open from pointer/toolbar input while a canvas key is
            // still held. Retire that timer before it starts feeding the new
            // focus owner (or duplicates the command palette's own repeat).
            self.clear_key_repeat();
            return;
        }
        let Some(key) = self.key_repeat_key else {
            return;
        };
        let Some(next) = self.key_repeat_next_tick else {
            return;
        };
        if now < next {
            return;
        }
        self.dispatch_key_repeat(key, conn, qh);
        self.key_repeat_next_tick = Some(now + KEY_REPEAT_INTERVAL);
    }

    /// Re-dispatch a held key through the same routing a fresh press uses
    /// (overlay-ready gate, eyedropper/zoom/pan guards, toolbar routing, then
    /// `apply_input_key`). Shared by the manual repeat tick and sctk's
    /// `repeat_key`.
    fn dispatch_key_repeat(&mut self, key: Key, conn: &Connection, qh: &QueueHandle<Self>) {
        if !self.is_overlay_ready() {
            return;
        }
        if self.input_state.eyedropper_is_engaged() {
            return;
        }
        if matches!(key, Key::Space) && self.board_pan_key_held() {
            return;
        }
        if self.zoom.active {
            match key {
                Key::Up | Key::Down | Key::Left | Key::Right => {
                    if self.zoom.locked {
                        return;
                    }
                    let step = if self.input_state.modifiers.shift {
                        WaylandState::ZOOM_PAN_STEP_LARGE
                    } else {
                        WaylandState::ZOOM_PAN_STEP
                    };
                    let (dx, dy) = match key {
                        Key::Up => (0.0, step),
                        Key::Down => (0.0, -step),
                        Key::Left => (step, 0.0),
                        Key::Right => (-step, 0.0),
                        _ => (0.0, 0.0),
                    };
                    self.zoom.pan_by_screen_delta(
                        dx,
                        dy,
                        self.surface.width(),
                        self.surface.height(),
                    );
                    self.sync_input_zoom_state();
                    self.input_state.dirty_tracker.mark_full();
                    self.input_state.needs_redraw = true;
                    return;
                }
                _ => {}
            }
        }
        if self.input_state.command_palette_open && matches!(key, Key::Up | Key::Down) {
            return;
        }
        let modal_capture = self.input_state.modal_owns_text_input();
        if should_try_toolbar_key(key, modal_capture)
            && self.handle_toolbar_key(key, Some(conn), Some(qh))
        {
            return;
        }
        self.apply_input_key(key);
    }
}

fn should_try_toolbar_key(key: Key, modal_capture_active: bool) -> bool {
    if modal_capture_active {
        return false;
    }
    matches!(key, Key::Tab | Key::Return | Key::Space | Key::Escape)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_routing_is_blocked_while_a_modal_capture_is_active() {
        assert!(!should_try_toolbar_key(Key::Tab, true));
        assert!(!should_try_toolbar_key(Key::Return, true));
        assert!(!should_try_toolbar_key(Key::Space, true));
    }

    #[test]
    fn toolbar_routing_only_allows_activate_and_tab_keys() {
        assert!(should_try_toolbar_key(Key::Tab, false));
        assert!(should_try_toolbar_key(Key::Return, false));
        assert!(should_try_toolbar_key(Key::Space, false));
        assert!(should_try_toolbar_key(Key::Escape, false));
        assert!(!should_try_toolbar_key(Key::Down, false));
    }

    #[test]
    fn text_and_navigation_keys_auto_repeat() {
        // The reported case (hold Backspace to delete) plus the rest of the
        // editing/navigation set.
        assert!(WaylandState::is_repeatable_key(Key::Backspace));
        assert!(WaylandState::is_repeatable_key(Key::Delete));
        assert!(WaylandState::is_repeatable_key(Key::Char('a')));
        assert!(WaylandState::is_repeatable_key(Key::Space));
        for key in [Key::Left, Key::Right, Key::Up, Key::Down] {
            assert!(WaylandState::is_repeatable_key(key));
        }
    }

    #[test]
    fn one_shot_keys_do_not_auto_repeat() {
        // Holding these must never spam their effect.
        assert!(!WaylandState::is_repeatable_key(Key::Return));
        assert!(!WaylandState::is_repeatable_key(Key::Escape));
        assert!(!WaylandState::is_repeatable_key(Key::Tab));
        assert!(!WaylandState::is_repeatable_key(Key::F10));
    }
}
