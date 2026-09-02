use std::time::{Duration, Instant};

use crate::input::Key;

#[derive(Default)]
pub(in crate::backend::wayland) struct KeyRepeatState {
    key: Option<Key>,
    next_tick: Option<Instant>,
}

impl KeyRepeatState {
    pub(in crate::backend::wayland) fn arm(&mut self, key: Key, now: Instant, delay: Duration) {
        self.key = Some(key);
        self.next_tick = Some(now + delay);
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
        interval: Duration,
    ) -> Option<Key> {
        if !can_repeat {
            self.clear();
            return None;
        }
        let key = self.key?;
        if now < self.next_tick? {
            return None;
        }
        self.next_tick = Some(now + interval);
        Some(key)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::KeyRepeatState;
    use crate::input::Key;

    #[test]
    fn due_repeat_reschedules_without_burst_catch_up() {
        let start = Instant::now();
        let mut state = KeyRepeatState::default();
        state.arm(Key::Left, start, Duration::from_millis(400));

        let delayed_tick = start + Duration::from_secs(1);
        assert_eq!(
            state.take_due(delayed_tick, true, Duration::from_millis(40)),
            Some(Key::Left)
        );
        assert_eq!(
            state.timeout(delayed_tick, true),
            Some(Duration::from_millis(40))
        );
    }

    #[test]
    fn blocked_repeat_clears_the_held_key() {
        let start = Instant::now();
        let mut state = KeyRepeatState::default();
        state.arm(Key::Left, start, Duration::ZERO);

        assert_eq!(
            state.take_due(start, false, Duration::from_millis(40)),
            None
        );
        assert_eq!(state.take_due(start, true, Duration::ZERO), None);
    }
}
