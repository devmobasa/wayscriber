use crate::input::state::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ToolbarSliderSpec {
    pub(crate) min: f64,
    pub(crate) max: f64,
    pub(crate) step: Option<f64>,
    pub(crate) snap_to_step: bool,
}

impl ToolbarSliderSpec {
    pub(crate) const FONT_SIZE: Self = Self {
        min: 8.0,
        max: 72.0,
        step: Some(2.0),
        snap_to_step: false,
    };
    pub(crate) const DELAY_SECONDS: Self = Self {
        min: 0.05,
        max: 5.0,
        step: None,
        snap_to_step: false,
    };
    pub(crate) const MARKER_OPACITY: Self = Self {
        min: 0.05,
        max: 0.9,
        step: Some(0.05),
        snap_to_step: false,
    };
    pub(crate) const SPOTLIGHT_MAGNIFICATION: Self = Self {
        min: crate::draw::MIN_SPOTLIGHT_MAGNIFICATION,
        max: crate::draw::MAX_SPOTLIGHT_MAGNIFICATION,
        step: Some(crate::draw::SPOTLIGHT_MAGNIFICATION_STEP),
        snap_to_step: true,
    };
    pub(crate) const THICKNESS: Self = Self {
        min: MIN_STROKE_THICKNESS,
        max: MAX_STROKE_THICKNESS,
        step: Some(1.0),
        snap_to_step: false,
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
        self.normalize_value(self.min + t.clamp(0.0, 1.0) * self.span())
    }

    pub(crate) fn t_from_value(self, value: f64) -> f64 {
        let span = self.span();
        if span <= f64::EPSILON {
            return 0.0;
        }
        ((self.clamp(value) - self.min) / span).clamp(0.0, 1.0)
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

    #[test]
    fn slider_spec_maps_values_to_normalized_positions() {
        let spec = ToolbarSliderSpec {
            min: 10.0,
            max: 20.0,
            step: None,
            snap_to_step: false,
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
    fn existing_sliders_remain_continuous() {
        let slider = ToolbarSlider {
            target: ToolbarSliderTarget::Thickness,
            spec: ToolbarSliderSpec::THICKNESS,
            value: 1.0,
        };

        match slider.event_for_value(2.13) {
            ToolbarEvent::SetThickness(value) => assert_close(value, 2.13),
            other => panic!("unexpected event: {other:?}"),
        }
        let t = ToolbarSliderSpec::THICKNESS.t_from_value(2.13);
        assert_close(ToolbarSliderSpec::THICKNESS.value_from_t(t), 2.13);
    }

    #[test]
    fn pointer_mapping_uses_hit_rect_not_visual_knob_travel() {
        let spec = ToolbarSliderSpec {
            min: 10.0,
            max: 20.0,
            step: None,
            snap_to_step: false,
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
            },
            value: 10.0,
        };

        match slider.event_for_pointer_x(200.0, 100.0, 200.0) {
            ToolbarEvent::SetThickness(value) => assert_close(value, 15.0),
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
