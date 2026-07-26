//! Shared animation envelopes and explicit reduced-motion policy.
//!
//! Chrome fades route through these helpers instead of hand-rolled piecewise
//! math so timing stays consistent and the `[ui] reduced_motion` setting can
//! disable non-essential animation in one place (WCAG 2.3.3). When motion is
//! disabled, envelopes snap to their resting value: transient chrome shows at
//! full opacity for its lifetime and disappears instantly.

use std::time::{Duration, Instant};

/// Cubic ease-out easing curve (pure math; not gated).
pub fn ease_out_cubic(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// A retargetable eased value: `retarget` starts a timed ease-out-cubic
/// transition from the current value to a new target; `advance` moves the
/// value along it. When motion is disabled (`[ui] reduced_motion`) both
/// operations snap directly to the target with no intermediate values.
#[derive(Debug, Clone)]
pub struct Envelope {
    current: f64,
    from: f64,
    target: f64,
    started: Option<Instant>,
    duration: Duration,
}

impl Envelope {
    pub fn new(value: f64) -> Self {
        Self {
            current: value,
            from: value,
            target: value,
            started: None,
            duration: Duration::ZERO,
        }
    }

    /// Current value without advancing time.
    pub fn value(&self) -> f64 {
        self.current
    }

    /// Whether the envelope has reached its target (no transition running).
    pub fn settled(&self) -> bool {
        self.started.is_none()
    }

    /// Begin easing toward `target` over `duration`. A no-op when `target`
    /// is already the active target; a retarget mid-flight eases from the
    /// current (partial) value. Snaps when motion is disabled.
    pub fn retarget(
        &mut self,
        target: f64,
        duration: Duration,
        now: Instant,
        motion_enabled: bool,
    ) {
        if !motion_enabled {
            self.current = target;
            self.target = target;
            self.started = None;
            return;
        }
        if target == self.target {
            return;
        }
        self.target = target;
        if self.current == target || duration.is_zero() {
            self.current = target;
            self.started = None;
            return;
        }
        self.from = self.current;
        self.started = Some(now);
        self.duration = duration;
    }

    /// Advance the running transition to `now` and return the new value.
    pub fn advance(&mut self, now: Instant, motion_enabled: bool) -> f64 {
        if !motion_enabled {
            self.current = self.target;
            self.started = None;
            return self.current;
        }
        let Some(started) = self.started else {
            return self.current;
        };
        let elapsed = now.saturating_duration_since(started).as_secs_f64();
        let progress = elapsed / self.duration.as_secs_f64();
        if progress >= 1.0 {
            self.current = self.target;
            self.started = None;
        } else {
            self.current = self.from + (self.target - self.from) * ease_out_cubic(progress);
        }
        self.current
    }
}

/// Fully opaque for `hold_ratio` of the lifetime, then a linear fade to zero
/// at `progress == 1.0`. Used by preset toasts.
pub fn hold_then_fade_out(progress: f64, hold_ratio: f64, motion_enabled: bool) -> f64 {
    if !motion_enabled {
        return if progress >= 1.0 { 0.0 } else { 1.0 };
    }
    if progress <= hold_ratio {
        1.0
    } else {
        let fade_progress = (progress - hold_ratio) / (1.0 - hold_ratio);
        (1.0 - fade_progress).clamp(0.0, 1.0)
    }
}

/// Opaque until the final `fade_secs` of `duration_secs`, then a linear fade
/// to zero. Used by UI toasts (fixed short end fade keeps long-lived toasts
/// crisp for nearly their full lifetime).
pub fn end_fade(
    elapsed_secs: f64,
    duration_secs: f64,
    fade_secs: f64,
    motion_enabled: bool,
) -> f64 {
    let fade_duration = duration_secs.min(fade_secs);
    if fade_duration <= 0.0 {
        return 0.0;
    }
    if !motion_enabled {
        return if elapsed_secs >= duration_secs {
            0.0
        } else {
            1.0
        };
    }
    let fade_start = duration_secs - fade_duration;
    if elapsed_secs <= fade_start {
        1.0
    } else {
        ((duration_secs - elapsed_secs) / fade_duration).clamp(0.0, 1.0)
    }
}

/// Attack/hold/release flash: ramp to `peak` by `attack_end`, hold until
/// `hold_end`, then release to zero at `progress == 1.0`. Used by the
/// blocked-action edge flash.
pub fn flash(
    progress: f64,
    attack_end: f64,
    hold_end: f64,
    peak: f64,
    motion_enabled: bool,
) -> f64 {
    if !motion_enabled {
        return if progress >= 1.0 { 0.0 } else { peak };
    }
    if progress < attack_end {
        (progress / attack_end) * peak
    } else if progress < hold_end {
        peak
    } else {
        peak * (1.0 - (progress - hold_end) / (1.0 - hold_end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ease_out_cubic_clamps_and_eases() {
        assert_eq!(ease_out_cubic(-1.0), 0.0);
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert!((ease_out_cubic(0.5) - 0.875).abs() < 1e-9);
        assert_eq!(ease_out_cubic(1.0), 1.0);
        assert_eq!(ease_out_cubic(2.0), 1.0);
    }

    #[test]
    fn flash_ramps_holds_and_releases() {
        assert!((flash(0.075, 0.15, 0.4, 0.22, true) - 0.11).abs() < 1e-9);
        assert!((flash(0.2, 0.15, 0.4, 0.22, true) - 0.22).abs() < 1e-9);
        assert!((flash(0.7, 0.15, 0.4, 0.22, true) - 0.11).abs() < 1e-9);
        assert!(flash(1.0, 0.15, 0.4, 0.22, true).abs() < 1e-9);
    }

    #[test]
    fn envelope_eases_toward_the_target_and_settles() {
        let start = Instant::now();
        let mut envelope = Envelope::new(1.0);
        assert!(envelope.settled());

        envelope.retarget(0.55, Duration::from_millis(300), start, true);
        assert!(!envelope.settled());
        assert_eq!(envelope.value(), 1.0, "retarget does not jump the value");

        let mid = envelope.advance(start + Duration::from_millis(150), true);
        let expected = 1.0 + (0.55 - 1.0) * ease_out_cubic(0.5);
        assert!((mid - expected).abs() < 1e-9);
        assert!(mid < 1.0 && mid > 0.55);

        assert_eq!(
            envelope.advance(start + Duration::from_millis(300), true),
            0.55
        );
        assert!(envelope.settled());
        // Once settled, advancing further holds the target.
        assert_eq!(envelope.advance(start + Duration::from_secs(9), true), 0.55);
    }

    #[test]
    fn envelope_retargets_mid_flight_from_the_partial_value() {
        let start = Instant::now();
        let mut envelope = Envelope::new(1.0);
        envelope.retarget(0.55, Duration::from_millis(300), start, true);
        let partial = envelope.advance(start + Duration::from_millis(100), true);
        assert!(partial < 1.0);

        let restart = start + Duration::from_millis(100);
        envelope.retarget(1.0, Duration::from_millis(150), restart, true);
        let mid = envelope.advance(restart + Duration::from_millis(75), true);
        assert!(mid > partial && mid < 1.0, "eases up from {partial}: {mid}");
        assert_eq!(
            envelope.advance(restart + Duration::from_millis(150), true),
            1.0
        );
        assert!(envelope.settled());
    }

    #[test]
    fn envelope_snaps_with_no_intermediate_values_under_reduced_motion() {
        let start = Instant::now();
        let mut envelope = Envelope::new(1.0);
        envelope.retarget(0.55, Duration::from_millis(300), start, false);
        assert_eq!(envelope.value(), 0.55, "retarget snaps instantly");
        assert!(
            envelope.settled(),
            "no animation ticking under reduced motion"
        );
        assert_eq!(
            envelope.advance(start + Duration::from_millis(1), false),
            0.55
        );

        envelope.retarget(1.0, Duration::from_millis(150), start, false);
        assert_eq!(envelope.value(), 1.0);
        assert!(envelope.settled());
    }
}
