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
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ToolbarColorPicker {
    pub(crate) color: Color,
}
