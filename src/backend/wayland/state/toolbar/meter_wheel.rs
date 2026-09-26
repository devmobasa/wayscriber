//! Wheel steps for the built-in style pill's level meters.
//!
//! One meter level per wheel notch. A high-resolution wheel reports a notch as
//! several `value120` frames and a touchpad as a stream of continuous deltas,
//! so neither frame alone is a notch: partial amounts accumulate here, per
//! meter, until they add up to whole levels. The GTK meters get the same
//! behavior from GTK's discrete scroll controller.

use crate::ui::toolbar::model::StrokeSetting;

/// `value120` units in one wheel notch.
const VALUE120_PER_LEVEL: f64 = 120.0;

/// Continuous (touchpad) scroll units in one level: the size of one legacy
/// wheel detent, so a finger swipe moves about as far as a wheel notch.
const CONTINUOUS_PER_LEVEL: f64 = 15.0;

/// Which unit stream a partial amount belongs to. A remainder never carries
/// from one stream into the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WheelUnit {
    Value120,
    Continuous,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PartialScroll {
    setting: StrokeSetting,
    unit: WheelUnit,
    amount: f64,
}

/// Partial wheel travel over one meter, waiting to make a whole level.
#[derive(Debug, Default)]
pub(in crate::backend::wayland) struct MeterWheel {
    partial: Option<PartialScroll>,
}

impl MeterWheel {
    /// Folds one vertical axis frame over the meter for `setting` into whole
    /// levels, in Wayland's sign (positive scrolls down). A frame carrying
    /// `value120` or continuous travel may complete zero, one, or several
    /// levels; a legacy discrete frame is its own count of notches.
    pub(in crate::backend::wayland) fn levels(
        &mut self,
        setting: StrokeSetting,
        value120: i32,
        discrete: i32,
        absolute: f64,
    ) -> i32 {
        if value120 != 0 {
            self.accumulate(
                setting,
                WheelUnit::Value120,
                f64::from(value120),
                VALUE120_PER_LEVEL,
            )
        } else if discrete != 0 {
            self.partial = None;
            discrete
        } else if absolute != 0.0 {
            self.accumulate(
                setting,
                WheelUnit::Continuous,
                absolute,
                CONTINUOUS_PER_LEVEL,
            )
        } else {
            0
        }
    }

    /// Whether a partial level is waiting for more travel.
    pub(in crate::backend::wayland) fn is_pending(&self) -> bool {
        self.partial.is_some()
    }

    /// Drops any partial travel, when the pointer leaves the meter or the
    /// scroll sequence ends.
    pub(in crate::backend::wayland) fn reset(&mut self) {
        self.partial = None;
    }

    /// Keeps partial travel only while the pointer stays on the meter it was
    /// gathered over; `None` (off every meter) or another meter drops it.
    pub(in crate::backend::wayland) fn keep_only(&mut self, setting: Option<StrokeSetting>) {
        if self.partial.map(|partial| partial.setting) != setting {
            self.partial = None;
        }
    }

    fn accumulate(
        &mut self,
        setting: StrokeSetting,
        unit: WheelUnit,
        delta: f64,
        per_level: f64,
    ) -> i32 {
        let previous = self
            .partial
            .filter(|partial| partial.setting == setting && partial.unit == unit)
            .map_or(0.0, |partial| partial.amount);
        let total = previous + delta;
        let levels = (total / per_level).trunc();
        let amount = total - levels * per_level;

        self.partial = (amount != 0.0).then_some(PartialScroll {
            setting,
            unit,
            amount,
        });
        levels as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMOOTHING: StrokeSetting = StrokeSetting::Smoothing;

    #[test]
    fn quarter_notch_value120_frames_make_one_level() {
        let mut wheel = MeterWheel::default();

        let levels: Vec<i32> = (0..4)
            .map(|_| wheel.levels(SMOOTHING, -30, 0, 0.0))
            .collect();

        assert_eq!(levels, [0, 0, 0, -1]);
    }

    #[test]
    fn a_coalesced_frame_applies_every_whole_notch() {
        let mut wheel = MeterWheel::default();

        assert_eq!(wheel.levels(SMOOTHING, 240, 0, 0.0), 2);
        assert_eq!(wheel.levels(SMOOTHING, 180, 0, 0.0), 1);
        assert_eq!(wheel.levels(SMOOTHING, 60, 0, 0.0), 1);
    }

    #[test]
    fn touchpad_travel_accumulates_to_a_detent_per_level() {
        let mut wheel = MeterWheel::default();

        let levels: i32 = (0..6).map(|_| wheel.levels(SMOOTHING, 0, 0, 5.0)).sum();

        assert_eq!(levels, 2);
    }

    #[test]
    fn frames_too_small_to_name_a_direction_still_add_up() {
        let mut wheel = MeterWheel::default();

        let levels: i32 = [-14.0, -0.0625, -0.9375]
            .into_iter()
            .map(|absolute| wheel.levels(SMOOTHING, 0, 0, absolute))
            .sum();

        assert_eq!(levels, -1);
        assert!(!wheel.is_pending(), "the three frames make exactly a level");
    }

    #[test]
    fn a_legacy_discrete_notch_steps_at_once_and_drops_partials() {
        let mut wheel = MeterWheel::default();

        assert_eq!(wheel.levels(SMOOTHING, -60, 0, 0.0), 0);
        assert_eq!(wheel.levels(SMOOTHING, 0, -1, -10.0), -1);
        assert_eq!(wheel.levels(SMOOTHING, -60, 0, 0.0), 0);
    }

    #[test]
    fn partial_travel_does_not_carry_to_another_meter_or_after_a_reset() {
        let mut wheel = MeterWheel::default();

        assert_eq!(wheel.levels(SMOOTHING, -90, 0, 0.0), 0);
        assert_eq!(
            wheel.levels(StrokeSetting::ShapeDetection, -90, 0, 0.0),
            0,
            "a new meter starts from zero"
        );

        wheel.reset();
        assert_eq!(wheel.levels(SMOOTHING, -90, 0, 0.0), 0);
    }

    #[test]
    fn partial_travel_survives_only_on_its_own_meter() {
        let mut wheel = MeterWheel::default();

        assert_eq!(wheel.levels(SMOOTHING, -90, 0, 0.0), 0);
        wheel.keep_only(Some(SMOOTHING));
        assert!(wheel.is_pending(), "moving within the meter keeps it");
        assert_eq!(wheel.levels(SMOOTHING, -30, 0, 0.0), -1);

        assert_eq!(wheel.levels(SMOOTHING, -90, 0, 0.0), 0);
        wheel.keep_only(None);
        assert!(!wheel.is_pending(), "leaving the meter drops it");
        assert_eq!(
            wheel.levels(SMOOTHING, -30, 0, 0.0),
            0,
            "a return visit starts from zero"
        );

        wheel.keep_only(Some(StrokeSetting::ShapeDetection));
        assert!(!wheel.is_pending(), "another meter drops it too");
    }
}
