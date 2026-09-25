use std::time::{Duration, Instant};

use super::base::{DrawingState, InputState};
use crate::config::LaserConfig;
use crate::draw::LaserStyle;
use crate::input::state::laser::LaserSettings;
use crate::input::tool::Tool;

impl InputState {
    /// Applies `[laser]` appearance and timing.
    pub(crate) fn init_laser_from_config(&mut self, config: &LaserConfig) {
        self.laser
            .set_settings(LaserSettings::from(config), &mut self.dirty_tracker);
    }

    /// Color and width laser ink is drawn with, live and finished alike.
    pub(crate) fn laser_style(&self) -> LaserStyle {
        self.laser.style()
    }

    /// Width the status bar reports for `tool`: the laser draws with its own
    /// `[laser]` width, not the pen thickness the other tools share.
    pub(crate) fn status_size_for_tool(&self, tool: Tool) -> f64 {
        if tool == Tool::Laser {
            self.laser_style().width
        } else {
            self.thickness_for_tool(tool)
        }
    }

    /// Color the status bar shows for `tool`, following the laser's own color.
    pub(crate) fn status_color_for_tool(&self, tool: Tool) -> crate::draw::Color {
        if tool == Tool::Laser {
            self.laser_style().color
        } else {
            self.color_for_tool(tool)
        }
    }

    /// Hands a released laser stroke to the fading ink. It never reaches the
    /// frame, history, or the session.
    pub(crate) fn commit_laser_stroke(&mut self, points: Vec<(i32, i32)>) {
        self.commit_laser_stroke_at(points, Instant::now());
    }

    pub(crate) fn commit_laser_stroke_at(&mut self, points: Vec<(i32, i32)>, now: Instant) {
        // The finished ink paints exactly what the preview did, so only the
        // stroke's own path is repainted, not the preview's whole box.
        let _ = self.take_provisional_dirty_bounds();
        self.laser.commit(points, now, &mut self.dirty_tracker);
        self.needs_redraw = true;
    }

    /// Advances the laser fade; returns whether it needs continuous frames.
    pub fn advance_laser_ink(&mut self, now: Instant) -> bool {
        self.advance_laser_ink_for(now, crate::ui::anim::motion_enabled())
    }

    /// Split from the accessor so both motion settings can be exercised
    /// without writing the process-wide flag every parallel test shares.
    pub(crate) fn advance_laser_ink_for(&mut self, now: Instant, motion: bool) -> bool {
        let drawing = self.laser_stroke_in_progress();
        self.laser
            .advance(now, drawing, motion, &mut self.dirty_tracker)
    }

    /// When the laser ink next changes on its own, for a loop that is not
    /// already ticking for animation.
    pub(crate) fn laser_ink_wake_after(&self, now: Instant) -> Option<Duration> {
        self.laser_ink_wake_after_for(now, crate::ui::anim::motion_enabled())
    }

    pub(crate) fn laser_ink_wake_after_for(&self, now: Instant, motion: bool) -> Option<Duration> {
        self.laser
            .wake_after(now, self.laser_stroke_in_progress(), motion)
    }

    /// Whether the laser ink has reached a change that needs a frame.
    pub(crate) fn laser_ink_due(&self, now: Instant) -> bool {
        self.laser_ink_wake_after(now) == Some(Duration::ZERO)
    }

    /// Paints finished laser ink at its current fade.
    pub(crate) fn render_laser_ink(&self, ctx: &cairo::Context) {
        self.laser.render(ctx);
    }

    /// Removes all laser ink at once.
    pub(crate) fn clear_laser_ink(&mut self) {
        if self.laser.has_ink() {
            self.laser.clear(&mut self.dirty_tracker);
            self.needs_redraw = true;
        }
    }

    fn laser_stroke_in_progress(&self) -> bool {
        matches!(
            self.state,
            DrawingState::Drawing {
                tool: Tool::Laser,
                ..
            }
        )
    }
}
