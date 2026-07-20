//! Idle fade state for the top-strip islands (M3 Phase B).
//!
//! The strip dims to [`TOP_STRIP_DIM_LEVEL`] after [`TOP_STRIP_IDLE_DELAY`]
//! without drawing activity and restores when the pointer approaches the
//! toolbar (or any drawing happens). The policy lives here — renderer
//! neutral — so both frontends consume one `ToolbarSnapshot::top_fade`
//! value instead of computing their own fade state. Reduced motion snaps
//! between full and dimmed with no intermediate values (the underlying
//! [`Envelope`] handles that gate).

use std::time::{Duration, Instant};

use crate::ui::anim::Envelope;

/// Opacity of the dimmed top strip (1.0 = full).
pub const TOP_STRIP_DIM_LEVEL: f64 = 0.55;
/// Drawing-idle time before the strip starts dimming.
pub const TOP_STRIP_IDLE_DELAY: Duration = Duration::from_secs(4);
/// Duration of the dim (fade-out) transition.
pub const TOP_STRIP_FADE_OUT: Duration = Duration::from_millis(300);
/// Duration of the restore (fade-in) transition — snappier than the dim.
pub const TOP_STRIP_RESTORE: Duration = Duration::from_millis(150);
/// Wakeup cadence while a fade transition is in flight (~60fps).
pub const TOP_STRIP_FADE_TICK: Duration = Duration::from_millis(16);

/// Everything the fade policy looks at for one evaluation.
#[derive(Debug, Clone, Copy)]
pub struct TopStripFadeInputs {
    /// Time since the last stroke start/commit.
    pub idle_for: Duration,
    /// Pointer over/approaching the top toolbar (hover or focus).
    pub pointer_near: bool,
    /// Any top menu/popover open (shapes, overflow, Canvas/Session/Settings).
    pub menus_open: bool,
    /// Minimized restore tab, micro chip, or hidden strip: minimal chrome
    /// never fades.
    pub reduced_chrome: bool,
}

impl TopStripFadeInputs {
    fn dim_candidate(&self) -> bool {
        !self.pointer_near && !self.menus_open && !self.reduced_chrome
    }

    fn wants_dim(&self) -> bool {
        self.dim_candidate() && self.idle_for >= TOP_STRIP_IDLE_DELAY
    }
}

/// The top-strip fade engine: one owner (the backend) updates it once per
/// event-loop pass; the resulting value is published on the snapshot.
#[derive(Debug, Clone)]
pub struct TopStripFade {
    envelope: Envelope,
}

impl Default for TopStripFade {
    fn default() -> Self {
        Self::new()
    }
}

impl TopStripFade {
    pub fn new() -> Self {
        Self {
            envelope: Envelope::new(1.0),
        }
    }

    /// Re-evaluate the target from `inputs` and advance the transition to
    /// `now`. Returns the current fade value.
    pub fn update(&mut self, inputs: &TopStripFadeInputs, now: Instant) -> f64 {
        if inputs.wants_dim() {
            self.envelope
                .retarget(TOP_STRIP_DIM_LEVEL, TOP_STRIP_FADE_OUT, now);
        } else {
            self.envelope.retarget(1.0, TOP_STRIP_RESTORE, now);
        }
        self.envelope.advance(now)
    }

    /// Current fade value (1.0 = full, [`TOP_STRIP_DIM_LEVEL`] = dimmed).
    pub fn value(&self) -> f64 {
        self.envelope.value()
    }

    /// Whether a transition is still in flight.
    pub fn animating(&self) -> bool {
        !self.envelope.settled()
    }

    /// How long the owner may sleep before this fade needs attention again:
    /// the next animation tick while transitioning, the remaining idle time
    /// while a dim is pending, or `None` when fully settled with no pending
    /// trigger. The idle deadline applies under reduced motion too — the
    /// snap still has to happen at the 4s mark.
    pub fn wake_after(&self, inputs: &TopStripFadeInputs) -> Option<Duration> {
        if self.animating() {
            return Some(TOP_STRIP_FADE_TICK);
        }
        // The envelope only retargets inside `update`. If the inputs already
        // demand a transition the envelope has not been pointed at yet (the
        // idle deadline passed between the loop-bottom update and this
        // loop-top timeout computation, or a restore trigger appeared), ask
        // for an immediate wake — otherwise dispatch could block until an
        // unrelated event and the dim/restore would stall.
        if inputs.wants_dim() && self.value() > TOP_STRIP_DIM_LEVEL {
            return Some(Duration::ZERO);
        }
        if !inputs.wants_dim() && self.value() < 1.0 {
            return Some(Duration::ZERO);
        }
        if self.value() > TOP_STRIP_DIM_LEVEL && inputs.dim_candidate() && !inputs.wants_dim() {
            return Some(TOP_STRIP_IDLE_DELAY.saturating_sub(inputs.idle_for));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::anim::override_motion_for_test;

    fn inputs(idle_secs: u64) -> TopStripFadeInputs {
        TopStripFadeInputs {
            idle_for: Duration::from_secs(idle_secs),
            pointer_near: false,
            menus_open: false,
            reduced_chrome: false,
        }
    }

    #[test]
    fn strip_dims_after_the_idle_delay_and_restores_on_pointer_approach() {
        let _motion = override_motion_for_test(true);
        let mut fade = TopStripFade::new();
        let start = Instant::now();

        // Not yet idle long enough: full, with a wakeup at the 4s mark.
        assert_eq!(fade.update(&inputs(1), start), 1.0);
        assert!(!fade.animating());
        assert_eq!(
            fade.wake_after(&inputs(1)),
            Some(TOP_STRIP_IDLE_DELAY - Duration::from_secs(1))
        );

        // Past the idle delay: the fade-out toward 0.55 starts (still at
        // full for zero elapsed time) and requests animation ticks.
        let dim_start = start + Duration::from_millis(1);
        assert_eq!(fade.update(&inputs(5), dim_start), 1.0);
        assert!(fade.animating());
        assert_eq!(fade.wake_after(&inputs(5)), Some(TOP_STRIP_FADE_TICK));

        // Halfway through the 300ms fade-out: an intermediate value.
        let mid = fade.update(&inputs(5), dim_start + TOP_STRIP_FADE_OUT / 2);
        assert!(mid < 1.0 && mid > TOP_STRIP_DIM_LEVEL, "dimming: {mid}");

        // The transition settles exactly at the dim level and stops ticking.
        let settled = fade.update(&inputs(5), dim_start + TOP_STRIP_FADE_OUT);
        assert_eq!(settled, TOP_STRIP_DIM_LEVEL);
        assert!(!fade.animating());
        assert_eq!(fade.wake_after(&inputs(5)), None);

        // Pointer approach restores toward full (faster envelope).
        let mut near = inputs(9);
        near.pointer_near = true;
        let restore_start = dim_start + TOP_STRIP_FADE_OUT;
        assert_eq!(fade.update(&near, restore_start), TOP_STRIP_DIM_LEVEL);
        assert!(fade.animating(), "restore transition in flight");
        let rising = fade.update(&near, restore_start + TOP_STRIP_RESTORE / 2);
        assert!(
            rising > TOP_STRIP_DIM_LEVEL && rising < 1.0,
            "rising: {rising}"
        );
        assert_eq!(fade.update(&near, restore_start + TOP_STRIP_RESTORE), 1.0);
        assert!(!fade.animating());
        // Fully restored under the pointer: nothing to wake for.
        assert_eq!(fade.wake_after(&near), None);
    }

    #[test]
    fn draw_activity_restores_a_dimmed_strip() {
        let _motion = override_motion_for_test(true);
        let mut fade = TopStripFade::new();
        let start = Instant::now();
        fade.update(&inputs(10), start);
        fade.update(&inputs(10), start + TOP_STRIP_FADE_OUT);
        assert_eq!(fade.value(), TOP_STRIP_DIM_LEVEL);

        // A stroke resets the idle clock; the strip fades back to full.
        let after_stroke = inputs(0);
        fade.update(&after_stroke, start + TOP_STRIP_FADE_OUT);
        assert_eq!(
            fade.update(
                &after_stroke,
                start + TOP_STRIP_FADE_OUT + TOP_STRIP_RESTORE
            ),
            1.0
        );
    }

    #[test]
    fn menus_and_reduced_chrome_force_full_opacity() {
        let _motion = override_motion_for_test(true);
        let mut fade = TopStripFade::new();
        let start = Instant::now();

        let mut menu_open = inputs(30);
        menu_open.menus_open = true;
        assert_eq!(fade.update(&menu_open, start), 1.0);
        assert_eq!(fade.wake_after(&menu_open), None, "no dim pending");

        let mut minimal = inputs(30);
        minimal.reduced_chrome = true;
        assert_eq!(fade.update(&minimal, start), 1.0);
        assert_eq!(fade.wake_after(&minimal), None);
    }

    #[test]
    fn stalled_pending_transitions_request_an_immediate_wake() {
        let _motion = override_motion_for_test(true);
        let mut fade = TopStripFade::new();
        let start = Instant::now();

        // The last update ran before the idle deadline, so the envelope
        // still targets full and is settled...
        assert_eq!(fade.update(&inputs(3), start), 1.0);
        assert!(!fade.animating());
        // ...and by the time the loop computes its timeout the deadline has
        // passed. The stalled dim must request an immediate wake instead of
        // letting dispatch block until an arbitrary event.
        assert_eq!(fade.wake_after(&inputs(5)), Some(Duration::ZERO));

        // Mirror case: dimmed and settled, then a restore trigger appears
        // before any update has retargeted the envelope.
        let mut fade = TopStripFade::new();
        fade.update(&inputs(10), start);
        fade.update(&inputs(10), start + TOP_STRIP_FADE_OUT);
        assert_eq!(fade.value(), TOP_STRIP_DIM_LEVEL);
        assert!(!fade.animating());
        let mut near = inputs(10);
        near.pointer_near = true;
        assert_eq!(fade.wake_after(&near), Some(Duration::ZERO));
    }

    #[test]
    fn reduced_motion_snaps_between_full_and_dim_without_ticking() {
        let _motion = override_motion_for_test(false);
        let mut fade = TopStripFade::new();
        let start = Instant::now();

        assert_eq!(fade.update(&inputs(1), start), 1.0);
        // The 4s idle trigger still needs its wakeup under reduced motion.
        assert_eq!(
            fade.wake_after(&inputs(1)),
            Some(TOP_STRIP_IDLE_DELAY - Duration::from_secs(1))
        );

        // Hard snap: no intermediate values, no animation ticking.
        assert_eq!(fade.update(&inputs(5), start), TOP_STRIP_DIM_LEVEL);
        assert!(!fade.animating());
        assert_eq!(fade.wake_after(&inputs(5)), None);

        let mut near = inputs(9);
        near.pointer_near = true;
        assert_eq!(fade.update(&near, start), 1.0);
        assert!(!fade.animating());
    }
}
