// Bridges Wayland key events into our `InputState`, including capture-action plumbing.
mod translate;

use log::{debug, warn};
use smithay_client_toolkit::seat::keyboard::{KeyEvent, KeyboardHandler, Modifiers, RawModifiers};
use wayland_client::{
    Connection, QueueHandle,
    protocol::{wl_keyboard, wl_surface},
};

use crate::{input::Key, notification};

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

        if self.surface.is_xdg_window() && self.focus_exit_suppressed() {
            warn!("Keyboard focus lost in xdg fallback; suppressing exit after clipboard action");
            self.request_xdg_activation(qh);
            return;
        }

        if self.surface.is_xdg_window() {
            warn!("Keyboard focus lost in xdg fallback; exiting overlay");
            notification::send_notification_async(
                &self.tokio_handle,
                "Wayscriber lost focus".to_string(),
                "GNOME could not keep the overlay focused; closing fallback window.".to_string(),
                Some("dialog-warning".to_string()),
            );
            self.input_state.should_exit = true;
        }
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
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
                    self.input_state.dirty_tracker.mark_full();
                    self.input_state.needs_redraw = true;
                    return;
                }
                _ => {}
            }
        }
        debug!("Key pressed: {:?}", key);
        if should_try_toolbar_key(key, self.input_state.command_palette_open)
            && self.handle_toolbar_key(key)
        {
            return;
        }

        self.apply_input_key(key);

        if let Some(action) = self.input_state.take_pending_capture_action() {
            self.handle_capture_action(action);
        }
        if let Some(action) = self.input_state.take_pending_zoom_action() {
            self.handle_zoom_action(action);
        }
        if let Some(action) = self.input_state.take_pending_preset_action() {
            self.handle_preset_action(action);
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
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        // Block keybinds until overlay is fully ready
        if !self.is_overlay_ready() {
            return;
        }
        let key = keysym_to_key(event.keysym);
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
                    self.input_state.dirty_tracker.mark_full();
                    self.input_state.needs_redraw = true;
                    return;
                }
                _ => {}
            }
        }
        if should_try_toolbar_key(key, self.input_state.command_palette_open)
            && self.handle_toolbar_key(key)
        {
            return;
        }
        debug!("Key repeated: {:?}", key);

        self.apply_input_key(key);

        if let Some(action) = self.input_state.take_pending_zoom_action() {
            self.handle_zoom_action(action);
        }
    }
}

fn should_try_toolbar_key(key: Key, command_palette_open: bool) -> bool {
    if command_palette_open {
        return false;
    }
    matches!(key, Key::Tab | Key::Return | Key::Space)
}

impl WaylandState {
    fn apply_input_key(&mut self, key: Key) {
        #[cfg(tablet)]
        let prev_thickness = self.input_state.current_thickness;
        let prefs_before = (
            self.input_state.current_color,
            self.input_state.current_thickness,
            self.input_state.eraser_mode,
            self.input_state.marker_opacity,
            self.input_state.current_font_size,
            self.input_state.font_descriptor.clone(),
            self.input_state.fill_enabled,
        );
        let highlight_before = (
            self.input_state.click_highlight_enabled(),
            self.input_state.highlight_tool_ring_enabled(),
            self.input_state.presenter_mode,
        );
        self.input_state.on_key_press(key);
        self.input_state.needs_redraw = true;
        let prefs_changed = prefs_before.0 != self.input_state.current_color
            || (prefs_before.1 - self.input_state.current_thickness).abs() > f64::EPSILON
            || prefs_before.2 != self.input_state.eraser_mode
            || (prefs_before.3 - self.input_state.marker_opacity).abs() > f64::EPSILON
            || (prefs_before.4 - self.input_state.current_font_size).abs() > f64::EPSILON
            || prefs_before.5 != self.input_state.font_descriptor
            || prefs_before.6 != self.input_state.fill_enabled;
        if prefs_changed {
            self.save_drawing_preferences();
        }
        let highlight_after = (
            self.input_state.click_highlight_enabled(),
            self.input_state.highlight_tool_ring_enabled(),
            self.input_state.presenter_mode,
        );
        if highlight_before.2 == highlight_after.2
            && (highlight_before.0 != highlight_after.0 || highlight_before.1 != highlight_after.1)
        {
            self.save_click_highlight_preferences();
        }

        #[cfg(tablet)]
        if (self.input_state.current_thickness - prev_thickness).abs() > f64::EPSILON {
            self.stylus_base_thickness = Some(self.input_state.current_thickness);
            if self.stylus_tip_down {
                self.stylus_pressure_thickness = Some(self.input_state.current_thickness);
                self.record_stylus_peak(self.input_state.current_thickness);
            } else {
                self.stylus_pressure_thickness = None;
                self.stylus_peak_thickness = None;
            }
        }

        if let Some(action) = self.input_state.take_pending_zoom_action() {
            self.handle_zoom_action(action);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_routing_is_blocked_when_command_palette_is_open() {
        assert!(!should_try_toolbar_key(Key::Tab, true));
        assert!(!should_try_toolbar_key(Key::Return, true));
        assert!(!should_try_toolbar_key(Key::Space, true));
    }

    #[test]
    fn toolbar_routing_only_allows_activate_and_tab_keys() {
        assert!(should_try_toolbar_key(Key::Tab, false));
        assert!(should_try_toolbar_key(Key::Return, false));
        assert!(should_try_toolbar_key(Key::Space, false));
        assert!(!should_try_toolbar_key(Key::Down, false));
    }
}
