use crate::config::{ArrowConfig, DrawingConfig, QuickColorPalette, SpotlightConfig};
use crate::draw::{ArrowStyle, BlurStyle, Color, EraserKind, FontDescriptor, clamp_regular_sides};
use crate::input::state::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};
use crate::input::{EraserMode, PerToolDrawingSettings, Tool};

use super::{PressureThicknessEditMode, PressureThicknessEntryMode};

/// Runtime drawing defaults and per-tool appearance settings.
#[derive(Debug, Clone)]
pub struct DrawingStyle {
    pub current_color: Color,
    pub(crate) quick_colors: QuickColorPalette,
    pub(crate) recent_colors: Vec<Color>,
    pub current_thickness: f64,
    pub(crate) tool_settings: PerToolDrawingSettings,
    pub(crate) pressure_variation_threshold: f64,
    pub(crate) pressure_thickness_edit_mode: PressureThicknessEditMode,
    pub(crate) pressure_thickness_entry_mode: PressureThicknessEntryMode,
    pub(crate) pressure_thickness_scale_step: f64,
    pub eraser_size: f64,
    pub eraser_kind: EraserKind,
    pub eraser_mode: EraserMode,
    pub marker_opacity: f64,
    pub pen_smoothing: u8,
    pub blur_style: BlurStyle,
    pub spotlight_dim_opacity: f64,
    pub spotlight_feather: f64,
    pub spotlight_magnification: f64,
    pub current_font_size: f64,
    pub font_descriptor: FontDescriptor,
    pub(crate) font_cycle: Vec<String>,
    pub text_background_enabled: bool,
    pub text_wrap_width: Option<i32>,
    pub arrow_length: f64,
    pub arrow_angle: f64,
    pub arrow_head_at_end: bool,
    pub arrow_style: ArrowStyle,
    pub arrow_label_enabled: bool,
    pub arrow_label_counter: u32,
    pub step_marker_counter: u32,
    pub fill_enabled: bool,
    pub polygon_sides: u8,
    pub(in crate::input::state::core) tool_override: Option<Tool>,
}

impl From<(&DrawingConfig, &ArrowConfig, &SpotlightConfig)> for DrawingStyle {
    fn from((drawing, arrow, spotlight): (&DrawingConfig, &ArrowConfig, &SpotlightConfig)) -> Self {
        let current_color = drawing.default_color.to_color();
        let current_thickness = drawing.default_thickness;
        let current_font_size = drawing.default_font_size;
        let mut tool_settings = PerToolDrawingSettings::new(current_color, current_thickness);
        tool_settings.step_marker.thickness =
            super::utility::default_step_marker_size(current_font_size);

        Self {
            current_color,
            quick_colors: QuickColorPalette::from_config(&drawing.quick_colors),
            recent_colors: Vec::new(),
            current_thickness,
            tool_settings,
            pressure_variation_threshold: 0.1,
            pressure_thickness_edit_mode: PressureThicknessEditMode::Disabled,
            pressure_thickness_entry_mode: PressureThicknessEntryMode::PressureOnly,
            pressure_thickness_scale_step: 0.1,
            eraser_size: drawing
                .default_eraser_size
                .clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS),
            eraser_kind: EraserKind::Circle,
            eraser_mode: drawing.default_eraser_mode,
            marker_opacity: drawing.marker_opacity,
            pen_smoothing: crate::draw::shape::clamp_pen_smoothing(drawing.pen_smoothing),
            blur_style: drawing.default_blur_style,
            spotlight_dim_opacity: spotlight.dim_opacity,
            spotlight_feather: spotlight.feather,
            spotlight_magnification: spotlight.magnification,
            current_font_size,
            font_descriptor: FontDescriptor::new(
                drawing.font_family.clone(),
                drawing.font_weight.clone(),
                drawing.font_style.clone(),
            ),
            font_cycle: drawing.font_cycle.clone(),
            text_background_enabled: drawing.text_background_enabled,
            text_wrap_width: None,
            arrow_length: arrow.length,
            arrow_angle: arrow.angle_degrees,
            arrow_head_at_end: arrow.head_at_end,
            arrow_style: arrow.style,
            arrow_label_enabled: false,
            arrow_label_counter: 1,
            step_marker_counter: 1,
            fill_enabled: drawing.default_fill_enabled,
            polygon_sides: clamp_regular_sides(drawing.polygon_sides),
            tool_override: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawing_style_maps_each_config_owner() {
        let drawing = DrawingConfig {
            default_thickness: 13.0,
            default_eraser_size: 29.0,
            default_eraser_mode: EraserMode::Stroke,
            default_blur_style: BlurStyle::BlackOut,
            marker_opacity: 0.41,
            pen_smoothing: 5,
            default_fill_enabled: true,
            polygon_sides: 9,
            default_font_size: 38.0,
            font_family: "Fixture Sans".into(),
            font_weight: "bold".into(),
            font_style: "italic".into(),
            font_cycle: vec!["Fixture Sans".into(), "Fixture Serif".into()],
            text_background_enabled: true,
            ..Default::default()
        };

        let arrow = ArrowConfig {
            length: 31.0,
            angle_degrees: 43.0,
            head_at_end: false,
            style: ArrowStyle::Curved,
        };
        let spotlight = SpotlightConfig {
            dim_opacity: 0.72,
            feather: 0.21,
            magnification: 2.5,
        };

        let style = DrawingStyle::from((&drawing, &arrow, &spotlight));

        assert_eq!(
            (
                style.current_thickness,
                style.eraser_size,
                style.eraser_mode,
                style.blur_style,
                style.marker_opacity,
                style.pen_smoothing,
                style.fill_enabled,
                style.polygon_sides,
                style.current_font_size,
                style.font_descriptor.family.as_str(),
                style.font_cycle.as_slice(),
                style.text_background_enabled,
            ),
            (
                13.0,
                29.0,
                EraserMode::Stroke,
                BlurStyle::BlackOut,
                0.41,
                5,
                true,
                9,
                38.0,
                "Fixture Sans",
                ["Fixture Sans".to_string(), "Fixture Serif".to_string()].as_slice(),
                true,
            )
        );
        assert_eq!(
            (
                style.arrow_length,
                style.arrow_angle,
                style.arrow_head_at_end,
                style.arrow_style,
            ),
            (31.0, 43.0, false, ArrowStyle::Curved)
        );
        assert_eq!(
            (
                style.spotlight_dim_opacity,
                style.spotlight_feather,
                style.spotlight_magnification,
            ),
            (0.72, 0.21, 2.5)
        );
        assert_eq!(
            style.tool_settings.step_marker.thickness,
            super::super::utility::default_step_marker_size(38.0)
        );
    }
}
