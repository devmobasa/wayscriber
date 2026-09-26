use super::{ToolbarEvent, ToolbarSliderSpec, ToolbarSnapshot};

/// What the marker opacity slider paints instead of a plain track and a
/// percentage: its track fades from clear to solid in the current color, and
/// a swatch shows a stroke at the current opacity over sample text (see
/// `toolbar_icons::draw_opacity_track` and `draw_opacity_swatch`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OpacityPaint {
    pub(crate) rgb: (f64, f64, f64),
    /// The alpha a marker stroke gets at the slider's value: the color's own
    /// alpha times the marker opacity, clamped as the canvas clamps it.
    pub(crate) stroke_alpha: f64,
    /// The stroke alphas at the slider's two ends, the ends of the track's
    /// fade.
    pub(crate) alpha_range: (f64, f64),
}

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

    /// The opacity paint, for the marker opacity slider only; the other
    /// sliders keep the plain track and their numeric readout. Alphas come
    /// from the calculation the canvas uses for a marker stroke, so a
    /// translucent color previews as translucent as it will draw.
    pub(crate) fn opacity_paint(self, snapshot: &ToolbarSnapshot) -> Option<OpacityPaint> {
        (self == Self::Opacity).then(|| {
            let (spec, value) = self.value(snapshot);
            let color = snapshot.color;
            let stroke_alpha =
                |opacity: f64| crate::input::tool::marker_color_with_opacity(color, opacity).a;
            OpacityPaint {
                rgb: (color.r, color.g, color.b),
                stroke_alpha: stroke_alpha(spec.clamp(value)),
                alpha_range: (stroke_alpha(spec.min), stroke_alpha(spec.max)),
            }
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::Color;
    use crate::input::state::test_support::make_test_input_state;
    use crate::ui::toolbar::ToolbarBindingHints;

    fn snapshot(color: Color, marker_opacity: f64) -> ToolbarSnapshot {
        let state = make_test_input_state();
        let mut snapshot =
            ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default());
        snapshot.color = color;
        snapshot.marker_opacity = marker_opacity;
        snapshot
    }

    /// The previews show the alpha the stroke will actually get: a 20% color
    /// at 90% marker opacity draws at 18%, and the track's fade spans what the
    /// slider can reach with that color.
    #[test]
    fn the_opacity_paint_uses_the_stroke_alpha_of_a_translucent_color() {
        let translucent = Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 0.2,
        };

        let paint = StylePillSlider::Opacity
            .opacity_paint(&snapshot(translucent, 0.9))
            .expect("opacity paint");

        assert!((paint.stroke_alpha - 0.18).abs() < 1e-9, "{paint:?}");
        assert!((paint.alpha_range.0 - 0.05).abs() < 1e-9, "{paint:?}");
        assert!((paint.alpha_range.1 - 0.18).abs() < 1e-9, "{paint:?}");
        assert_eq!(
            StylePillSlider::Thickness.opacity_paint(&snapshot(translucent, 0.9)),
            None
        );
    }
}
