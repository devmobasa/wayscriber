//! Shared animation envelopes and the reduced-motion gate (M1 foundation).
//!
//! Chrome fades route through these helpers instead of hand-rolled piecewise
//! math so timing stays consistent and the `[ui] reduced_motion` setting can
//! disable non-essential animation in one place (WCAG 2.3.3). When motion is
//! disabled, envelopes snap to their resting value: transient chrome shows at
//! full opacity for its lifetime and disappears instantly.

use std::sync::atomic::{AtomicBool, Ordering};

static MOTION_ENABLED: AtomicBool = AtomicBool::new(true);

/// Enable/disable non-essential chrome animation process-wide. Wired from the
/// `[ui] reduced_motion` config at startup.
pub fn set_motion_enabled(enabled: bool) {
    MOTION_ENABLED.store(enabled, Ordering::Relaxed);
}

/// Whether non-essential chrome animation is enabled.
pub fn motion_enabled() -> bool {
    MOTION_ENABLED.load(Ordering::Relaxed)
}

/// Cubic ease-out easing curve (pure math; not gated).
#[allow(dead_code)] // consumed by island fades in M2+
pub fn ease_out_cubic(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Fully opaque for `hold_ratio` of the lifetime, then a linear fade to zero
/// at `progress == 1.0`. Used by preset toasts.
pub fn hold_then_fade_out(progress: f64, hold_ratio: f64) -> f64 {
    if !motion_enabled() {
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
pub fn end_fade(elapsed_secs: f64, duration_secs: f64, fade_secs: f64) -> f64 {
    let fade_duration = duration_secs.min(fade_secs);
    if fade_duration <= 0.0 {
        return 0.0;
    }
    if !motion_enabled() {
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
pub fn flash(progress: f64, attack_end: f64, hold_end: f64, peak: f64) -> f64 {
    if !motion_enabled() {
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
        assert!((flash(0.075, 0.15, 0.4, 0.22) - 0.11).abs() < 1e-9);
        assert!((flash(0.2, 0.15, 0.4, 0.22) - 0.22).abs() < 1e-9);
        assert!((flash(0.7, 0.15, 0.4, 0.22) - 0.11).abs() < 1e-9);
        assert!(flash(1.0, 0.15, 0.4, 0.22).abs() < 1e-9);
    }
}
