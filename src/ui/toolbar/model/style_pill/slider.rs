use super::{ToolbarEvent, ToolbarSliderSpec, ToolbarSnapshot};

/// Slider-only policy shared by the GTK and Cairo adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StylePillSlider {
    Thickness,
    Opacity,
    SpotlightMagnification,
    FontSize,
}

impl StylePillSlider {
    pub(crate) fn value(self, snapshot: &ToolbarSnapshot) -> (ToolbarSliderSpec, f64) {
        match self {
            Self::Thickness => (ToolbarSliderSpec::THICKNESS, snapshot.thickness),
            Self::Opacity => (ToolbarSliderSpec::MARKER_OPACITY, snapshot.marker_opacity),
            Self::SpotlightMagnification => (
                ToolbarSliderSpec::SPOTLIGHT_MAGNIFICATION,
                snapshot.spotlight_magnification,
            ),
            Self::FontSize => (ToolbarSliderSpec::FONT_SIZE, snapshot.font_size),
        }
    }

    pub(crate) fn event(self, value: f64) -> ToolbarEvent {
        match self {
            Self::Thickness => ToolbarEvent::SetThickness(value),
            Self::Opacity => ToolbarEvent::SetMarkerOpacity(value),
            Self::SpotlightMagnification => ToolbarEvent::SetSpotlightMagnification(value),
            Self::FontSize => ToolbarEvent::SetFontSize(value),
        }
    }

    pub(crate) fn formatter(self) -> fn(f64) -> String {
        match self {
            Self::Thickness => |value| format!("{value:.0}px"),
            Self::Opacity => |value| format!("{:.0}%", value * 100.0),
            Self::SpotlightMagnification => crate::draw::format_spotlight_magnification,
            Self::FontSize => |value| format!("{value:.0}pt"),
        }
    }
}
