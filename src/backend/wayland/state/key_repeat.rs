use std::collections::HashMap;
use std::time::{Duration, Instant};

use wayland_client::{Proxy, protocol::wl_keyboard};

use crate::backend::wayland::handlers::keyboard::{KEY_REPEAT_INITIAL_DELAY, KEY_REPEAT_INTERVAL};
use crate::input::Key;

/// When a held key starts repeating, and how often it repeats after that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) struct KeyRepeatTiming {
    pub(in crate::backend::wayland) delay: Duration,
    pub(in crate::backend::wayland) interval: Duration,
}

impl KeyRepeatTiming {
    /// Used until the seat sends its own repeat settings.
    pub(in crate::backend::wayland) const FALLBACK: Self = Self {
        delay: KEY_REPEAT_INITIAL_DELAY,
        interval: KEY_REPEAT_INTERVAL,
    };
}

/// The `wl_keyboard` a key event came from.
///
/// Each keyboard reports its own `repeat_info`, and a seat other than the one
/// being typed on can report different settings, so a held key repeats with
/// the settings of the keyboard that pressed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::backend::wayland) struct KeyboardId(u32);

impl KeyboardId {
    pub(in crate::backend::wayland) fn of(keyboard: &wl_keyboard::WlKeyboard) -> Self {
        Self(keyboard.id().protocol_id())
    }

    #[cfg(test)]
    pub(in crate::backend::wayland) const fn for_test(id: u32) -> Self {
        Self(id)
    }
}

/// The key being held for repeat and the keyboard that pressed it.
struct HeldKey {
    key: Key,
    keyboard: KeyboardId,
    next_tick: Instant,
}

#[derive(Default)]
pub(in crate::backend::wayland) struct KeyRepeatState {
    held: Option<HeldKey>,
    /// Each keyboard's reported settings; `None` when that keyboard turned key
    /// repeat off. A keyboard that has not reported yet uses the fallback.
    timings: HashMap<KeyboardId, Option<KeyRepeatTiming>>,
}

impl KeyRepeatState {
    /// Adopt `keyboard`'s repeat settings. `None` turns repeat off for that
    /// keyboard and stops a key it is repeating now; a key held on another
    /// keyboard is unaffected. A held key picks up a new interval at its next
    /// repeat.
    pub(in crate::backend::wayland) fn set_timing(
        &mut self,
        keyboard: KeyboardId,
        timing: Option<KeyRepeatTiming>,
    ) {
        self.timings.insert(keyboard, timing);
        if timing.is_none() && self.held_by(keyboard) {
            self.clear();
        }
    }

    /// Drop a released keyboard's settings, and stop a key it was repeating.
    pub(in crate::backend::wayland) fn forget_keyboard(&mut self, keyboard: KeyboardId) {
        self.timings.remove(&keyboard);
        if self.held_by(keyboard) {
            self.clear();
        }
    }

    /// Start repeating `key` after `keyboard`'s delay, unless that keyboard
    /// turned repeat off.
    pub(in crate::backend::wayland) fn arm(
        &mut self,
        key: Key,
        keyboard: KeyboardId,
        now: Instant,
    ) {
        let Some(timing) = self.timing_for(keyboard) else {
            self.clear();
            return;
        };

        self.held = Some(HeldKey {
            key,
            keyboard,
            next_tick: now + timing.delay,
        });
    }

    pub(in crate::backend::wayland) fn clear(&mut self) {
        self.held = None;
    }

    pub(in crate::backend::wayland) fn clear_if_released(&mut self, key: Key) {
        if self.held.as_ref().is_some_and(|held| held.key == key) {
            self.clear();
        }
    }

    pub(in crate::backend::wayland) fn timeout(
        &self,
        now: Instant,
        can_repeat: bool,
    ) -> Option<Duration> {
        can_repeat
            .then(|| {
                self.held
                    .as_ref()
                    .map(|held| held.next_tick.saturating_duration_since(now))
            })
            .flatten()
    }

    pub(in crate::backend::wayland) fn take_due(
        &mut self,
        now: Instant,
        can_repeat: bool,
    ) -> Option<Key> {
        let keyboard = self.held.as_ref()?.keyboard;
        let Some(timing) = self.timing_for(keyboard).filter(|_| can_repeat) else {
            self.clear();
            return None;
        };
        let held = self.held.as_mut()?;
        if now < held.next_tick {
            return None;
        }

        held.next_tick = now + timing.interval;
        Some(held.key)
    }

    fn timing_for(&self, keyboard: KeyboardId) -> Option<KeyRepeatTiming> {
        self.timings
            .get(&keyboard)
            .copied()
            .unwrap_or(Some(KeyRepeatTiming::FALLBACK))
    }

    fn held_by(&self, keyboard: KeyboardId) -> bool {
        self.held
            .as_ref()
            .is_some_and(|held| held.keyboard == keyboard)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{KeyRepeatState, KeyRepeatTiming, KeyboardId};
    use crate::input::Key;

    const ACTIVE: KeyboardId = KeyboardId::for_test(3);
    const OTHER_SEAT: KeyboardId = KeyboardId::for_test(7);

    fn timing(delay_ms: u64, interval_ms: u64) -> Option<KeyRepeatTiming> {
        Some(KeyRepeatTiming {
            delay: Duration::from_millis(delay_ms),
            interval: Duration::from_millis(interval_ms),
        })
    }

    #[test]
    fn due_repeat_reschedules_without_burst_catch_up() {
        let start = Instant::now();
        let mut state = KeyRepeatState::default();
        state.set_timing(ACTIVE, timing(400, 40));
        state.arm(Key::Left, ACTIVE, start);

        let delayed_tick = start + Duration::from_secs(1);
        assert_eq!(state.take_due(delayed_tick, true), Some(Key::Left));
        assert_eq!(
            state.timeout(delayed_tick, true),
            Some(Duration::from_millis(40))
        );
    }

    #[test]
    fn blocked_repeat_clears_the_held_key() {
        let start = Instant::now();
        let mut state = KeyRepeatState::default();
        state.set_timing(ACTIVE, timing(0, 0));
        state.arm(Key::Left, ACTIVE, start);

        assert_eq!(state.take_due(start, false), None);
        assert_eq!(state.take_due(start, true), None);
    }

    #[test]
    fn repeat_waits_the_fallback_delay_until_the_seat_reports_one() {
        let start = Instant::now();
        let mut state = KeyRepeatState::default();
        state.arm(Key::Left, ACTIVE, start);

        assert_eq!(
            state.timeout(start, true),
            Some(KeyRepeatTiming::FALLBACK.delay)
        );
    }

    #[test]
    fn repeat_follows_the_seat_delay_and_interval() {
        let start = Instant::now();
        let mut state = KeyRepeatState::default();
        state.set_timing(ACTIVE, timing(250, 16));
        state.arm(Key::Backspace, ACTIVE, start);

        assert_eq!(state.timeout(start, true), Some(Duration::from_millis(250)));
        let first = start + Duration::from_millis(250);
        assert_eq!(state.take_due(first, true), Some(Key::Backspace));
        assert_eq!(state.timeout(first, true), Some(Duration::from_millis(16)));
    }

    #[test]
    fn a_seat_with_repeat_disabled_never_repeats() {
        let start = Instant::now();
        let mut state = KeyRepeatState::default();
        state.set_timing(ACTIVE, None);
        state.arm(Key::Char('a'), ACTIVE, start);

        assert_eq!(state.timeout(start, true), None);
        assert_eq!(state.take_due(start + Duration::from_secs(5), true), None);
    }

    #[test]
    fn disabling_repeat_stops_a_key_that_is_repeating() {
        let start = Instant::now();
        let mut state = KeyRepeatState::default();
        state.arm(Key::Char('a'), ACTIVE, start);

        state.set_timing(ACTIVE, None);

        assert_eq!(state.timeout(start, true), None);
        assert_eq!(state.take_due(start + Duration::from_secs(5), true), None);
    }

    #[test]
    fn another_seat_disabling_repeat_leaves_the_active_keyboard_repeating() {
        let start = Instant::now();
        let mut state = KeyRepeatState::default();
        state.set_timing(ACTIVE, timing(250, 16));
        state.set_timing(OTHER_SEAT, None);

        state.arm(Key::Backspace, ACTIVE, start);
        state.set_timing(OTHER_SEAT, None);

        let first = start + Duration::from_millis(250);
        assert_eq!(state.take_due(first, true), Some(Key::Backspace));
        assert_eq!(state.timeout(first, true), Some(Duration::from_millis(16)));
    }

    #[test]
    fn each_keyboard_repeats_with_its_own_settings() {
        let start = Instant::now();
        let mut state = KeyRepeatState::default();
        state.set_timing(ACTIVE, timing(250, 16));
        state.set_timing(OTHER_SEAT, timing(600, 100));

        state.arm(Key::Char('a'), OTHER_SEAT, start);

        assert_eq!(state.timeout(start, true), Some(Duration::from_millis(600)));
        let first = start + Duration::from_millis(600);
        assert_eq!(state.take_due(first, true), Some(Key::Char('a')));
        assert_eq!(state.timeout(first, true), Some(Duration::from_millis(100)));
    }

    #[test]
    fn a_keyboard_with_repeat_disabled_does_not_repeat_even_if_another_does() {
        let start = Instant::now();
        let mut state = KeyRepeatState::default();
        state.set_timing(ACTIVE, timing(250, 16));
        state.set_timing(OTHER_SEAT, None);

        state.arm(Key::Char('a'), OTHER_SEAT, start);

        assert_eq!(state.timeout(start, true), None);
    }

    #[test]
    fn releasing_a_keyboard_stops_its_repeat_and_forgets_its_settings() {
        let start = Instant::now();
        let mut state = KeyRepeatState::default();
        state.set_timing(ACTIVE, None);
        state.arm(Key::Left, ACTIVE, start);
        state.set_timing(OTHER_SEAT, timing(250, 16));
        state.arm(Key::Right, OTHER_SEAT, start);

        state.forget_keyboard(OTHER_SEAT);
        assert_eq!(state.timeout(start, true), None);

        state.forget_keyboard(ACTIVE);
        state.arm(Key::Left, ACTIVE, start);
        assert_eq!(
            state.timeout(start, true),
            Some(KeyRepeatTiming::FALLBACK.delay)
        );
    }
}
