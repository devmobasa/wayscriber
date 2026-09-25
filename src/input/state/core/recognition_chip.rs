//! The brief chip that names what Shape Pen recognized.
//!
//! A recognized stroke commits as a shape, and one undo gives its ink back.
//! That contract is invisible unless something says so at the moment it
//! matters, so the chip names the shape beside it together with the undo
//! shortcut, then fades. It is transient chrome: it lives here rather than in
//! any frame, so exports, captures, and sessions never see it.

use std::time::{Duration, Instant};

use super::base::InputState;
use crate::domain::Action;
use crate::draw::Shape;
use crate::util::Rect;

/// How long the chip stays up, including its fade.
const RECOGNITION_CHIP_LIFETIME: Duration = Duration::from_millis(1500);
/// The chip fades over the last part of its lifetime.
const RECOGNITION_CHIP_FADE: Duration = Duration::from_millis(400);

/// Radii within this fraction of each other read as a circle.
const CIRCLE_RADIUS_TOLERANCE: f64 = 0.1;

/// One shown chip: its text, where the shape is, and when it appeared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecognitionChip {
    label: String,
    /// Canvas bounds of the recognized shape; the chip sits beside them.
    anchor: Rect,
    started: Instant,
}

impl RecognitionChip {
    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    pub(crate) const fn anchor(&self) -> Rect {
        self.anchor
    }

    /// How long the chip has been up.
    fn age(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.started)
    }

    /// Fully opaque, then fading out over the end of its lifetime. Under
    /// `[ui] reduced_motion` it stays opaque and then disappears.
    pub(crate) fn opacity(&self, now: Instant) -> f64 {
        crate::ui::anim::end_fade(
            self.age(now).as_secs_f64(),
            RECOGNITION_CHIP_LIFETIME.as_secs_f64(),
            RECOGNITION_CHIP_FADE.as_secs_f64(),
        )
    }

    fn expired(&self, now: Instant) -> bool {
        self.age(now) >= RECOGNITION_CHIP_LIFETIME
    }
}

/// Whether recognition feedback is enabled, and the chip currently shown.
#[derive(Debug, Clone)]
pub(in crate::input::state) struct RecognitionFeedback {
    enabled: bool,
    chip: Option<RecognitionChip>,
}

impl Default for RecognitionFeedback {
    fn default() -> Self {
        Self {
            enabled: true,
            chip: None,
        }
    }
}

impl InputState {
    /// Applies `[drawing] shape_recognition_feedback`.
    pub(crate) fn set_shape_recognition_feedback(&mut self, enabled: bool) {
        self.recognition_feedback.enabled = enabled;
        if !enabled {
            self.clear_recognition_chip();
        }
    }

    /// Shows the chip for a stroke Shape Pen just turned into `shape`, whose
    /// canvas bounds are `bounds`. It replaces any chip still showing.
    pub(in crate::input::state) fn show_recognition_chip(
        &mut self,
        shape: &Shape,
        bounds: Option<Rect>,
        now: Instant,
    ) {
        let Some(anchor) = bounds.filter(|_| self.recognition_feedback.enabled) else {
            return;
        };

        let label = format!(
            "{} · {}",
            recognized_shape_label(shape),
            self.recognition_undo_hint()
        );
        self.recognition_feedback.chip = Some(RecognitionChip {
            label,
            anchor,
            started: now,
        });
        self.needs_redraw = true;
    }

    /// Takes the chip away early, for example once the undo it advertises ran.
    pub(in crate::input::state) fn clear_recognition_chip(&mut self) {
        if self.recognition_feedback.chip.take().is_some() {
            self.needs_redraw = true;
        }
    }

    /// The chip currently shown, if any.
    pub(crate) fn recognition_chip(&self) -> Option<&RecognitionChip> {
        self.recognition_feedback.chip.as_ref()
    }

    /// Expires the chip once its lifetime is over. Returns whether it is still
    /// up and so needs frames for its fade.
    pub fn advance_recognition_chip(&mut self, now: Instant) -> bool {
        let Some(chip) = &self.recognition_feedback.chip else {
            return false;
        };
        if chip.expired(now) {
            self.recognition_feedback.chip = None;
            return false;
        }
        true
    }

    /// "Ctrl+Z keeps ink" with the configured undo shortcut, or a plain
    /// wording when undo has no binding.
    fn recognition_undo_hint(&self) -> String {
        match self.shortcut_for_action(Action::Undo) {
            Some(shortcut) => format!("{shortcut} keeps ink"),
            None => "Undo keeps ink".to_string(),
        }
    }
}

/// The name the chip gives a recognized shape. Shape Pen fits circles as
/// ellipses, so near-equal radii are called a circle.
fn recognized_shape_label(shape: &Shape) -> &'static str {
    match shape {
        Shape::Ellipse { rx, ry, .. } => {
            let (rx, ry) = (f64::from(rx.unsigned_abs()), f64::from(ry.unsigned_abs()));
            if (rx - ry).abs() <= rx.max(ry) * CIRCLE_RADIUS_TOLERANCE {
                "Circle"
            } else {
                "Ellipse"
            }
        }
        other => other.kind_name(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::Color;

    fn ellipse(rx: i32, ry: i32) -> Shape {
        Shape::Ellipse {
            cx: 100,
            cy: 100,
            rx,
            ry,
            fill: false,
            color: Color::new(1.0, 0.0, 0.0, 1.0),
            thick: 3.0,
        }
    }

    #[test]
    fn near_equal_radii_are_a_circle_and_others_an_ellipse() {
        assert_eq!(recognized_shape_label(&ellipse(60, 57)), "Circle");
        assert_eq!(recognized_shape_label(&ellipse(80, 40)), "Ellipse");
    }

    #[test]
    fn the_drawing_config_switch_reaches_input_state() {
        let mut config = crate::config::Config::default();
        assert!(
            InputState::from_config(&config)
                .recognition_feedback
                .enabled
        );

        config.drawing.shape_recognition_feedback = false;

        assert!(
            !InputState::from_config(&config)
                .recognition_feedback
                .enabled
        );
    }

    #[test]
    fn other_shapes_use_their_kind_name() {
        let line = Shape::Line {
            x1: 0,
            y1: 0,
            x2: 40,
            y2: 0,
            color: Color::new(1.0, 0.0, 0.0, 1.0),
            thick: 3.0,
        };

        assert_eq!(recognized_shape_label(&line), "Line");
    }
}
