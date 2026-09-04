use std::time::{Duration, Instant};

/// Scheduling state for runtime UI animations.
pub(in crate::backend::wayland) struct UiAnimationClock {
    interval: Option<Duration>,
    next_tick: Option<Instant>,
}

impl UiAnimationClock {
    pub(in crate::backend::wayland) fn from_fps(fps: u32) -> Self {
        Self {
            interval: (fps != 0).then(|| Duration::from_secs_f64(1.0 / fps as f64)),
            next_tick: None,
        }
    }

    pub(in crate::backend::wayland) fn schedule(&mut self, now: Instant, active: bool) {
        self.next_tick = if active {
            self.interval.map(|interval| now + interval)
        } else {
            None
        };
    }

    pub(in crate::backend::wayland) fn timeout(&self, now: Instant) -> Option<Duration> {
        self.interval?;
        self.next_tick
            .map(|next| next.saturating_duration_since(now))
    }

    pub(in crate::backend::wayland) fn is_due(&self, now: Instant) -> bool {
        self.interval.is_some() && self.next_tick.is_some_and(|next| now >= next)
    }

    pub(in crate::backend::wayland) fn is_uncapped(&self) -> bool {
        self.interval.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capped_clock_schedules_and_reports_its_deadline() {
        let now = Instant::now();
        let mut clock = UiAnimationClock::from_fps(20);

        clock.schedule(now, true);

        assert_eq!(clock.timeout(now), Some(Duration::from_millis(50)));
        assert!(!clock.is_due(now + Duration::from_millis(49)));
        assert!(clock.is_due(now + Duration::from_millis(50)));
        assert_eq!(
            clock.timeout(now + Duration::from_millis(51)),
            Some(Duration::ZERO)
        );
    }

    #[test]
    fn inactive_animation_clears_a_scheduled_tick() {
        let now = Instant::now();
        let mut clock = UiAnimationClock::from_fps(60);
        clock.schedule(now, true);

        clock.schedule(now, false);

        assert_eq!(clock.timeout(now), None);
        assert!(!clock.is_due(now + Duration::from_secs(1)));
    }

    #[test]
    fn zero_fps_is_uncapped_and_never_schedules_a_timeout() {
        let now = Instant::now();
        let mut clock = UiAnimationClock::from_fps(0);

        clock.schedule(now, true);

        assert!(clock.is_uncapped());
        assert_eq!(clock.timeout(now), None);
        assert!(!clock.is_due(now + Duration::from_secs(1)));
    }
}
