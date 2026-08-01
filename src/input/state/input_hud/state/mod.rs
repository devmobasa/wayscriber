use std::collections::VecDeque;
use std::time::Instant;

use crate::config::{InputHudMode, InputHudPosition};
use crate::draw::DirtyTracker;

use super::settings::InputHudSettings;

/// Which chrome a chip is drawn with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputHudEntryKind {
    /// Keyboard chord — drawn as a keycap.
    Key,
    /// Pointer button — drawn as a rounded pill.
    Mouse,
    /// Scroll tick — drawn as a rounded pill.
    Scroll,
}

/// Which capture source is currently feeding the HUD.
///
/// The system monitor sees every press on the seat, including the ones the
/// overlay's own surfaces receive, so while it is active the overlay-side
/// hooks must stay silent or every press would be reported twice. System is a
/// strict superset of overlay, so suppression needs no de-dup heuristic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputHudActiveSource {
    /// Only wayscriber's own surfaces report input.
    #[default]
    Overlay,
    /// A libinput reader thread reports all seat input.
    System,
}

/// One chip in the row: a chord label plus its repeat counter and fade clock.
#[derive(Debug, Clone)]
pub struct InputHudEntry {
    label: String,
    kind: InputHudEntryKind,
    count: u32,
    /// Last press or repeat — the anchor the hold/fade clock runs from.
    refreshed_at: Instant,
}

impl InputHudEntry {
    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn kind(&self) -> InputHudEntryKind {
        self.kind
    }

    pub fn count(&self) -> u32 {
        self.count
    }
}

/// The input HUD's owned model: the live chip row plus the settings that
/// govern filtering, coalescing, and the fade clock.
///
/// Held by value on `InputState` and mutated only by the thread that owns it,
/// exactly like `ClickHighlightState`.
#[derive(Clone)]
pub struct InputHudState {
    settings: InputHudSettings,
    enabled: bool,
    /// Oldest chip first; the row renders left to right, so new chips push the
    /// older ones toward the row's start.
    entries: VecDeque<InputHudEntry>,
    source: InputHudActiveSource,
    /// Set on every runtime enable; consumed by the backend after it
    /// reconciles the reader thread, where the effective source is actually
    /// known. Deliberately not set by construction or `apply_settings`, so a
    /// config-enabled HUD does not toast on every overlay show.
    announce_source_pending: bool,
}

impl InputHudState {
    pub fn new(settings: InputHudSettings) -> Self {
        let enabled = settings.enabled;
        Self {
            settings,
            enabled,
            entries: VecDeque::new(),
            source: InputHudActiveSource::Overlay,
            announce_source_pending: false,
        }
    }

    /// Replace the settings snapshot (startup config application). The live
    /// enabled flag follows the new snapshot; any chips from the old settings
    /// are dropped because their fade clock no longer applies.
    pub fn apply_settings(&mut self, settings: InputHudSettings) {
        self.enabled = settings.enabled;
        self.settings = settings;
        self.entries.clear();
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn configured_mode(&self) -> InputHudMode {
        self.settings.mode
    }

    pub fn position(&self) -> InputHudPosition {
        self.settings.position
    }

    pub fn font_size(&self) -> f64 {
        self.settings.font_size
    }

    pub fn active_source(&self) -> InputHudActiveSource {
        self.source
    }

    /// Point the HUD at a capture source. Returns true when the source
    /// changed, so callers can announce the effective mode.
    pub fn set_active_source(&mut self, source: InputHudActiveSource) -> bool {
        if self.source == source {
            return false;
        }
        self.source = source;
        true
    }

    pub fn has_entries(&self) -> bool {
        !self.entries.is_empty()
    }

    pub fn entries(&self) -> std::collections::vec_deque::Iter<'_, InputHudEntry> {
        self.entries.iter()
    }

    /// Fade factor for one chip: fully opaque for `display`, then a linear
    /// ramp to zero over `fade`.
    pub fn alpha(&self, entry: &InputHudEntry, now: Instant) -> f64 {
        let elapsed = now.saturating_duration_since(entry.refreshed_at);
        if elapsed <= self.settings.display {
            return 1.0;
        }
        let fade = self.settings.fade.as_secs_f64();
        if fade <= 0.0 {
            return 0.0;
        }
        let faded = (elapsed - self.settings.display).as_secs_f64();
        (1.0 - faded / fade).clamp(0.0, 1.0)
    }

    /// Flip the HUD on or off. Turning it off clears the row so a later
    /// re-enable never resurrects stale chips.
    pub fn toggle(&mut self, tracker: &mut DirtyTracker) -> bool {
        self.enabled = !self.enabled;
        if self.enabled {
            self.announce_source_pending = true;
        } else {
            self.entries.clear();
            self.announce_source_pending = false;
        }
        tracker.mark_full();
        self.enabled
    }

    /// Whether a source announcement is waiting, without consuming it.
    pub fn has_source_announce(&self) -> bool {
        self.announce_source_pending
    }

    /// Take the pending "announce the effective source" request set by a
    /// runtime enable. The backend consumes this after `sync_input_monitor`
    /// has reconciled the reader thread, so the announcement can never claim
    /// a source the HUD does not actually have.
    pub fn take_source_announce(&mut self) -> bool {
        std::mem::take(&mut self.announce_source_pending)
    }

    /// Force the HUD to a given enabled state (presenter mode entry/exit).
    /// Returns true when the state changed.
    pub fn set_enabled(&mut self, enabled: bool, tracker: &mut DirtyTracker) -> bool {
        if self.enabled == enabled {
            return false;
        }
        let _ = self.toggle(tracker);
        true
    }

    /// Drop every chip without changing the enabled flag.
    pub fn clear(&mut self, tracker: &mut DirtyTracker) {
        if self.entries.is_empty() {
            return;
        }
        self.entries.clear();
        tracker.mark_full();
    }

    /// Record a key chord. `bare_modifier` marks presses of Ctrl/Shift/Alt on
    /// their own, which `show_bare_modifiers` can filter out.
    pub fn note_key(
        &mut self,
        source: InputHudActiveSource,
        label: String,
        bare_modifier: bool,
        now: Instant,
    ) -> bool {
        if bare_modifier && !self.settings.show_bare_modifiers {
            return false;
        }
        self.note(source, InputHudEntryKind::Key, label, now)
    }

    /// Record a pointer button press.
    pub fn note_mouse(
        &mut self,
        source: InputHudActiveSource,
        label: String,
        now: Instant,
    ) -> bool {
        if !self.settings.show_mouse {
            return false;
        }
        self.note(source, InputHudEntryKind::Mouse, label, now)
    }

    /// Record a scroll tick. Consecutive ticks in the same direction coalesce
    /// into one chip's counter.
    pub fn note_scroll(
        &mut self,
        source: InputHudActiveSource,
        label: String,
        now: Instant,
    ) -> bool {
        if !self.settings.show_mouse {
            return false;
        }
        self.note(source, InputHudEntryKind::Scroll, label, now)
    }

    fn note(
        &mut self,
        source: InputHudActiveSource,
        kind: InputHudEntryKind,
        label: String,
        now: Instant,
    ) -> bool {
        if !self.enabled || source != self.source {
            return false;
        }

        let coalesce = self.settings.combine_repeats
            && self.entries.back().is_some_and(|newest| {
                newest.kind == kind && newest.label == label && self.alpha_at(newest, now) > 0.0
            });
        if coalesce && let Some(newest) = self.entries.back_mut() {
            newest.count = newest.count.saturating_add(1);
            newest.refreshed_at = now;
            return true;
        }

        self.entries.push_back(InputHudEntry {
            label,
            kind,
            count: 1,
            refreshed_at: now,
        });
        while self.entries.len() > self.settings.max_entries {
            self.entries.pop_front();
        }
        true
    }

    /// Drop fully-faded chips; returns whether any chip is still on screen (and
    /// therefore whether the UI animation clock must keep ticking).
    pub fn advance(&mut self, now: Instant) -> bool {
        if self.entries.is_empty() {
            return false;
        }
        let display = self.settings.display;
        let fade = self.settings.fade;
        self.entries.retain(|entry| {
            let elapsed = now.saturating_duration_since(entry.refreshed_at);
            elapsed <= display || (fade > std::time::Duration::ZERO && elapsed < display + fade)
        });
        !self.entries.is_empty()
    }

    /// Borrow-friendly alpha used inside `&mut self` methods.
    fn alpha_at(&self, entry: &InputHudEntry, now: Instant) -> f64 {
        let elapsed = now.saturating_duration_since(entry.refreshed_at);
        if elapsed <= self.settings.display {
            return 1.0;
        }
        let fade = self.settings.fade.as_secs_f64();
        if fade <= 0.0 {
            return 0.0;
        }
        let faded = (elapsed - self.settings.display).as_secs_f64();
        (1.0 - faded / fade).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::config::InputHudConfig;

    fn settings(enabled: bool) -> InputHudSettings {
        InputHudSettings::from(&InputHudConfig {
            enabled,
            ..InputHudConfig::default()
        })
    }

    fn enabled_state() -> InputHudState {
        InputHudState::new(settings(true))
    }

    fn note_key(state: &mut InputHudState, label: &str, now: Instant) -> bool {
        state.note_key(InputHudActiveSource::Overlay, label.to_string(), false, now)
    }

    #[test]
    fn disabled_hud_ignores_notes() {
        let mut state = InputHudState::new(settings(false));
        assert!(!note_key(&mut state, "A", Instant::now()));
        assert!(!state.has_entries());
    }

    #[test]
    fn toggle_enables_and_clearing_disable_drops_chips() {
        let mut state = InputHudState::new(settings(false));
        let mut tracker = DirtyTracker::new();
        assert!(state.toggle(&mut tracker));
        assert!(note_key(&mut state, "A", Instant::now()));
        assert!(!state.toggle(&mut tracker));
        assert!(!state.has_entries());
    }

    #[test]
    fn pushing_past_max_entries_evicts_the_oldest() {
        let mut state = InputHudState::new(InputHudSettings::from(&InputHudConfig {
            enabled: true,
            max_entries: 3,
            ..InputHudConfig::default()
        }));
        let now = Instant::now();
        for label in ["A", "B", "C", "D"] {
            assert!(note_key(&mut state, label, now));
        }
        let labels: Vec<_> = state.entries().map(|entry| entry.label()).collect();
        assert_eq!(labels, vec!["B", "C", "D"]);
    }

    #[test]
    fn repeats_coalesce_into_a_counter_when_enabled() {
        let mut state = enabled_state();
        let now = Instant::now();
        assert!(note_key(&mut state, "Backspace", now));
        assert!(note_key(&mut state, "Backspace", now));
        assert!(note_key(&mut state, "Backspace", now));
        assert_eq!(state.entries().len(), 1);
        assert_eq!(state.entries().next().map(|entry| entry.count()), Some(3));
    }

    #[test]
    fn repeats_stay_separate_chips_when_coalescing_is_off() {
        let mut state = InputHudState::new(InputHudSettings::from(&InputHudConfig {
            enabled: true,
            combine_repeats: false,
            ..InputHudConfig::default()
        }));
        let now = Instant::now();
        assert!(note_key(&mut state, "A", now));
        assert!(note_key(&mut state, "A", now));
        assert_eq!(state.entries().len(), 2);
    }

    #[test]
    fn bare_modifier_filter_respects_the_setting() {
        let mut shown = enabled_state();
        assert!(shown.note_key(
            InputHudActiveSource::Overlay,
            "Ctrl".to_string(),
            true,
            Instant::now()
        ));

        let mut hidden = InputHudState::new(InputHudSettings::from(&InputHudConfig {
            enabled: true,
            show_bare_modifiers: false,
            ..InputHudConfig::default()
        }));
        assert!(!hidden.note_key(
            InputHudActiveSource::Overlay,
            "Ctrl".to_string(),
            true,
            Instant::now()
        ));
        assert!(!hidden.has_entries());
    }

    #[test]
    fn mouse_filter_respects_the_setting() {
        let mut hidden = InputHudState::new(InputHudSettings::from(&InputHudConfig {
            enabled: true,
            show_mouse: false,
            ..InputHudConfig::default()
        }));
        assert!(!hidden.note_mouse(
            InputHudActiveSource::Overlay,
            "Click".to_string(),
            Instant::now()
        ));
        assert!(!hidden.note_scroll(
            InputHudActiveSource::Overlay,
            "Scroll \u{2191}".to_string(),
            Instant::now()
        ));
    }

    #[test]
    fn advance_drops_fully_faded_chips_and_reports_liveness() {
        let mut state = InputHudState::new(InputHudSettings::from(&InputHudConfig {
            enabled: true,
            display_ms: 200,
            fade_ms: 200,
            ..InputHudConfig::default()
        }));
        let start = Instant::now();
        assert!(note_key(&mut state, "A", start));

        assert!(state.advance(start + Duration::from_millis(100)));
        assert!(state.has_entries());
        assert!(state.advance(start + Duration::from_millis(300)));
        assert!(!state.advance(start + Duration::from_millis(500)));
        assert!(!state.has_entries());
    }

    #[test]
    fn alpha_holds_then_ramps_to_zero() {
        let state = InputHudState::new(InputHudSettings::from(&InputHudConfig {
            enabled: true,
            display_ms: 1000,
            fade_ms: 400,
            ..InputHudConfig::default()
        }));
        let start = Instant::now();
        let entry = InputHudEntry {
            label: "A".to_string(),
            kind: InputHudEntryKind::Key,
            count: 1,
            refreshed_at: start,
        };

        assert_eq!(state.alpha(&entry, start), 1.0);
        assert_eq!(
            state.alpha(&entry, start + Duration::from_millis(1000)),
            1.0
        );
        let mid = state.alpha(&entry, start + Duration::from_millis(1200));
        assert!((mid - 0.5).abs() < 1e-9);
        assert_eq!(
            state.alpha(&entry, start + Duration::from_millis(1400)),
            0.0
        );
    }

    #[test]
    fn system_source_suppresses_overlay_notes() {
        let mut state = enabled_state();
        assert!(state.set_active_source(InputHudActiveSource::System));
        assert!(!note_key(&mut state, "A", Instant::now()));
        assert!(state.note_key(
            InputHudActiveSource::System,
            "A".to_string(),
            false,
            Instant::now()
        ));
        assert_eq!(state.entries().len(), 1);
    }
}
