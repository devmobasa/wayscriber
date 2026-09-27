use crate::domain::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};

use super::super::ToolbarEvent;

// Activation payloads are plain `ToolbarEvent` values on model controls. The
// historical module name now groups the IDs and slider math used to construct
// those event-bearing controls; it does not define an activation abstraction.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ToolbarControlId {
    LayoutModeSimple,
    LayoutModeRegular,
    LayoutModeAdvanced,
    SettingsContextAwareUi,
    SettingsIconMode,
    SettingsTextControls,
    SettingsStatusBar,
    StatusBarContents,
    BackStatusBarContents,
    SettingsStatusBarInteractive,
    SettingsStatusActiveOutput,
    SettingsStatusSelectionInfo,
    SettingsStatusBoardBadge,
    SettingsStatusPageBadge,
    SettingsStatusColor,
    SettingsStatusTool,
    SettingsStatusSize,
    SettingsStatusContextIndicators,
    SettingsStatusToolbarHint,
    SettingsStatusHelp,
    SettingsStatusAbout,
    SettingsFloatingBadgeAlways,
    SettingsPresetToasts,
    SettingsIdleFade,
    SettingsInputHud,
    SettingsPresets,
    SettingsActions,
    SettingsZoomActions,
    SettingsAdvancedActions,
    SettingsBoards,
    SettingsPages,
    SettingsStepControls,
    CustomizeToolbarItems,
    BackToolbarSettings,
    ResetToolbarHiddenItems,
    ResetToolbarItemOrder,
    OpenConfigurator,
    OpenConfigFile,
    OpenAbout,
    OpenCommandPalette,
    ResetRuntimeUi,
    ConfirmRuntimeUiReset,
    CancelRuntimeUiReset,
    RetryRuntimeUiPersistence,
    AdoptRuntimeUiFromDisk,
    PreserveInvalidRuntimeUi,
    CancelRuntimeUiRecovery,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ToolbarSlider {
    pub(crate) target: ToolbarSliderTarget,
    pub(crate) spec: ToolbarSliderSpec,
    pub(crate) value: f64,
}

impl ToolbarSlider {
    pub(crate) fn event_for_value(&self, value: f64) -> ToolbarEvent {
        let value = self.spec.normalize_value(value);
        match self.target {
            ToolbarSliderTarget::Thickness => ToolbarEvent::SetThickness(value),
            ToolbarSliderTarget::MarkerOpacity => ToolbarEvent::SetMarkerOpacity(value),
            ToolbarSliderTarget::SpotlightMagnification => {
                ToolbarEvent::SetSpotlightMagnification(value)
            }
            ToolbarSliderTarget::FontSize => ToolbarEvent::SetFontSize(value),
            ToolbarSliderTarget::UndoDelay => ToolbarEvent::SetUndoDelay(value),
            ToolbarSliderTarget::RedoDelay => ToolbarEvent::SetRedoDelay(value),
            ToolbarSliderTarget::CustomUndoDelay => ToolbarEvent::SetCustomUndoDelay(value),
            ToolbarSliderTarget::CustomRedoDelay => ToolbarEvent::SetCustomRedoDelay(value),
        }
    }

    pub(crate) fn event_for_pointer_x(
        &self,
        pointer_x: f64,
        hit_x: f64,
        hit_w: f64,
    ) -> ToolbarEvent {
        self.event_for_value(self.spec.value_from_pointer_x(pointer_x, hit_x, hit_w))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarSliderTarget {
    Thickness,
    MarkerOpacity,
    SpotlightMagnification,
    FontSize,
    UndoDelay,
    RedoDelay,
    CustomUndoDelay,
    CustomRedoDelay,
}

/// How a slider's track position maps to its value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SliderCurve {
    Linear,
    /// `value = min + span * t^exponent`. An exponent above 1 gives the low
    /// end of the range more of the track.
    Power(f64),
}

impl SliderCurve {
    /// Fraction of the value span at track position `t`.
    fn span_fraction(self, t: f64) -> f64 {
        match self {
            Self::Linear => t,
            Self::Power(exponent) => t.powf(exponent),
        }
    }

    /// Track position for a fraction of the value span.
    fn track_position(self, fraction: f64) -> f64 {
        match self {
            Self::Linear => fraction,
            Self::Power(exponent) => fraction.powf(exponent.recip()),
        }
    }
}

/// Stroke widths people use most sit at the low end, so half the thickness
/// track covers 1-10 px: the exponent is `log2(49 / 9)`, so that
/// `1 + 49 * 0.5^exponent = 10`.
const THICKNESS_CURVE_EXPONENT: f64 = 2.444_784_842_672_896;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ToolbarSliderSpec {
    pub(crate) min: f64,
    pub(crate) max: f64,
    pub(crate) step: Option<f64>,
    pub(crate) snap_to_step: bool,
    pub(crate) curve: SliderCurve,
    /// Values marked with a faint tick on the track.
    pub(crate) ticks: &'static [f64],
}

impl ToolbarSliderSpec {
    pub(crate) const FONT_SIZE: Self = Self {
        min: 8.0,
        max: 72.0,
        step: Some(2.0),
        snap_to_step: false,
        curve: SliderCurve::Linear,
        ticks: &[],
    };
    pub(crate) const DELAY_SECONDS: Self = Self {
        min: 0.05,
        max: 5.0,
        step: None,
        snap_to_step: false,
        curve: SliderCurve::Linear,
        ticks: &[],
    };
    pub(crate) const MARKER_OPACITY: Self = Self {
        min: 0.05,
        max: 0.9,
        step: Some(0.05),
        snap_to_step: false,
        curve: SliderCurve::Linear,
        ticks: &[],
    };
    pub(crate) const SPOTLIGHT_MAGNIFICATION: Self = Self {
        min: crate::draw::MIN_SPOTLIGHT_MAGNIFICATION,
        max: crate::draw::MAX_SPOTLIGHT_MAGNIFICATION,
        step: Some(crate::draw::SPOTLIGHT_MAGNIFICATION_STEP),
        snap_to_step: true,
        curve: SliderCurve::Linear,
        ticks: &[],
    };
    pub(crate) const THICKNESS: Self = Self {
        min: MIN_STROKE_THICKNESS,
        max: MAX_STROKE_THICKNESS,
        step: Some(1.0),
        snap_to_step: true,
        curve: SliderCurve::Power(THICKNESS_CURVE_EXPONENT),
        ticks: &[5.0, 10.0, 20.0],
    };

    pub(crate) fn clamp(self, value: f64) -> f64 {
        value.clamp(self.min, self.max)
    }

    pub(crate) fn normalize_value(self, value: f64) -> f64 {
        let clamped = self.clamp(value);
        if !self.snap_to_step {
            return clamped;
        }
        let Some(step) = self.step.filter(|step| step.is_finite() && *step > 0.0) else {
            return clamped;
        };
        (self.min + ((clamped - self.min) / step).round() * step).clamp(self.min, self.max)
    }

    pub(crate) fn value_from_t(self, t: f64) -> f64 {
        let fraction = self.curve.span_fraction(t.clamp(0.0, 1.0));

        self.normalize_value(self.min + fraction * self.span())
    }

    pub(crate) fn t_from_value(self, value: f64) -> f64 {
        let span = self.span();
        if span <= f64::EPSILON {
            return 0.0;
        }

        let fraction = ((self.clamp(value) - self.min) / span).clamp(0.0, 1.0);
        self.curve.track_position(fraction).clamp(0.0, 1.0)
    }

    /// Track positions of the spec's ticks, in `[0, 1]`.
    pub(crate) fn tick_positions(self) -> impl Iterator<Item = f64> {
        self.ticks
            .iter()
            .map(move |value| self.t_from_value(*value))
    }

    pub(crate) fn t_from_pointer_x(pointer_x: f64, hit_x: f64, hit_w: f64) -> f64 {
        if !hit_w.is_finite() || hit_w <= f64::EPSILON {
            return 0.0;
        }
        ((pointer_x - hit_x) / hit_w).clamp(0.0, 1.0)
    }

    pub(crate) fn value_from_pointer_x(self, pointer_x: f64, hit_x: f64, hit_w: f64) -> f64 {
        self.value_from_t(Self::t_from_pointer_x(pointer_x, hit_x, hit_w))
    }

    /// Inset travel for the slider geometry contract exercised below.
    #[cfg(test)]
    pub(crate) fn knob_center_x(
        self,
        track_x: f64,
        track_w: f64,
        knob_radius: f64,
        value: f64,
    ) -> f64 {
        let t = self.t_from_value(value);
        track_x + t * (track_w - knob_radius * 2.0) + knob_radius
    }

    fn span(self) -> f64 {
        self.max - self.min
    }
}

/// Convert a delay in milliseconds to normalized slider position [0, 1].
pub(crate) fn delay_t_from_ms(delay_ms: u64) -> f64 {
    ToolbarSliderSpec::DELAY_SECONDS.t_from_value(delay_ms as f64 / 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 0.000_001,
            "expected {expected}, got {actual}"
        );
    }

    /// Half the thickness track covers the widths people use most, 1-10 px,
    /// and every whole width round-trips through the curve to itself.
    #[test]
    fn thickness_track_gives_the_small_widths_half_its_length() {
        let spec = ToolbarSliderSpec::THICKNESS;

        assert_close(spec.value_from_t(0.5), 10.0);
        assert!(
            spec.t_from_value(5.0) > 0.3,
            "5 px is a third of the way in"
        );
        for width in 1..=50 {
            let width = f64::from(width);
            assert_close(spec.value_from_t(spec.t_from_value(width)), width);
        }
        assert_close(spec.t_from_value(1.0), 0.0);
        assert_close(spec.t_from_value(50.0), 1.0);
    }

    #[test]
    fn only_the_thickness_track_is_curved_and_ticked() {
        let ticks: Vec<f64> = ToolbarSliderSpec::THICKNESS.tick_positions().collect();

        assert_eq!(ticks.len(), 3);
        assert_close(ticks[1], 0.5);
        assert!(ticks.windows(2).all(|pair| pair[0] < pair[1]));
        for spec in [
            ToolbarSliderSpec::FONT_SIZE,
            ToolbarSliderSpec::MARKER_OPACITY,
            ToolbarSliderSpec::DELAY_SECONDS,
            ToolbarSliderSpec::SPOTLIGHT_MAGNIFICATION,
        ] {
            assert_eq!(spec.curve, SliderCurve::Linear);
            assert_eq!(spec.tick_positions().count(), 0);
            assert_close(spec.t_from_value((spec.min + spec.max) / 2.0), 0.5);
        }
    }

    #[test]
    fn slider_spec_maps_values_to_normalized_positions() {
        let spec = ToolbarSliderSpec {
            min: 10.0,
            max: 20.0,
            step: None,
            snap_to_step: false,
            curve: SliderCurve::Linear,
            ticks: &[],
        };

        assert_close(spec.t_from_value(10.0), 0.0);
        assert_close(spec.t_from_value(20.0), 1.0);
        assert_close(spec.t_from_value(15.0), 0.5);
        assert_close(spec.t_from_value(5.0), 0.0);
        assert_close(spec.t_from_value(25.0), 1.0);
    }

    #[test]
    fn slider_spec_maps_normalized_positions_to_values() {
        let spec = ToolbarSliderSpec {
            min: 10.0,
            max: 20.0,
            step: None,
            snap_to_step: false,
            curve: SliderCurve::Linear,
            ticks: &[],
        };

        assert_close(spec.value_from_t(0.0), 10.0);
        assert_close(spec.value_from_t(1.0), 20.0);
        assert_close(spec.value_from_t(0.5), 15.0);
        assert_close(spec.value_from_t(-1.0), 10.0);
        assert_close(spec.value_from_t(2.0), 20.0);
    }

    #[test]
    fn spotlight_slider_snaps_to_quarter_steps() {
        let spec = ToolbarSliderSpec::SPOTLIGHT_MAGNIFICATION;

        assert_close(spec.normalize_value(2.13), 2.25);
        assert_close(spec.normalize_value(0.5), 1.0);
        assert_close(spec.normalize_value(5.0), 4.0);

        let slider = ToolbarSlider {
            target: ToolbarSliderTarget::SpotlightMagnification,
            spec,
            value: 1.0,
        };
        match slider.event_for_value(2.13) {
            ToolbarEvent::SetSpotlightMagnification(value) => assert_close(value, 2.25),
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn thickness_slider_snaps_to_whole_pixels() {
        let slider = ToolbarSlider {
            target: ToolbarSliderTarget::Thickness,
            spec: ToolbarSliderSpec::THICKNESS,
            value: 1.0,
        };

        match slider.event_for_value(2.13) {
            ToolbarEvent::SetThickness(value) => assert_close(value, 2.0),
            other => panic!("unexpected event: {other:?}"),
        }
        let t = ToolbarSliderSpec::THICKNESS.t_from_value(2.13);
        assert_close(ToolbarSliderSpec::THICKNESS.value_from_t(t), 2.0);
    }

    #[test]
    fn pointer_mapping_uses_hit_rect_not_visual_knob_travel() {
        let spec = ToolbarSliderSpec {
            min: 10.0,
            max: 20.0,
            step: None,
            snap_to_step: false,
            curve: SliderCurve::Linear,
            ticks: &[],
        };

        assert_close(spec.value_from_pointer_x(100.0, 100.0, 200.0), 10.0);
        assert_close(spec.value_from_pointer_x(200.0, 100.0, 200.0), 15.0);
        assert_close(spec.value_from_pointer_x(300.0, 100.0, 200.0), 20.0);
        assert_close(spec.value_from_pointer_x(50.0, 100.0, 200.0), 10.0);
        assert_close(spec.value_from_pointer_x(350.0, 100.0, 200.0), 20.0);
    }

    #[test]
    fn visual_knob_mapping_uses_inset_travel_range() {
        let spec = ToolbarSliderSpec {
            min: 10.0,
            max: 20.0,
            step: None,
            snap_to_step: false,
            curve: SliderCurve::Linear,
            ticks: &[],
        };

        assert_close(spec.knob_center_x(100.0, 200.0, 8.0, 10.0), 108.0);
        assert_close(spec.knob_center_x(100.0, 200.0, 8.0, 20.0), 292.0);
        assert_close(spec.knob_center_x(100.0, 200.0, 8.0, 15.0), 200.0);
    }

    #[test]
    fn delay_helper_uses_delay_slider_spec() {
        assert_close(
            ToolbarSliderSpec::DELAY_SECONDS.value_from_t(0.0),
            ToolbarSliderSpec::DELAY_SECONDS.min,
        );
        assert_close(
            ToolbarSliderSpec::DELAY_SECONDS.value_from_t(1.0),
            ToolbarSliderSpec::DELAY_SECONDS.max,
        );

        let t = delay_t_from_ms(2525);
        assert_close(ToolbarSliderSpec::DELAY_SECONDS.value_from_t(t), 2.525);
    }

    #[test]
    fn slider_emits_event_from_pointer_position() {
        let slider = ToolbarSlider {
            target: ToolbarSliderTarget::Thickness,
            spec: ToolbarSliderSpec {
                min: 10.0,
                max: 20.0,
                step: None,
                snap_to_step: false,
                curve: SliderCurve::Linear,
                ticks: &[],
            },
            value: 10.0,
        };

        match slider.event_for_pointer_x(200.0, 100.0, 200.0) {
            ToolbarEvent::SetThickness(value) => assert_close(value, 15.0),
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
