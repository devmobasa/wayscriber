use crate::draw::Color;
use crate::input::state::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};

use super::super::ToolbarEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ToolbarControlId {
    DragTop,
    DragSide,
    IconModeIcons,
    IconModeText,
    LayoutModeSimple,
    LayoutModeFull,
    PinTop,
    PinSide,
    CloseTop,
    CloseSide,
    DrawerMore,
    BoardChip,
    SettingsContextAwareUi,
    SettingsTextControls,
    SettingsStatusBar,
    SettingsStatusBoardBadge,
    SettingsStatusPageBadge,
    SettingsFloatingBadgeAlways,
    SettingsPresetToasts,
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
}

#[derive(Debug, Clone)]
pub(crate) enum ToolbarActivation {
    Click(ToolbarEvent),
    Drag(ToolbarDragTarget),
    Slider(ToolbarSlider),
    ColorPicker(ToolbarColorPicker),
    None,
}

impl ToolbarActivation {
    pub(crate) fn compatibility_event(&self) -> ToolbarEvent {
        match self {
            Self::Click(event) => event.clone(),
            Self::Drag(ToolbarDragTarget::MoveTopToolbar) => {
                ToolbarEvent::MoveTopToolbar { x: 0.0, y: 0.0 }
            }
            Self::Drag(ToolbarDragTarget::MoveSideToolbar) => {
                ToolbarEvent::MoveSideToolbar { x: 0.0, y: 0.0 }
            }
            Self::Slider(slider) => slider.event_for_value(slider.value),
            Self::ColorPicker(_) | Self::None => ToolbarEvent::OpenColorPickerPopup,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarDragTarget {
    MoveTopToolbar,
    MoveSideToolbar,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ToolbarSlider {
    pub(crate) target: ToolbarSliderTarget,
    pub(crate) spec: ToolbarSliderSpec,
    pub(crate) value: f64,
}

impl ToolbarSlider {
    pub(crate) fn event_for_value(&self, value: f64) -> ToolbarEvent {
        let value = self.spec.clamp(value);
        match self.target {
            ToolbarSliderTarget::Thickness => ToolbarEvent::SetThickness(value),
            ToolbarSliderTarget::MarkerOpacity => ToolbarEvent::SetMarkerOpacity(value),
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
}

impl ToolbarSliderSpec {
    pub(crate) const FONT_SIZE: Self = Self {
        min: 8.0,
        max: 72.0,
        step: Some(2.0),
    };
    pub(crate) const DELAY_SECONDS: Self = Self {
        min: 0.05,
        max: 5.0,
        step: None,
    };
    pub(crate) const MARKER_OPACITY: Self = Self {
        min: 0.05,
        max: 0.9,
        step: Some(0.05),
    };
    pub(crate) const THICKNESS: Self = Self {
        min: MIN_STROKE_THICKNESS,
        max: MAX_STROKE_THICKNESS,
        step: Some(1.0),
    };

    pub(crate) fn clamp(self, value: f64) -> f64 {
        value.clamp(self.min, self.max)
    }

    pub(crate) fn value_from_t(self, t: f64) -> f64 {
        self.clamp(self.min + t.clamp(0.0, 1.0) * self.span())
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

/// Convert normalized delay slider position [0, 1] to seconds.
pub(crate) fn delay_secs_from_t(t: f64) -> f64 {
    ToolbarSliderSpec::DELAY_SECONDS.value_from_t(t)
}

/// Convert a delay in milliseconds to normalized slider position [0, 1].
pub(crate) fn delay_t_from_ms(delay_ms: u64) -> f64 {
    ToolbarSliderSpec::DELAY_SECONDS.t_from_value(delay_ms as f64 / 1000.0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ToolbarColorPicker {
    pub(crate) color: Color,
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
        };

        assert_close(spec.value_from_t(0.0), 10.0);
        assert_close(spec.value_from_t(1.0), 20.0);
        assert_close(spec.value_from_t(0.5), 15.0);
        assert_close(spec.value_from_t(-1.0), 10.0);
        assert_close(spec.value_from_t(2.0), 20.0);
    }

    #[test]
    fn pointer_mapping_uses_hit_rect_not_visual_knob_travel() {
        let spec = ToolbarSliderSpec {
            min: 10.0,
            max: 20.0,
            step: None,
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
        };

        assert_close(spec.knob_center_x(100.0, 200.0, 8.0, 10.0), 108.0);
        assert_close(spec.knob_center_x(100.0, 200.0, 8.0, 20.0), 292.0);
        assert_close(spec.knob_center_x(100.0, 200.0, 8.0, 15.0), 200.0);
    }

    #[test]
    fn delay_helpers_use_delay_slider_spec() {
        assert_close(delay_secs_from_t(0.0), ToolbarSliderSpec::DELAY_SECONDS.min);
        assert_close(delay_secs_from_t(1.0), ToolbarSliderSpec::DELAY_SECONDS.max);

        let t = delay_t_from_ms(2525);
        assert_close(delay_secs_from_t(t), 2.525);
    }

    #[test]
    fn slider_emits_event_from_pointer_position() {
        let slider = ToolbarSlider {
            target: ToolbarSliderTarget::Thickness,
            spec: ToolbarSliderSpec {
                min: 10.0,
                max: 20.0,
                step: None,
            },
            value: 10.0,
        };

        match slider.event_for_pointer_x(200.0, 100.0, 200.0) {
            ToolbarEvent::SetThickness(value) => assert_close(value, 15.0),
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
