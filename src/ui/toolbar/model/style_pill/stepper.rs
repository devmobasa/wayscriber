//! The −/value/+ steppers of the style pill: their halves and their captions.

use super::*;

impl StylePillControl {
    /// The −/+ halves of a stepper, in reading order.
    pub(crate) fn steps(self, snapshot: &ToolbarSnapshot) -> Option<[StylePillStep; 2]> {
        if self == Self::PenSmoothingStepper {
            return Some(pen_smoothing_steps(snapshot));
        }
        if self == Self::ShapeSensitivityStepper {
            return Some(shape_sensitivity_steps(snapshot));
        }
        let Self::SelectionStepper(kind) = self else {
            return None;
        };
        let entry = selection_entry(snapshot, kind)?;
        let (minus_id, plus_id) = match kind {
            SelectionPropertyKind::Thickness => (
                "top.style.sel.thickness.minus",
                "top.style.sel.thickness.plus",
            ),
            SelectionPropertyKind::FontSize => (
                "top.style.sel.font-size.minus",
                "top.style.sel.font-size.plus",
            ),
            SelectionPropertyKind::ArrowLength => (
                "top.style.sel.arrow-length.minus",
                "top.style.sel.arrow-length.plus",
            ),
            SelectionPropertyKind::ArrowAngle => (
                "top.style.sel.arrow-angle.minus",
                "top.style.sel.arrow-angle.plus",
            ),
            SelectionPropertyKind::SpotlightMagnification => (
                "top.style.sel.spotlight-magnification.minus",
                "top.style.sel.spotlight-magnification.plus",
            ),
            _ => return None,
        };
        Some([
            StylePillStep {
                id: minus_id,
                label: "\u{2212}",
                event: ToolbarEvent::AdjustSelectionProperty {
                    kind,
                    direction: -1,
                },
                tooltip: format!("Decrease {}", entry.label.to_lowercase()),
            },
            StylePillStep {
                id: plus_id,
                label: "+",
                event: ToolbarEvent::AdjustSelectionProperty { kind, direction: 1 },
                tooltip: format!("Increase {}", entry.label.to_lowercase()),
            },
        ])
    }

    /// −/+ halves for a stepper already present in the spec.
    pub(crate) fn required_steps(self, snapshot: &ToolbarSnapshot) -> [StylePillStep; 2] {
        self.steps(snapshot)
            .expect("this style-pill stepper has minus/plus halves")
    }

    /// Short visible caption drawn before a stepper whose readout alone does
    /// not say what it steps.
    ///
    /// A bare "− 3 +" names nothing, and Shape Pen shows two of them side by
    /// side. The docked selection steppers need none: their readouts carry a
    /// unit ("3px", "24pt"), and the properties popup names them in full.
    /// Kept to one short word because both frontends budget a fixed caption
    /// slot; the full name stays the accessible label.
    pub(crate) fn caption(self) -> Option<&'static str> {
        match self {
            Self::PenSmoothingStepper => Some("Smooth"),
            Self::ShapeSensitivityStepper => Some("Detect"),
            _ => None,
        }
    }
}

/// The smoothing stepper's halves, clamped to the range the setting accepts.
///
/// Each half carries the level it would land on rather than a direction: the
/// pill's events are absolute, and computing the target here keeps the clamp in
/// one place instead of in both frontends.
fn pen_smoothing_steps(snapshot: &ToolbarSnapshot) -> [StylePillStep; 2] {
    let level = snapshot.pen_smoothing;
    [
        StylePillStep {
            id: "top.style.pen-smoothing.minus",
            label: "\u{2212}",
            event: ToolbarEvent::SetPenSmoothing(level.saturating_sub(1)),
            tooltip: "Less smoothing".to_string(),
        },
        StylePillStep {
            id: "top.style.pen-smoothing.plus",
            label: "+",
            event: ToolbarEvent::SetPenSmoothing(
                level.saturating_add(1).min(crate::draw::MAX_PEN_SMOOTHING),
            ),
            tooltip: "More smoothing".to_string(),
        },
    ]
}

/// The Shape Pen sensitivity stepper's halves, clamped like smoothing's.
fn shape_sensitivity_steps(snapshot: &ToolbarSnapshot) -> [StylePillStep; 2] {
    let level = snapshot.shape_recognition_sensitivity;
    [
        StylePillStep {
            id: "top.style.shape-sensitivity.minus",
            label: "\u{2212}",
            event: ToolbarEvent::SetShapeRecognitionSensitivity(level.saturating_sub(1)),
            tooltip: "Keep more strokes as ink".to_string(),
        },
        StylePillStep {
            id: "top.style.shape-sensitivity.plus",
            label: "+",
            event: ToolbarEvent::SetShapeRecognitionSensitivity(
                level
                    .saturating_add(1)
                    .min(crate::config::MAX_SHAPE_RECOGNITION_SENSITIVITY),
            ),
            tooltip: "Recognize rougher strokes".to_string(),
        },
    ]
}
