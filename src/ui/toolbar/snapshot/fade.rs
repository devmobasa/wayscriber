//! Idle hide/reveal state for the top-strip islands.
//!
//! With `ui.toolbar.idle_fade` on, the strip fades out until it is fully
//! transparent once it has sat unused for [`TOP_STRIP_IDLE_DELAY`]: no stroke
//! started or committed, and nothing holding it (pointer on or near it, an
//! open menu). It fades back in when the pointer enters the reveal zone the
//! backend measures around it, hovers it, or a menu opens, and
//! [`TopStripFade::reveal_briefly`] flashes it after a keyboard tool or color
//! change. Drawing keeps a visible strip up but never reveals a hidden one,
//! so annotating after a pause does not pop the bar back in.
//!
//! A strip at [`TOP_STRIP_HIDDEN_LEVEL`] ([`top_strip_hidden`]) passes pointer
//! input through to the canvas in every frontend. The policy lives here,
//! renderer neutral, so both frontends consume one `ToolbarSnapshot::top_fade`
//! value instead of computing their own state. Reduced motion snaps between
//! shown and hidden with no intermediate values (the underlying [`Envelope`]
//! handles that gate).

use std::time::{Duration, Instant};

use crate::ui::anim::Envelope;

/// Opacity of the fully shown top strip.
pub const TOP_STRIP_SHOWN_LEVEL: f64 = 1.0;
/// Opacity of the idle-hidden top strip.
pub const TOP_STRIP_HIDDEN_LEVEL: f64 = 0.0;
/// Unused time before the strip starts hiding.
pub const TOP_STRIP_IDLE_DELAY: Duration = Duration::from_secs(4);
/// Duration of the hide (fade-out) transition.
pub const TOP_STRIP_FADE_OUT: Duration = Duration::from_millis(300);
/// Duration of the reveal (fade-in) transition, snappier than the hide.
pub const TOP_STRIP_RESTORE: Duration = Duration::from_millis(150);
/// How long a keyboard tool/color change keeps an otherwise idle strip up.
pub const TOP_STRIP_REVEAL_PULSE: Duration = Duration::from_millis(1500);
/// Wakeup cadence while a fade transition is in flight (~60fps).
pub const TOP_STRIP_FADE_TICK: Duration = Duration::from_millis(16);

/// Whether a published `top_fade` value is the fully hidden strip, which
/// must not intercept pointer input.
pub fn top_strip_hidden(fade: f64) -> bool {
    fade <= TOP_STRIP_HIDDEN_LEVEL
}

/// Everything the fade policy looks at for one evaluation.
#[derive(Debug, Clone, Copy)]
pub struct TopStripFadeInputs {
    /// Time since the last stroke start/commit.
    pub idle_for: Duration,
    /// Pointer (or keyboard focus) on the strip, or inside the reveal zone
    /// around it.
    pub pointer_near: bool,
    /// Any top menu/popover open (shapes, overflow, Canvas/Session/Settings).
    pub menus_open: bool,
    /// Minimized restore tab, micro chip, or hidden strip: minimal chrome
    /// never fades.
    pub reduced_chrome: bool,
    /// Authored/runtime preference: when false the strip stays fully
    /// visible and no idle deadline is scheduled.
    pub idle_fade_enabled: bool,
}

impl TopStripFadeInputs {
    /// Anything that pins the strip visible at this moment.
    fn holds_strip(&self) -> bool {
        !self.idle_fade_enabled || self.pointer_near || self.menus_open || self.reduced_chrome
    }
}

/// The top-strip fade engine: one owner (the backend) updates it once per
/// event-loop pass; the resulting value is published on the snapshot.
#[derive(Debug, Clone)]
pub struct TopStripFade {
    envelope: Envelope,
    /// Latched hide decision. Only a hold or a reveal pulse clears it, so a
    /// stroke does not bring a hidden strip back.
    hidden: bool,
    /// Last update at which something held the strip. The idle delay also
    /// counts from here, so the strip lingers after the pointer leaves.
    last_held: Option<Instant>,
    /// End of the brief reveal that a keyboard tool/color change requested.
    reveal_until: Option<Instant>,
}

impl Default for TopStripFade {
    fn default() -> Self {
        Self::new()
    }
}

impl TopStripFade {
    pub fn new() -> Self {
        Self {
            envelope: Envelope::new(TOP_STRIP_SHOWN_LEVEL),
            hidden: false,
            last_held: None,
            reveal_until: None,
        }
    }

    /// Re-evaluate the target from `inputs` and advance the transition to
    /// `now`. Returns the current fade value.
    pub fn update(&mut self, inputs: &TopStripFadeInputs, now: Instant) -> f64 {
        if inputs.holds_strip() {
            self.last_held = Some(now);
        }
        self.hidden = self.next_hidden(inputs, now);
        if self.reveal_until.is_some_and(|until| now >= until) {
            self.reveal_until = None;
        }

        if self.hidden {
            self.envelope
                .retarget(TOP_STRIP_HIDDEN_LEVEL, TOP_STRIP_FADE_OUT, now);
        } else {
            self.envelope
                .retarget(TOP_STRIP_SHOWN_LEVEL, TOP_STRIP_RESTORE, now);
        }
        self.envelope.advance(now)
    }

    /// Show the strip for [`TOP_STRIP_REVEAL_PULSE`] so a keyboard tool or
    /// color change is visible, then let the idle rules take over again.
    /// Applied by the next [`Self::update`].
    pub fn reveal_briefly(&mut self, now: Instant) {
        self.reveal_until = Some(now.checked_add(TOP_STRIP_REVEAL_PULSE).unwrap_or(now));
    }

    /// Current fade value ([`TOP_STRIP_SHOWN_LEVEL`] = shown,
    /// [`TOP_STRIP_HIDDEN_LEVEL`] = hidden).
    pub fn value(&self) -> f64 {
        self.envelope.value()
    }

    /// Whether a transition is still in flight.
    pub fn animating(&self) -> bool {
        !self.envelope.settled()
    }

    /// How long the owner may sleep before this fade needs attention again:
    /// the next animation tick while transitioning, the moment both the idle
    /// delay and any reveal pulse run out while shown, or `None` when
    /// settled with nothing pending. A hidden strip waits for input (pointer
    /// motion, a key, a menu), which wakes the loop on its own. The idle
    /// deadline applies under reduced motion too: the snap still has to
    /// happen on time.
    pub fn wake_after(&self, inputs: &TopStripFadeInputs, now: Instant) -> Option<Duration> {
        if self.animating() {
            return Some(TOP_STRIP_FADE_TICK);
        }

        // The latch only moves inside `update`. If the inputs already demand
        // a transition (the idle deadline passed between the loop-bottom
        // update and this loop-top timeout computation, or a hold or reveal
        // appeared), ask for an immediate wake, or dispatch could block until
        // an unrelated event and the hide/reveal would stall.
        let hidden = self.next_hidden(inputs, now);
        if hidden != self.hidden || self.value() != level(hidden) {
            return Some(Duration::ZERO);
        }
        if hidden || inputs.holds_strip() {
            return None;
        }

        let idle_left = TOP_STRIP_IDLE_DELAY.saturating_sub(self.idle_elapsed(inputs, now));
        let reveal_left = self
            .reveal_until
            .map_or(Duration::ZERO, |until| until.saturating_duration_since(now));
        Some(idle_left.max(reveal_left))
    }

    fn next_hidden(&self, inputs: &TopStripFadeInputs, now: Instant) -> bool {
        if inputs.holds_strip() || self.revealing(now) {
            return false;
        }
        self.hidden || self.idle_elapsed(inputs, now) >= TOP_STRIP_IDLE_DELAY
    }

    fn revealing(&self, now: Instant) -> bool {
        self.reveal_until.is_some_and(|until| now < until)
    }

    /// Unused time: since the last stroke or the last hold, whichever is
    /// more recent.
    fn idle_elapsed(&self, inputs: &TopStripFadeInputs, now: Instant) -> Duration {
        let since_held = self
            .last_held
            .map_or(Duration::MAX, |held| now.saturating_duration_since(held));
        inputs.idle_for.min(since_held)
    }
}

fn level(hidden: bool) -> f64 {
    if hidden {
        TOP_STRIP_HIDDEN_LEVEL
    } else {
        TOP_STRIP_SHOWN_LEVEL
    }
}

#[cfg(test)]
mod tests;
