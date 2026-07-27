use std::time::Instant;

use crate::config::{InputHudMode, InputHudPosition};
use crate::input::Key;
use crate::input::modifiers::Modifiers;
use crate::input::state::input_hud::{
    InputHudActiveSource, InputHudEntry, InputHudSettings, input_hud_key_label,
    input_hud_mouse_label, input_hud_scroll_label, is_bare_modifier,
};

use super::base::InputState;

impl InputState {
    /// Apply the startup settings snapshot for the input HUD.
    pub fn init_input_hud_from_config(&mut self, settings: InputHudSettings) {
        self.input_hud.apply_settings(settings);
    }

    /// Returns whether the input HUD is currently enabled.
    pub fn input_hud_enabled(&self) -> bool {
        self.input_hud.enabled()
    }

    /// The configured input source preference (`auto`/`overlay`/`system`).
    pub fn input_hud_configured_mode(&self) -> InputHudMode {
        self.input_hud.configured_mode()
    }

    /// The source currently feeding the HUD.
    pub fn input_hud_active_source(&self) -> InputHudActiveSource {
        self.input_hud.active_source()
    }

    /// Screen anchor for the chip row.
    pub fn input_hud_position(&self) -> InputHudPosition {
        self.input_hud.position()
    }

    /// Configured chip label size in points.
    pub fn input_hud_font_size(&self) -> f64 {
        self.input_hud.font_size()
    }

    /// Live chips, oldest first.
    pub fn input_hud_entries(&self) -> std::collections::vec_deque::Iter<'_, InputHudEntry> {
        self.input_hud.entries()
    }

    /// Fade factor for one chip at `now`.
    pub fn input_hud_alpha(&self, entry: &InputHudEntry, now: Instant) -> f64 {
        self.input_hud.alpha(entry, now)
    }

    /// Whether the HUD would draw anything this frame; the render, damage, and
    /// animation gates all read this so they can never disagree.
    pub fn input_hud_visible(&self) -> bool {
        self.input_hud.enabled() && self.input_hud.has_entries()
    }

    /// Point the HUD at a capture source; returns true when it changed.
    pub fn set_input_hud_source(&mut self, source: InputHudActiveSource) -> bool {
        if self.input_hud.set_active_source(source) {
            self.input_hud.clear(&mut self.dirty_tracker);
            self.needs_redraw = true;
            true
        } else {
            false
        }
    }

    /// Take the pending source announcement set by a runtime enable; the
    /// backend consumes it after reconciling the reader thread.
    pub fn take_input_hud_source_announce(&mut self) -> bool {
        self.input_hud.take_source_announce()
    }

    /// Toggle the input HUD and mark the frame for redraw.
    pub fn toggle_input_hud(&mut self) -> bool {
        let enabled = self.input_hud.toggle(&mut self.dirty_tracker);
        self.needs_redraw = true;
        enabled
    }

    /// Force the HUD on or off (presenter mode entry/exit); returns whether
    /// the enabled state changed.
    pub fn set_input_hud_enabled(&mut self, enabled: bool) -> bool {
        if self.input_hud.set_enabled(enabled, &mut self.dirty_tracker) {
            self.needs_redraw = true;
            true
        } else {
            false
        }
    }

    /// Drop every chip without changing the enabled flag.
    pub fn clear_input_hud(&mut self) {
        if self.input_hud.has_entries() {
            self.input_hud.clear(&mut self.dirty_tracker);
            self.needs_redraw = true;
        }
    }

    /// Report a key press received by wayscriber's own surfaces.
    ///
    /// Cheap no-op while the HUD is disabled or while the system monitor owns
    /// the reporting, so the keyboard handler can call it unconditionally.
    pub fn note_input_hud_key(&mut self, key: Key, modifiers: Modifiers) {
        if !self.input_hud.enabled()
            || self.input_hud.active_source() != InputHudActiveSource::Overlay
        {
            return;
        }
        let Some(label) = input_hud_key_label(key, modifiers) else {
            return;
        };
        if self.input_hud.note_key(
            InputHudActiveSource::Overlay,
            label,
            is_bare_modifier(key),
            Instant::now(),
        ) {
            self.needs_redraw = true;
        }
    }

    /// Report a pointer button press received by wayscriber's own surfaces.
    pub fn note_input_hud_mouse(&mut self, button: &str, modifiers: Modifiers) {
        if !self.input_hud.enabled()
            || self.input_hud.active_source() != InputHudActiveSource::Overlay
        {
            return;
        }
        let label = input_hud_mouse_label(button, modifiers);
        if self
            .input_hud
            .note_mouse(InputHudActiveSource::Overlay, label, Instant::now())
        {
            self.needs_redraw = true;
        }
    }

    /// Report a scroll tick received by wayscriber's own surfaces.
    pub fn note_input_hud_scroll(&mut self, up: bool, modifiers: Modifiers) {
        if !self.input_hud.enabled()
            || self.input_hud.active_source() != InputHudActiveSource::Overlay
        {
            return;
        }
        let label = input_hud_scroll_label(up, modifiers);
        if self
            .input_hud
            .note_scroll(InputHudActiveSource::Overlay, label, Instant::now())
        {
            self.needs_redraw = true;
        }
    }

    /// Report a chip already translated by the system monitor thread.
    ///
    /// Labels arrive fully formatted so no raw keysym ever reaches this side
    /// of the channel; the HUD never logs them.
    #[cfg_attr(not(feature = "input-monitor"), allow(dead_code))]
    pub fn note_input_hud_system_key(&mut self, label: String, bare_modifier: bool) {
        if self.input_hud.note_key(
            InputHudActiveSource::System,
            label,
            bare_modifier,
            Instant::now(),
        ) {
            self.needs_redraw = true;
        }
    }

    /// Report a system-monitor pointer button.
    #[cfg_attr(not(feature = "input-monitor"), allow(dead_code))]
    pub fn note_input_hud_system_mouse(&mut self, label: String) {
        if self
            .input_hud
            .note_mouse(InputHudActiveSource::System, label, Instant::now())
        {
            self.needs_redraw = true;
        }
    }

    /// Report a system-monitor scroll tick.
    #[cfg_attr(not(feature = "input-monitor"), allow(dead_code))]
    pub fn note_input_hud_system_scroll(&mut self, label: String) {
        if self
            .input_hud
            .note_scroll(InputHudActiveSource::System, label, Instant::now())
        {
            self.needs_redraw = true;
        }
    }

    /// Advance chip fades; returns true while chips remain on screen.
    pub fn advance_input_hud(&mut self, now: Instant) -> bool {
        let had_entries = self.input_hud.has_entries();
        let alive = self.input_hud.advance(now);
        if had_entries && !alive {
            // The last chip just left the row; the effect-damage collector
            // needs one more frame to clean its footprint.
            self.needs_redraw = true;
        }
        alive
    }
}
