//! Fading laser-pointer ink.
//!
//! Finished laser strokes live here and nowhere else. They never enter a
//! frame, so undo, session snapshots, selection, hit-testing, exports, and
//! captures cannot see them by construction.
//!
//! The strokes fade as one group. Each release restarts the group's hold, and
//! drawing another stroke keeps the group fully visible, so a gesture made of
//! several strokes stays whole and disappears together.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::config::LaserConfig;
use crate::draw::{Color, DirtyTracker, LaserStyle};
use crate::util::Rect;

/// Finished strokes kept at once. A presenter who never pauses long enough
/// for the ink to fade drops the oldest stroke instead of growing the group
/// without bound.
const MAX_LASER_STROKES: usize = 64;

/// Appearance and timing of laser ink.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LaserSettings {
    pub(crate) style: LaserStyle,
    /// How long finished ink stays fully visible after the latest release.
    pub(crate) hold: Duration,
    /// How long it then takes to fade out.
    pub(crate) fade: Duration,
}

impl From<&LaserConfig> for LaserSettings {
    fn from(config: &LaserConfig) -> Self {
        let [r, g, b, a] = config.color;
        Self {
            style: LaserStyle {
                color: Color { r, g, b, a },
                width: config.width,
            },
            hold: Duration::from_millis(config.hold_ms),
            fade: Duration::from_millis(config.fade_ms),
        }
    }
}

impl Default for LaserSettings {
    fn default() -> Self {
        Self::from(&LaserConfig::default())
    }
}

struct LaserStroke {
    points: Vec<(i32, i32)>,
    damage: Vec<Rect>,
}

/// Where the group is in its visible lifetime at one instant.
#[derive(Debug, Clone, Copy, PartialEq)]
enum LaserPhase {
    Holding,
    Fading(f64),
    Expired,
}

/// Finished laser strokes and the clock they fade on.
pub(crate) struct LaserInk {
    settings: LaserSettings,
    strokes: VecDeque<LaserStroke>,
    /// When the hold last restarted: the latest release, or the latest frame
    /// in which another laser stroke was being drawn.
    held_since: Option<Instant>,
    /// Opacity the ink was last painted with. Rendering reads this instead of
    /// the clock, so a frame paints exactly what its damage accounted for.
    painted_opacity: f64,
    /// Whether the last advance asked for continuous frames, so the loop is
    /// already ticking for the fade.
    animating: bool,
}

impl LaserInk {
    pub(crate) fn new(settings: LaserSettings) -> Self {
        Self {
            settings,
            strokes: VecDeque::new(),
            held_since: None,
            painted_opacity: 1.0,
            animating: false,
        }
    }

    pub(crate) fn style(&self) -> LaserStyle {
        self.settings.style
    }

    /// Replaces the appearance and timing. Ink already on screen is
    /// repainted in the new style.
    pub(crate) fn set_settings(&mut self, settings: LaserSettings, tracker: &mut DirtyTracker) {
        if self.settings == settings {
            return;
        }
        self.mark_all(tracker);

        self.settings = settings;
        for stroke in &mut self.strokes {
            stroke.damage = settings.style.damage_regions(&stroke.points);
        }
        self.mark_all(tracker);
    }

    pub(crate) fn has_ink(&self) -> bool {
        !self.strokes.is_empty()
    }

    /// Adds a released stroke and restarts the whole group's hold at `now`.
    pub(crate) fn commit(
        &mut self,
        points: Vec<(i32, i32)>,
        now: Instant,
        tracker: &mut DirtyTracker,
    ) {
        if points.is_empty() {
            return;
        }
        if self.strokes.len() >= MAX_LASER_STROKES
            && let Some(oldest) = self.strokes.pop_front()
        {
            mark_regions(tracker, &oldest.damage);
        }

        let damage = self.settings.style.damage_regions(&points);
        mark_regions(tracker, &damage);
        self.strokes.push_back(LaserStroke { points, damage });
        self.held_since = Some(now);

        if self.painted_opacity < 1.0 {
            self.painted_opacity = 1.0;
            self.mark_all(tracker);
        }
        self.animating = false;
    }

    /// Moves the group along its lifetime, marking damage for any change.
    ///
    /// `drawing` is whether another laser stroke is under the pointer, which
    /// keeps the group fully visible. Returns whether the ink needs
    /// continuous frames: only a fade in motion does. Under reduced motion
    /// the ink is still until it is removed, and [`Self::wake_after`]
    /// supplies that one deadline.
    pub(crate) fn advance(
        &mut self,
        now: Instant,
        drawing: bool,
        motion: bool,
        tracker: &mut DirtyTracker,
    ) -> bool {
        if self.strokes.is_empty() {
            return false;
        }
        if drawing {
            self.held_since = Some(now);
        }

        let (opacity, animating) = match self.phase_at(now) {
            LaserPhase::Expired => {
                self.clear(tracker);
                return false;
            }
            LaserPhase::Holding => (1.0, false),
            LaserPhase::Fading(_) if !motion => (1.0, false),
            LaserPhase::Fading(opacity) => (opacity, true),
        };

        // A fading frame always damages the ink, even at the instant the
        // fade begins, so the frame it asked for is never empty.
        if animating || opacity != self.painted_opacity {
            self.mark_all(tracker);
        }
        self.painted_opacity = opacity;
        self.animating = animating;
        animating
    }

    /// How long until the ink changes on its own, for a loop that is not
    /// already ticking for animation.
    ///
    /// `None` without ink, while a stroke is being drawn (pointer motion
    /// drives those frames), and during a fade the animation tick already
    /// covers. Otherwise the time until the hold ends, or under reduced motion
    /// until the ink is removed; zero once that moment has passed unpainted.
    pub(crate) fn wake_after(&self, now: Instant, drawing: bool, motion: bool) -> Option<Duration> {
        if self.strokes.is_empty() || drawing {
            return None;
        }
        let elapsed = now.saturating_duration_since(self.held_since?);
        let hold = self.settings.hold;
        let lifetime = hold.saturating_add(self.settings.fade);

        if elapsed >= lifetime {
            return Some(Duration::ZERO);
        }
        if !motion {
            return Some(lifetime - elapsed);
        }
        if elapsed < hold {
            return Some(hold - elapsed);
        }
        (!self.animating).then_some(Duration::ZERO)
    }

    /// Paints every finished stroke at the opacity the last advance chose.
    pub(crate) fn render(&self, ctx: &cairo::Context) {
        for stroke in &self.strokes {
            crate::draw::render_laser_stroke(
                ctx,
                &stroke.points,
                self.settings.style,
                self.painted_opacity,
            );
        }
    }

    /// Removes all ink at once.
    pub(crate) fn clear(&mut self, tracker: &mut DirtyTracker) {
        self.mark_all(tracker);
        self.strokes.clear();
        self.held_since = None;
        self.painted_opacity = 1.0;
        self.animating = false;
    }

    fn phase_at(&self, now: Instant) -> LaserPhase {
        let Some(held_since) = self.held_since else {
            return LaserPhase::Holding;
        };
        let elapsed = now.saturating_duration_since(held_since);
        let Some(into_fade) = elapsed.checked_sub(self.settings.hold) else {
            return LaserPhase::Holding;
        };
        if into_fade >= self.settings.fade {
            return LaserPhase::Expired;
        }
        let remaining = 1.0 - into_fade.as_secs_f64() / self.settings.fade.as_secs_f64();
        LaserPhase::Fading(remaining.clamp(0.0, 1.0))
    }

    fn mark_all(&self, tracker: &mut DirtyTracker) {
        for stroke in &self.strokes {
            mark_regions(tracker, &stroke.damage);
        }
    }

    #[cfg(test)]
    pub(crate) fn stroke_count(&self) -> usize {
        self.strokes.len()
    }

    #[cfg(test)]
    pub(crate) fn painted_opacity(&self) -> f64 {
        self.painted_opacity
    }
}

fn mark_regions(tracker: &mut DirtyTracker, regions: &[Rect]) {
    for region in regions {
        tracker.mark_rect(*region);
    }
}

#[cfg(test)]
mod tests;
