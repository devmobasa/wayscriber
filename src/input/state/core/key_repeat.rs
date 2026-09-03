use std::time::{Duration, Instant};

use crate::input::Key;

/// Keyboard-repeat schedule shared by keyboard-driven modal panels.
#[derive(Debug, Default)]
pub(crate) struct OverlayKeyRepeat {
    key: Option<Key>,
    next_tick: Option<Instant>,
    started: Option<Instant>,
}

impl OverlayKeyRepeat {
    pub(crate) fn start(&mut self, key: Key, now: Instant, initial_delay: Duration) {
        if self.key == Some(key) {
            return;
        }
        self.key = Some(key);
        self.started = Some(now);
        self.next_tick = Some(now + initial_delay);
    }

    pub(crate) fn clear(&mut self) {
        self.key = None;
        self.next_tick = None;
        self.started = None;
    }

    pub(crate) fn release(&mut self, key: Key) {
        if self.key == Some(key) {
            self.clear();
        }
    }

    pub(crate) fn timeout(&self, active: bool, now: Instant) -> Option<Duration> {
        if !active {
            return None;
        }
        self.next_tick
            .map(|next| next.saturating_duration_since(now))
    }

    pub(crate) fn due_key(&self, now: Instant) -> Option<Key> {
        if now < self.next_tick? {
            return None;
        }
        self.key
    }

    pub(crate) fn schedule_fixed(&mut self, now: Instant, interval: Duration) {
        self.next_tick = Some(now + interval);
    }

    pub(crate) fn schedule_ramped(
        &mut self,
        now: Instant,
        initial_delay: Duration,
        slow_interval: Duration,
        fast_interval: Duration,
        ramp: Duration,
    ) {
        let interval = self.started.map_or(slow_interval, |started| {
            let repeating = now
                .saturating_duration_since(started)
                .saturating_sub(initial_delay);
            let progress = (repeating.as_secs_f64() / ramp.as_secs_f64()).clamp(0.0, 1.0);
            Duration::from_secs_f64(
                slow_interval.as_secs_f64()
                    + (fast_interval.as_secs_f64() - slow_interval.as_secs_f64()) * progress,
            )
        });
        self.next_tick = Some(now + interval);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_clears_only_the_held_key() {
        let now = Instant::now();
        let mut repeat = OverlayKeyRepeat::default();
        repeat.start(Key::Down, now, Duration::from_millis(100));
        repeat.release(Key::Up);
        assert_eq!(
            repeat.due_key(now + Duration::from_millis(100)),
            Some(Key::Down)
        );
        repeat.release(Key::Down);
        assert_eq!(repeat.timeout(true, now), None);
    }
}
