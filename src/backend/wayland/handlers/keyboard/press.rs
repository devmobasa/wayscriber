//! Key-press routing shared by the overlay's own keyboard focus and the GTK
//! toolbar, which forwards the presses it receives while it holds focus.

use log::debug;
use smithay_client_toolkit::seat::keyboard::Keysym;
use std::time::Instant;
use wayland_client::{Connection, QueueHandle};

use super::super::super::state::WaylandState;
use super::{is_repeatable_key, keysym_to_key, should_try_toolbar_key};
use crate::input::Key;

/// Where a key press came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum KeyPressSource {
    /// The overlay's own `wl_keyboard`, which also reports the release.
    Seat,
    /// The GTK toolbar, which holds keyboard focus on its own Wayland
    /// connection and forwards presses only. GTK repeats held keys itself.
    GtkToolbar,
}

impl KeyPressSource {
    /// Held-key behaviors (board pan on Space, auto-repeat) wait for a
    /// release that only the overlay's own keyboard delivers.
    pub(in crate::backend::wayland) fn delivers_release(self) -> bool {
        matches!(self, Self::Seat)
    }
}

/// A key press the GTK toolbar received while it held keyboard focus, with
/// the modifier state GTK saw at that moment. `keyval` is a GDK keyval, which
/// uses the X keysym numbering xkbcommon shares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) struct ForwardedKey {
    pub(in crate::backend::wayland) keyval: u32,
    pub(in crate::backend::wayland) ctrl: bool,
    pub(in crate::backend::wayland) shift: bool,
    pub(in crate::backend::wayland) alt: bool,
    pub(in crate::backend::wayland) logo: bool,
}

impl ForwardedKey {
    /// The overlay key this press stands for, or `None` for a bare modifier:
    /// its state already rides on the next forwarded key, and forwarding the
    /// press itself would latch a modifier whose release never arrives.
    pub(in crate::backend::wayland) fn key(self) -> Option<Key> {
        let key = keysym_to_key(Keysym::new(self.keyval));
        let modifier = matches!(
            key,
            Key::Shift | Key::Ctrl | Key::Alt | Key::Super | Key::Tab | Key::Unknown
        );
        (!modifier).then_some(key)
    }
}

impl WaylandState {
    /// Route a key press the GTK toolbar forwarded while it held keyboard
    /// focus.
    ///
    /// The overlay lost keyboard focus to the toolbar, so its own modifier
    /// state was reset and its readiness gate closed. The press carries GTK's
    /// modifiers for the length of this dispatch only; the overlay's state is
    /// restored afterwards so no modifier stays latched.
    pub(in crate::backend::wayland) fn dispatch_gtk_forwarded_key(
        &mut self,
        forwarded: ForwardedKey,
        conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let Some(key) = forwarded.key() else {
            return;
        };

        let saved = self.input_state.modifiers;
        self.input_state.sync_modifiers(
            forwarded.shift,
            forwarded.ctrl,
            forwarded.alt,
            forwarded.logo,
        );
        self.dispatch_key_press(key, KeyPressSource::GtkToolbar, conn, qh);

        self.input_state.modifiers.tab = saved.tab;
        self.input_state
            .sync_modifiers(saved.shift, saved.ctrl, saved.alt, saved.logo);
    }

    /// Route one key press through the overlay's shortcut handling.
    ///
    /// Shared by presses from the overlay's own keyboard focus and presses the
    /// GTK toolbar forwards while it holds focus, so both reach the same
    /// region, zoom, toolbar, and input routes.
    pub(in crate::backend::wayland) fn dispatch_key_press(
        &mut self,
        key: Key,
        source: KeyPressSource,
        conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        // Report the physical press to the input HUD before any subsystem
        // routing consumes it, so the HUD always shows what was pressed rather
        // than what happened to reach the canvas. Compositor-synced modifier
        // state is already current here, so the chord label is exact.
        self.input_state
            .note_input_hud_key(key, self.input_state.modifiers);
        // Any fresh key press ends the previous auto-repeat; a repeatable one
        // re-arms it at the end of this handler.
        self.clear_key_repeat();
        // A finished scan card is transient chrome: the next interaction of any
        // kind takes it away rather than making the user wait it out.
        self.input_state.dismiss_ocr_scan_result();
        if self.try_handle_region_key(conn, key) {
            return;
        }
        if self.try_handle_eyedropper_key(key) {
            return;
        }
        if matches!(key, Key::Escape)
            && self.input_state.modifiers.shift
            && self.try_skip_first_run_onboarding()
        {
            return;
        }
        if self.try_handle_first_run_card_key(key) {
            return;
        }
        if matches!(key, Key::Space)
            && source.delivers_release()
            && self.should_capture_space_for_board_pan()
        {
            self.pointer.set_board_pan_key_held(true);
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
        if !modal_blocks_repeat
            && source.delivers_release()
            && is_repeatable_key(key)
            && self.focus.keyboard_focused()
        {
            self.key_repeat
                .arm(key, Instant::now(), Self::KEY_REPEAT_INITIAL_DELAY);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forwarded(keysym: Keysym) -> ForwardedKey {
        ForwardedKey {
            keyval: keysym.raw(),
            ctrl: false,
            shift: false,
            alt: false,
            logo: false,
        }
    }

    #[test]
    fn forwarded_shortcut_keys_map_to_overlay_keys() {
        assert_eq!(forwarded(Keysym::Escape).key(), Some(Key::Escape));
        assert_eq!(forwarded(Keysym::s).key(), Some(Key::Char('s')));
        assert_eq!(forwarded(Keysym::S).key(), Some(Key::Char('S')));
        assert_eq!(forwarded(Keysym::F1).key(), Some(Key::F1));
    }

    /// A forwarded modifier press has no matching release, so it must never
    /// reach the overlay's modifier latches.
    #[test]
    fn forwarded_modifier_presses_are_dropped() {
        for keysym in [
            Keysym::Shift_L,
            Keysym::Control_R,
            Keysym::Alt_L,
            Keysym::Super_L,
            Keysym::Tab,
            Keysym::Caps_Lock,
        ] {
            assert_eq!(forwarded(keysym).key(), None, "{keysym:?}");
        }
    }

    #[test]
    fn only_the_overlay_seat_delivers_releases() {
        assert!(KeyPressSource::Seat.delivers_release());
        assert!(!KeyPressSource::GtkToolbar.delivers_release());
    }
}
