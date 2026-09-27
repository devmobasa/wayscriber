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
    /// Gradient stops as (normalized slider position, stroke alpha), including
    /// the transitions into and out of the canvas's clamped alpha range.
    pub(crate) alpha_stops: [(f64, f64); 4],
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

    /// The slider a wheel over a style-pill hit steps: its track, or the
    /// numeral beside it, which opens precise entry on click.
    pub(crate) fn for_wheel(event: &ToolbarEvent) -> Option<Self> {
        use crate::ui::toolbar::PrecisionEntryTarget;

        match event {
            ToolbarEvent::SetThickness(_)
            | ToolbarEvent::OpenPrecisionEntry(PrecisionEntryTarget::Thickness) => {
                Some(Self::Thickness)
            }
            ToolbarEvent::SetMarkerOpacity(_) => Some(Self::Opacity),
            ToolbarEvent::SetSpotlightMagnification(_) => Some(Self::SpotlightMagnification),
            ToolbarEvent::SetFontSize(_)
            | ToolbarEvent::OpenPrecisionEntry(PrecisionEntryTarget::FontSize) => {
                Some(Self::FontSize)
            }
            _ => None,
        }
    }

    /// A relative step by `steps` spec steps, for sliders the input side
    /// steps itself: thickness follows the active tool (eraser size included)
    /// and lands on whole pixels, text size clamps to its range.
    pub(crate) fn nudge_event(self, steps: i32) -> Option<ToolbarEvent> {
        let steps = f64::from(steps);

        match self {
            Self::Thickness => Some(ToolbarEvent::NudgeThickness(
                steps * ToolbarSliderSpec::THICKNESS.step.unwrap_or(1.0),
            )),
            Self::FontSize => Some(ToolbarEvent::NudgeFontSize(
                steps * ToolbarSliderSpec::FONT_SIZE.step.unwrap_or(1.0),
            )),
            Self::Opacity | Self::SpotlightMagnification => None,
        }
    }

    /// The event a wheel stepping `steps` spec steps (positive raises the
    /// value) applies, from the value in `snapshot`.
    pub(crate) fn wheel_event(self, snapshot: &ToolbarSnapshot, steps: i32) -> ToolbarEvent {
        if let Some(event) = self.nudge_event(steps) {
            return event;
        }

        let (spec, value) = self.value(snapshot);
        self.event(spec.step_value(value, f64::from(steps)))
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
            let min_alpha = stroke_alpha(spec.min);
            let max_alpha = stroke_alpha(spec.max);
            let position_for_alpha = |alpha| {
                if color.a > 0.0 {
                    spec.t_from_value(alpha / color.a)
                } else {
                    0.0
                }
            };

            OpacityPaint {
                rgb: (color.r, color.g, color.b),
                stroke_alpha: stroke_alpha(spec.clamp(value)),
                alpha_stops: [
                    (0.0, min_alpha),
                    (position_for_alpha(min_alpha), min_alpha),
                    (position_for_alpha(max_alpha), max_alpha),
                    (1.0, max_alpha),
                ],
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

    /// A wheel over a slider's track or its numeral steps that slider:
    /// thickness and text size relatively (so eraser size and whole pixels
    /// follow the input side), opacity from the snapshot's value.
    #[test]
    fn a_wheel_over_a_track_or_numeral_steps_that_slider() {
        use crate::ui::toolbar::PrecisionEntryTarget;

        let thickness = StylePillSlider::for_wheel(&ToolbarEvent::SetThickness(12.0));
        let numeral = StylePillSlider::for_wheel(&ToolbarEvent::OpenPrecisionEntry(
            PrecisionEntryTarget::Thickness,
        ));
        assert_eq!(thickness, Some(StylePillSlider::Thickness));
        assert_eq!(numeral, Some(StylePillSlider::Thickness));
        assert_eq!(
            StylePillSlider::for_wheel(&ToolbarEvent::OpenPrecisionEntry(
                PrecisionEntryTarget::FontSize
            )),
            Some(StylePillSlider::FontSize)
        );
        assert_eq!(StylePillSlider::for_wheel(&ToolbarEvent::Undo), None);

        let snapshot = snapshot(
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            0.4,
        );
        assert_eq!(
            StylePillSlider::Thickness.wheel_event(&snapshot, -2),
            ToolbarEvent::NudgeThickness(-2.0)
        );
        assert_eq!(
            StylePillSlider::FontSize.wheel_event(&snapshot, 1),
            ToolbarEvent::NudgeFontSize(2.0)
        );
        match StylePillSlider::Opacity.wheel_event(&snapshot, 1) {
            ToolbarEvent::SetMarkerOpacity(value) => assert!((value - 0.45).abs() < 1e-9),
            other => panic!("unexpected {other:?}"),
        }
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
        assert!((paint.alpha_stops[0].1 - 0.05).abs() < 1e-9, "{paint:?}");
        assert!((paint.alpha_stops[3].1 - 0.18).abs() < 1e-9, "{paint:?}");
        assert_eq!(
            StylePillSlider::Thickness.opacity_paint(&snapshot(translucent, 0.9)),
            None
        );
    }
}
