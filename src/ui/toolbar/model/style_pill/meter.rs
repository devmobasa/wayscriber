//! Level meters of the style pill: pen smoothing and Shape Pen detection.
//!
//! A bare "− 3 +" said neither how high 3 is nor how far it goes, and the two
//! settings run over different ranges. A meter shows both at a glance: one bar
//! per level, filled up to the current one. Clicking a bar sets that level;
//! clicking the highest filled bar steps one below it, so the lowest level (Off
//! for smoothing, Precise for detection) stays one click away. The wheel steps
//! one level either way, and each level has a name for tooltips and assistive
//! tech.
//!
//! The same meters appear inline in the pill (`stroke_controls = "meter"`) and
//! inside the Pen feel panel (the default); [`StrokeSetting`] is the one place
//! both read their range, level names, and events from.

use super::*;

/// Names of pen smoothing levels 0..=6, weakest first.
const SMOOTHING_LEVEL_NAMES: [&str; 7] = [
    "Off", "Minimal", "Light", "Medium", "Strong", "Stronger", "Maximum",
];

/// Names of Shape Pen detection levels 0..=4, strictest first.
const DETECTION_LEVEL_NAMES: [&str; 5] = [
    "Precise",
    "Careful",
    "Balanced",
    "Forgiving",
    "Very forgiving",
];

/// One bar of a level meter.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StylePillMeterSegment {
    /// Stable widget id, `<meter id>.level-<n>`.
    pub(crate) id: String,
    /// Whether the bar sits at or below the current level.
    pub(crate) filled: bool,
    /// The level a click on this bar applies.
    pub(crate) event: ToolbarEvent,
    pub(crate) tooltip: String,
}

/// A level meter: the current level out of `max`, one bar per level above 0.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StylePillMeter {
    pub(crate) level: u8,
    pub(crate) max: u8,
    pub(crate) segments: Vec<StylePillMeterSegment>,
}

/// A stroke-feel setting the style pill adjusts.
///
/// The inline meters and the Pen feel panel both take their range, level
/// names, and events from here, so the two presentations cannot disagree
/// about what a level is called or what clicking it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum StrokeSetting {
    /// Pen and marker smoothing (`[drawing] pen_smoothing`).
    Smoothing,
    /// Shape Pen recognition sensitivity
    /// (`[drawing] shape_recognition_sensitivity`).
    ShapeDetection,
}

impl StrokeSetting {
    /// Setting name used in tooltips and panel sections ("Smoothing").
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Smoothing => "Smoothing",
            Self::ShapeDetection => "Shape detection",
        }
    }

    const fn level_names(self) -> &'static [&'static str] {
        match self {
            Self::Smoothing => &SMOOTHING_LEVEL_NAMES,
            Self::ShapeDetection => &DETECTION_LEVEL_NAMES,
        }
    }

    /// Highest level; the lowest is always 0.
    pub(crate) const fn max(self) -> u8 {
        (self.level_names().len() - 1) as u8
    }

    /// The current level, clamped into the setting's range.
    pub(crate) fn level(self, snapshot: &ToolbarSnapshot) -> u8 {
        let level = match self {
            Self::Smoothing => snapshot.pen_smoothing,
            Self::ShapeDetection => snapshot.shape_recognition_sensitivity,
        };
        level.min(self.max())
    }

    /// Name of `level` ("Medium"), clamped into the setting's range.
    pub(crate) fn level_name(self, level: u8) -> &'static str {
        self.level_names()[usize::from(level.min(self.max()))]
    }

    /// Name of the current level.
    pub(crate) fn current_level_name(self, snapshot: &ToolbarSnapshot) -> &'static str {
        self.level_name(self.level(snapshot))
    }

    /// The event applying `level`.
    pub(crate) fn event(self, level: u8) -> ToolbarEvent {
        match self {
            Self::Smoothing => ToolbarEvent::SetPenSmoothing(level),
            Self::ShapeDetection => ToolbarEvent::SetShapeRecognitionSensitivity(level),
        }
    }

    /// Relative wheel movement, applied against the application's live level
    /// so delayed frontend snapshots cannot lose or replay notches.
    pub(crate) fn nudge_event(self, steps: i32) -> ToolbarEvent {
        match self {
            Self::Smoothing => ToolbarEvent::NudgePenSmoothing(steps),
            Self::ShapeDetection => ToolbarEvent::NudgeShapeRecognitionSensitivity(steps),
        }
    }

    /// The meter for this setting, its bars named `<id_prefix>.level-<n>`.
    pub(crate) fn meter(self, snapshot: &ToolbarSnapshot, id_prefix: &str) -> StylePillMeter {
        let level = self.level(snapshot);
        let max = self.max();
        let current = format!(
            "{}: {} ({level} of {max})",
            self.name(),
            self.level_name(level)
        );

        let segments = (1..=max)
            .map(|bar| {
                let target = clicked_level(level, bar);
                StylePillMeterSegment {
                    id: format!("{id_prefix}.level-{bar}"),
                    filled: bar <= level,
                    event: self.event(target),
                    tooltip: format!(
                        "{current}. Click for {}, or scroll to adjust",
                        self.level_name(target)
                    ),
                }
            })
            .collect();

        StylePillMeter {
            level,
            max,
            segments,
        }
    }

    /// The event `steps` whole wheel notches apply: that many levels up for
    /// `steps > 0`, down for `steps < 0`, clamped to the range. `None` when the
    /// level would not change, so a wheel at either end is ignored.
    pub(crate) fn wheel_event(
        self,
        snapshot: &ToolbarSnapshot,
        steps: i32,
    ) -> Option<ToolbarEvent> {
        self.wheel_target(self.level(snapshot), steps)
            .map(|target| self.event(target))
    }

    /// The level `steps` whole notches reach from `level`, clamped to the
    /// range; `None` when that is `level` itself. Both frontends step through
    /// this against the application's current level.
    pub(crate) fn wheel_target(self, level: u8, steps: i32) -> Option<u8> {
        let target = i32::from(level)
            .saturating_add(steps)
            .clamp(0, i32::from(self.max())) as u8;

        (target != level).then_some(target)
    }

    /// The setting a wheel over a clicked control steps, when that control
    /// is a meter bar.
    ///
    /// A bar's click event names its setting, so any hit carrying one steps
    /// that setting. The inline steppers send the same events but never took
    /// the wheel, so the stepper style keeps them that way.
    pub(crate) fn for_wheel(event: &ToolbarEvent, snapshot: &ToolbarSnapshot) -> Option<Self> {
        if snapshot.stroke_controls == crate::config::ToolbarStrokeControls::Stepper {
            return None;
        }
        match event {
            ToolbarEvent::SetPenSmoothing(_) => Some(Self::Smoothing),
            ToolbarEvent::SetShapeRecognitionSensitivity(_) => Some(Self::ShapeDetection),
            _ => None,
        }
    }
}

/// The level a click on bar `bar` (1-based) applies: that level, or one below
/// it when the bar is already the highest filled one.
fn clicked_level(level: u8, bar: u8) -> u8 {
    if bar == level { bar - 1 } else { bar }
}

impl StylePillControl {
    /// The setting an inline meter adjusts, `None` for every other control.
    pub(crate) fn meter_setting(self) -> Option<StrokeSetting> {
        match self {
            Self::PenSmoothingMeter => Some(StrokeSetting::Smoothing),
            Self::ShapeSensitivityMeter => Some(StrokeSetting::ShapeDetection),
            _ => None,
        }
    }

    /// The level meter for a meter control, `None` for every other control.
    pub(crate) fn meter(self, snapshot: &ToolbarSnapshot) -> Option<StylePillMeter> {
        Some(self.meter_setting()?.meter(snapshot, &self.id()))
    }

    /// Meter for a control already known to be one.
    pub(crate) fn required_meter(self, snapshot: &ToolbarSnapshot) -> StylePillMeter {
        self.meter(snapshot)
            .expect("this style-pill control is a level meter")
    }

    /// Name of the meter's current level ("Medium"), for readouts and
    /// accessible values.
    pub(crate) fn meter_level_name(self, snapshot: &ToolbarSnapshot) -> Option<&'static str> {
        Some(self.meter_setting()?.current_level_name(snapshot))
    }

    /// The event a wheel notch over this meter applies (see
    /// [`StrokeSetting::wheel_event`]).
    pub(crate) fn meter_wheel_event(
        self,
        snapshot: &ToolbarSnapshot,
        steps: i32,
    ) -> Option<ToolbarEvent> {
        self.meter_setting()?.wheel_event(snapshot, steps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ToolbarStrokeControls;

    fn snapshot(smoothing: u8, sensitivity: u8) -> ToolbarSnapshot {
        let mut state = crate::input::state::test_support::make_test_input_state();
        state.set_pen_smoothing(smoothing);
        state.set_shape_recognition_sensitivity(sensitivity);
        ToolbarSnapshot::from_input(&state)
    }

    #[test]
    fn smoothing_meter_has_one_bar_per_level_filled_up_to_the_current_one() {
        let meter = StylePillControl::PenSmoothingMeter.required_meter(&snapshot(3, 3));

        assert_eq!((meter.level, meter.max), (3, 6));
        let filled: Vec<bool> = meter
            .segments
            .iter()
            .map(|segment| segment.filled)
            .collect();
        assert_eq!(filled, [true, true, true, false, false, false]);
        assert_eq!(meter.segments[0].id, "top.style.pen-smoothing.level-1");
    }

    #[test]
    fn detection_meter_covers_its_own_range() {
        let meter = StylePillControl::ShapeSensitivityMeter.required_meter(&snapshot(3, 2));

        assert_eq!((meter.level, meter.max), (2, 4));
        assert_eq!(meter.segments.len(), 4);
    }

    #[test]
    fn a_bar_sets_its_level_and_the_top_filled_bar_steps_one_below() {
        let meter = StylePillControl::PenSmoothingMeter.required_meter(&snapshot(3, 3));

        assert_eq!(meter.segments[4].event, ToolbarEvent::SetPenSmoothing(5));
        assert_eq!(meter.segments[0].event, ToolbarEvent::SetPenSmoothing(1));
        assert_eq!(meter.segments[2].event, ToolbarEvent::SetPenSmoothing(2));

        let lowest = StylePillControl::PenSmoothingMeter.required_meter(&snapshot(1, 3));
        assert_eq!(lowest.segments[0].event, ToolbarEvent::SetPenSmoothing(0));
    }

    #[test]
    fn tooltips_name_the_current_level_and_the_click_target() {
        let meter = StylePillControl::ShapeSensitivityMeter.required_meter(&snapshot(3, 3));

        assert_eq!(
            meter.segments[3].tooltip,
            "Shape detection: Forgiving (3 of 4). Click for Very forgiving, or scroll to adjust"
        );
        assert_eq!(
            StylePillControl::PenSmoothingMeter.meter_level_name(&snapshot(0, 3)),
            Some("Off")
        );
    }

    #[test]
    fn the_wheel_steps_whole_notches_and_stops_at_either_end() {
        let control = StylePillControl::ShapeSensitivityMeter;

        assert_eq!(
            control.meter_wheel_event(&snapshot(3, 2), 1),
            Some(ToolbarEvent::SetShapeRecognitionSensitivity(3))
        );
        assert_eq!(
            control.meter_wheel_event(&snapshot(3, 1), 2),
            Some(ToolbarEvent::SetShapeRecognitionSensitivity(3)),
            "a coalesced two-notch frame moves two levels"
        );
        assert_eq!(
            control.meter_wheel_event(&snapshot(3, 2), 9),
            Some(ToolbarEvent::SetShapeRecognitionSensitivity(4)),
            "clamped to the range"
        );
        assert_eq!(
            control.meter_wheel_event(&snapshot(3, 2), -1),
            Some(ToolbarEvent::SetShapeRecognitionSensitivity(1))
        );
        assert_eq!(control.meter_wheel_event(&snapshot(3, 4), 1), None);
        assert_eq!(control.meter_wheel_event(&snapshot(3, 0), -1), None);
    }

    /// The GTK meters step from the level they last showed, so a multi-notch
    /// frame lands where the built-in meter's would.
    #[test]
    fn a_wheel_target_applies_every_notch_from_a_given_level() {
        let smoothing = StrokeSetting::Smoothing;

        assert_eq!(smoothing.wheel_target(1, 2), Some(3));
        assert_eq!(smoothing.wheel_target(1, -9), Some(0));
        assert_eq!(smoothing.wheel_target(0, -1), None);
        assert_eq!(smoothing.wheel_target(smoothing.max(), 1), None);
    }

    /// The panel's meters and the inline ones are the same model under a
    /// different id prefix.
    #[test]
    fn a_setting_meter_matches_the_inline_meter_under_its_own_ids() {
        let snapshot = snapshot(4, 1);
        let inline = StylePillControl::PenSmoothingMeter.required_meter(&snapshot);
        let panel = StrokeSetting::Smoothing.meter(&snapshot, "top.feel.smoothing");

        assert_eq!(panel.level, inline.level);
        assert_eq!(panel.segments.len(), inline.segments.len());
        for (panel_bar, inline_bar) in panel.segments.iter().zip(&inline.segments) {
            assert_eq!(panel_bar.event, inline_bar.event);
            assert_eq!(panel_bar.filled, inline_bar.filled);
            assert_eq!(panel_bar.tooltip, inline_bar.tooltip);
        }
        assert_eq!(panel.segments[0].id, "top.feel.smoothing.level-1");
    }

    #[test]
    fn a_wheel_over_a_meter_bar_steps_its_setting_except_on_steppers() {
        let mut snapshot = snapshot(3, 3);

        for style in [ToolbarStrokeControls::Panel, ToolbarStrokeControls::Meter] {
            snapshot.stroke_controls = style;
            assert_eq!(
                StrokeSetting::for_wheel(&ToolbarEvent::SetPenSmoothing(2), &snapshot),
                Some(StrokeSetting::Smoothing),
                "{style:?}"
            );
            assert_eq!(
                StrokeSetting::for_wheel(
                    &ToolbarEvent::SetShapeRecognitionSensitivity(2),
                    &snapshot
                ),
                Some(StrokeSetting::ShapeDetection),
                "{style:?}"
            );
            assert_eq!(
                StrokeSetting::for_wheel(&ToolbarEvent::CycleArrowStyle, &snapshot),
                None
            );
        }

        snapshot.stroke_controls = ToolbarStrokeControls::Stepper;
        assert_eq!(
            StrokeSetting::for_wheel(&ToolbarEvent::SetPenSmoothing(2), &snapshot),
            None,
            "the steppers never took the wheel"
        );
    }
}
