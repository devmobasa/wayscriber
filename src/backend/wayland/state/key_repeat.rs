use std::time::{Duration, Instant};

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

pub(in crate::backend::wayland) struct KeyRepeatState {
    key: Option<Key>,
    next_tick: Option<Instant>,
    /// `None` when the seat has turned key repeat off.
    timing: Option<KeyRepeatTiming>,
}

impl Default for KeyRepeatState {
    fn default() -> Self {
        Self {
            key: None,
            next_tick: None,
            timing: Some(KeyRepeatTiming::FALLBACK),
        }
    }
}

impl KeyRepeatState {
    /// Adopt the seat's repeat settings. `None` turns repeat off and stops a
    /// key that is repeating now; a held key picks up a new interval at its
    /// next repeat.
    pub(in crate::backend::wayland) fn set_timing(&mut self, timing: Option<KeyRepeatTiming>) {
        self.timing = timing;
        if timing.is_none() {
            self.clear();
        }
    }

    /// Start repeating `key` after the seat's delay, unless repeat is off.
    pub(in crate::backend::wayland) fn arm(&mut self, key: Key, now: Instant) {
        let Some(timing) = self.timing else {
            self.clear();
            return;
        };

        self.key = Some(key);
        self.next_tick = Some(now + timing.delay);
    }

    pub(in crate::backend::wayland) fn clear(&mut self) {
        self.key = None;
        self.next_tick = None;
    }

    pub(in crate::backend::wayland) fn clear_if_released(&mut self, key: Key) {
        if self.key == Some(key) {
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
                self.next_tick
                    .map(|next| next.saturating_duration_since(now))
            })
            .flatten()
    }

    pub(in crate::backend::wayland) fn take_due(
        &mut self,
        now: Instant,
        can_repeat: bool,
    ) -> Option<Key> {
        let Some(timing) = self.timing.filter(|_| can_repeat) else {
            self.clear();
            return None;
        };
        let key = self.key?;
        if now < self.next_tick? {
            return None;
        }

        self.next_tick = Some(now + timing.interval);
        Some(key)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{KeyRepeatState, KeyRepeatTiming};
    use crate::input::Key;

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
        state.set_timing(timing(400, 40));
        state.arm(Key::Left, start);

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
        state.set_timing(timing(0, 0));
        state.arm(Key::Left, start);

        assert_eq!(state.take_due(start, false), None);
        assert_eq!(state.take_due(start, true), None);
    }

    #[test]
    fn repeat_waits_the_fallback_delay_until_the_seat_reports_one() {
        let start = Instant::now();
        let mut state = KeyRepeatState::default();
        state.arm(Key::Left, start);

        assert_eq!(
            state.timeout(start, true),
            Some(KeyRepeatTiming::FALLBACK.delay)
        );
    }

    #[test]
    fn repeat_follows_the_seat_delay_and_interval() {
        let start = Instant::now();
        let mut state = KeyRepeatState::default();
        state.set_timing(timing(250, 16));
        state.arm(Key::Backspace, start);

        assert_eq!(state.timeout(start, true), Some(Duration::from_millis(250)));
        let first = start + Duration::from_millis(250);
        assert_eq!(state.take_due(first, true), Some(Key::Backspace));
        assert_eq!(state.timeout(first, true), Some(Duration::from_millis(16)));
    }

    #[test]
    fn a_seat_with_repeat_disabled_never_repeats() {
        let start = Instant::now();
        let mut state = KeyRepeatState::default();
        state.set_timing(None);
        state.arm(Key::Char('a'), start);

        assert_eq!(state.timeout(start, true), None);
        assert_eq!(state.take_due(start + Duration::from_secs(5), true), None);
    }

    #[test]
    fn disabling_repeat_stops_a_key_that_is_repeating() {
        let start = Instant::now();
        let mut state = KeyRepeatState::default();
        state.arm(Key::Char('a'), start);

        state.set_timing(None);

        assert_eq!(state.timeout(start, true), None);
        assert_eq!(state.take_due(start + Duration::from_secs(5), true), None);
    }
}
