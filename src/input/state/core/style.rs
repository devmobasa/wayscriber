use crate::config::{
    ARROW_ANGLE_MAX, ARROW_ANGLE_MIN, ARROW_LENGTH_MAX, ARROW_LENGTH_MIN, ArrowConfig,
    DrawingConfig, MouseDragToolsConfig, PresetToolStatesConfig, QuickColorPalette,
    SpotlightConfig, ToolPresetConfig,
};
use crate::draw::{ArrowStyle, BlurStyle, Color, EraserKind, FontDescriptor, clamp_regular_sides};
use crate::input::state::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};
use crate::input::{EraserMode, PerToolDrawingSettings, Tool};
use crate::session::ToolStateSnapshot;
use crate::ui::toolbar::model::ToolbarSliderSpec;

use super::{PressureThicknessEditMode, PressureThicknessEntryMode};

/// Maximum number of session-only recently applied colors.
pub(crate) const RECENT_COLORS_CAP: usize = 6;

/// Runtime drawing defaults and per-tool appearance settings.
#[derive(Debug, Clone)]
pub(crate) struct DrawingStyle {
    pub(crate) current_color: Color,
    pub(crate) quick_colors: QuickColorPalette,
    pub(crate) recent_colors: Vec<Color>,
    pub(crate) current_thickness: f64,
    pub(crate) tool_settings: PerToolDrawingSettings,
    pub(crate) pressure_variation_threshold: f64,
    pub(crate) pressure_thickness_edit_mode: PressureThicknessEditMode,
    pub(crate) pressure_thickness_entry_mode: PressureThicknessEntryMode,
    pub(crate) pressure_thickness_scale_step: f64,
    pub(crate) eraser_size: f64,
    pub(crate) eraser_kind: EraserKind,
    pub(crate) eraser_mode: EraserMode,
    pub(crate) marker_opacity: f64,
    pub(crate) pen_smoothing: u8,
    pub(crate) blur_style: BlurStyle,
    pub(crate) spotlight_dim_opacity: f64,
    pub(crate) spotlight_feather: f64,
    pub(crate) spotlight_magnification: f64,
    pub(crate) current_font_size: f64,
    pub(crate) font_descriptor: FontDescriptor,
    pub(crate) font_cycle: Vec<String>,
    pub(crate) text_background_enabled: bool,
    pub(crate) text_wrap_width: Option<i32>,
    pub(crate) arrow_length: f64,
    pub(crate) arrow_angle: f64,
    pub(crate) arrow_head_at_end: bool,
    pub(crate) arrow_style: ArrowStyle,
    pub(crate) arrow_label_enabled: bool,
    pub(crate) arrow_label_counter: u32,
    pub(crate) step_marker_counter: u32,
    pub(crate) fill_enabled: bool,
    pub(crate) polygon_sides: u8,
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

impl DrawingStyle {
    pub(crate) fn color_for_tool(&self, tool: Tool) -> Color {
        self.tool_settings.get(tool).color
    }

    pub(crate) fn thickness_for_tool(&self, tool: Tool) -> f64 {
        if tool.uses_eraser_size() {
            self.eraser_size
        } else {
            self.tool_settings.get(tool).thickness
        }
    }

    pub(crate) fn replace_tool_settings(
        &mut self,
        settings: PerToolDrawingSettings,
        active_tool: Tool,
    ) {
        self.tool_settings = settings;
        self.sync_current_settings(active_tool);
    }

    pub(crate) fn sync_current_settings(&mut self, tool: Tool) {
        self.current_color = self.color_for_tool(tool);
        if tool.uses_drawing_thickness() {
            self.current_thickness = self.thickness_for_tool(tool);
        }
    }

    pub(crate) fn set_pen_color(&mut self, color: Color, active_tool: Tool) {
        self.tool_settings.pen.color = color;
        if PerToolDrawingSettings::settings_tool(active_tool) == Tool::Pen {
            self.current_color = color;
        }
    }

    pub(crate) fn preview_color(&mut self, tool: Tool, active_tool: Tool, color: Color) -> bool {
        if self.color_for_tool(tool) == color {
            return false;
        }
        self.tool_settings.get_mut(tool).color = color;
        if active_tool.settings_slot() == tool.settings_slot() {
            self.current_color = color;
        }
        true
    }

    #[cfg(feature = "tablet-input")]
    pub(crate) fn set_pressure_thickness(&mut self, tool: Tool, thickness: f64) -> f64 {
        let clamped = thickness.clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
        if tool.uses_drawing_thickness() {
            self.tool_settings.get_mut(tool).thickness = clamped;
        }
        self.current_thickness = clamped;
        clamped
    }

    pub(crate) fn set_tool_override(&mut self, tool: Option<Tool>) -> bool {
        if self.tool_override == tool {
            return false;
        }
        self.tool_override = tool;
        true
    }

    pub(crate) fn set_marker_opacity(&mut self, opacity: f64) -> bool {
        let spec = ToolbarSliderSpec::MARKER_OPACITY;
        let clamped = opacity.clamp(spec.min, spec.max);
        if (clamped - self.marker_opacity).abs() < f64::EPSILON {
            return false;
        }
        self.marker_opacity = clamped;
        true
    }

    pub(crate) fn nudge_pen_smoothing(&mut self, delta: i32) -> bool {
        let next = crate::draw::shape::clamp_pen_smoothing(
            i32::from(self.pen_smoothing)
                .saturating_add(delta)
                .clamp(0, i32::from(crate::draw::shape::MAX_PEN_SMOOTHING)) as u8,
        );
        self.set_pen_smoothing(next)
    }

    pub(crate) fn set_pen_smoothing(&mut self, level: u8) -> bool {
        let level = crate::draw::shape::clamp_pen_smoothing(level);
        if level == self.pen_smoothing {
            return false;
        }
        self.pen_smoothing = level;
        true
    }

    pub(crate) fn set_spotlight_magnification(&mut self, magnification: f64) -> bool {
        let normalized = ToolbarSliderSpec::SPOTLIGHT_MAGNIFICATION.normalize_value(
            crate::draw::normalize_spotlight_magnification(magnification),
        );
        if (normalized - self.spotlight_magnification).abs() < f64::EPSILON {
            return false;
        }
        self.spotlight_magnification = normalized;
        true
    }

    pub(crate) fn set_color(&mut self, tool: Tool, color: Color) -> bool {
        if self.color_for_tool(tool) == color {
            return false;
        }
        self.tool_settings.get_mut(tool).color = color;
        self.current_color = color;
        true
    }

    pub(crate) fn set_thickness(&mut self, tool: Tool, thickness: f64) -> bool {
        let clamped = thickness.clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
        if (clamped - self.tool_settings.get(tool).thickness).abs() < f64::EPSILON {
            return false;
        }
        self.tool_settings.get_mut(tool).thickness = clamped;
        self.current_thickness = clamped;
        true
    }

    pub(crate) fn set_eraser_size(&mut self, size: f64) -> bool {
        let clamped = size.clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
        if (clamped - self.eraser_size).abs() < f64::EPSILON {
            return false;
        }
        self.eraser_size = clamped;
        true
    }

    pub(crate) fn set_eraser_mode(&mut self, mode: EraserMode) -> bool {
        if self.eraser_mode == mode {
            return false;
        }
        self.eraser_mode = mode;
        true
    }

    pub(crate) fn toggle_eraser_mode(&mut self) -> bool {
        let next = match self.eraser_mode {
            EraserMode::Brush => EraserMode::Stroke,
            EraserMode::Stroke => EraserMode::Brush,
        };
        self.set_eraser_mode(next)
    }

    pub(crate) fn eraser_hit_radius(&self) -> f64 {
        (self.eraser_size / 2.0).max(1.0)
    }

    pub(crate) fn set_blur_style(&mut self, style: BlurStyle) -> bool {
        if self.blur_style == style {
            return false;
        }
        self.blur_style = style;
        true
    }

    pub(crate) fn cycle_blur_style(&mut self) -> bool {
        self.set_blur_style(self.blur_style.next())
    }

    pub(crate) fn set_arrow_style(&mut self, style: ArrowStyle) -> bool {
        if self.arrow_style == style {
            return false;
        }
        self.arrow_style = style;
        true
    }

    pub(crate) fn cycle_arrow_style(&mut self) -> bool {
        self.set_arrow_style(self.arrow_style.next())
    }

    pub(crate) fn set_font_descriptor(&mut self, descriptor: FontDescriptor) -> bool {
        if self.font_descriptor == descriptor {
            return false;
        }
        self.font_descriptor = descriptor;
        true
    }

    pub(crate) fn set_font_size(&mut self, size: f64) -> bool {
        let spec = ToolbarSliderSpec::FONT_SIZE;
        let clamped = size.clamp(spec.min, spec.max);
        if (clamped - self.current_font_size).abs() < f64::EPSILON {
            return false;
        }
        self.current_font_size = clamped;
        true
    }

    pub(crate) fn set_fill_enabled(&mut self, enabled: bool) -> bool {
        if self.fill_enabled == enabled {
            return false;
        }
        self.fill_enabled = enabled;
        true
    }

    pub(crate) fn set_polygon_sides(&mut self, sides: u8) -> bool {
        let clamped = clamp_regular_sides(sides);
        if self.polygon_sides == clamped {
            return false;
        }
        self.polygon_sides = clamped;
        true
    }

    pub(crate) fn nudge_polygon_sides(&mut self, delta: i8) -> bool {
        let next = if delta.is_negative() {
            self.polygon_sides.saturating_sub(delta.unsigned_abs())
        } else {
            self.polygon_sides.saturating_add(delta as u8)
        };
        self.set_polygon_sides(next)
    }

    pub(crate) fn apply_full_preset_tool_settings(
        &mut self,
        settings: &PresetToolStatesConfig,
    ) -> bool {
        let tool_settings = settings.to_runtime();
        let changed = self.tool_settings != tool_settings
            || (self.eraser_size - settings.eraser_size).abs() > f64::EPSILON;
        self.tool_settings = tool_settings;
        self.eraser_size = settings.eraser_size;
        changed
    }

    pub(crate) fn apply_preset_shape_settings(&mut self, preset: &ToolPresetConfig) -> bool {
        let mut changed = false;
        if let Some(length) = preset.arrow_length {
            let clamped = clamp_arrow_length(length);
            if (self.arrow_length - clamped).abs() > f64::EPSILON {
                self.arrow_length = clamped;
                changed = true;
            }
        }
        if let Some(angle) = preset.arrow_angle {
            let clamped = clamp_arrow_angle(angle);
            if (self.arrow_angle - clamped).abs() > f64::EPSILON {
                self.arrow_angle = clamped;
                changed = true;
            }
        }
        if let Some(head_at_end) = preset.arrow_head_at_end
            && self.arrow_head_at_end != head_at_end
        {
            self.arrow_head_at_end = head_at_end;
            changed = true;
        }
        if let Some(polygon_sides) = preset.polygon_sides {
            changed |= self.set_polygon_sides(polygon_sides);
        }
        changed
    }

    pub(crate) fn record_recent_color(&mut self, color: Color) {
        self.recent_colors.retain(|recent| *recent != color);
        self.recent_colors.insert(0, color);
        self.recent_colors.truncate(RECENT_COLORS_CAP);
    }

    pub(crate) fn restore_recent_colors(&mut self, colors: &[Color]) {
        self.recent_colors.clear();
        for color in colors {
            if self.recent_colors.contains(color) {
                continue;
            }
            self.recent_colors.push(*color);
            if self.recent_colors.len() == RECENT_COLORS_CAP {
                break;
            }
        }
    }

    pub(crate) fn restore_snapshot(&mut self, snapshot: &ToolStateSnapshot, active_tool: Tool) {
        let current_thickness = snapshot
            .current_thickness
            .clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
        let tool_settings = snapshot.tool_settings.clone().unwrap_or_else(|| {
            let mut settings =
                PerToolDrawingSettings::new(snapshot.current_color, current_thickness);
            settings.step_marker.thickness =
                super::utility::default_step_marker_size(snapshot.current_font_size);
            settings
        });
        self.replace_tool_settings(
            tool_settings.clamp_thicknesses(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS),
            active_tool,
        );
        let _ = self.set_eraser_size(snapshot.eraser_size);
        self.eraser_kind = snapshot.eraser_kind;
        let _ = self.set_eraser_mode(snapshot.eraser_mode);
        let _ = self.set_blur_style(snapshot.blur_style);
        self.restore_recent_colors(&snapshot.recent_colors);
        if let Some(level) = snapshot.pen_smoothing {
            let _ = self.set_pen_smoothing(level);
        }
        if let Some(opacity) = snapshot.marker_opacity {
            let _ = self.set_marker_opacity(opacity);
        }
        if let Some(magnification) = snapshot.spotlight_magnification {
            self.spotlight_magnification =
                crate::draw::normalize_spotlight_magnification(magnification);
        }
        if let Some(fill_enabled) = snapshot.fill_enabled {
            let _ = self.set_fill_enabled(fill_enabled);
        }
        if let Some(font_descriptor) = snapshot.font_descriptor.clone() {
            let _ = self.set_font_descriptor(font_descriptor);
        }
        let _ = self.set_font_size(snapshot.current_font_size);
        self.text_background_enabled = snapshot.text_background_enabled;
        self.arrow_length = clamp_arrow_length(snapshot.arrow_length);
        self.arrow_angle = clamp_arrow_angle(snapshot.arrow_angle);
        if let Some(head_at_end) = snapshot.arrow_head_at_end {
            self.arrow_head_at_end = head_at_end;
        }
        if let Some(style) = snapshot.arrow_style {
            let _ = self.set_arrow_style(style);
        }
        if let Some(label_enabled) = snapshot.arrow_label_enabled {
            self.arrow_label_enabled = label_enabled;
        }
        self.polygon_sides = clamp_regular_sides(snapshot.polygon_sides);
    }

    pub(crate) fn capture_preset(
        &self,
        selected_tool: Tool,
        show_status_bar: bool,
        drag_tools: MouseDragToolsConfig,
    ) -> ToolPresetConfig {
        ToolPresetConfig {
            name: None,
            tool: selected_tool,
            color: self.color_for_tool(selected_tool).into(),
            size: self.thickness_for_tool(selected_tool),
            tool_settings: Some(PresetToolStatesConfig::from_runtime(
                &self.tool_settings,
                self.eraser_size,
            )),
            eraser_kind: Some(self.eraser_kind),
            eraser_mode: Some(self.eraser_mode),
            marker_opacity: Some(self.marker_opacity),
            fill_enabled: Some(self.fill_enabled),
            font_size: Some(self.current_font_size),
            text_background_enabled: Some(self.text_background_enabled),
            arrow_length: Some(self.arrow_length),
            arrow_angle: Some(self.arrow_angle),
            arrow_head_at_end: Some(self.arrow_head_at_end),
            polygon_sides: Some(self.polygon_sides),
            show_status_bar: Some(show_status_bar),
            drag_tools: Some(drag_tools),
        }
    }
}

fn clamp_arrow_length(length: f64) -> f64 {
    length.clamp(ARROW_LENGTH_MIN, ARROW_LENGTH_MAX)
}

fn clamp_arrow_angle(angle: f64) -> f64 {
    angle.clamp(ARROW_ANGLE_MIN, ARROW_ANGLE_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_style() -> DrawingStyle {
        DrawingStyle::from((
            &DrawingConfig::default(),
            &ArrowConfig::default(),
            &SpotlightConfig::default(),
        ))
    }

    #[test]
    fn drawing_style_owns_per_tool_color_and_thickness_updates() {
        let mut style = default_style();
        let color = Color::new(0.2, 0.4, 0.6, 0.8);

        assert!(style.set_color(Tool::Marker, color));
        assert!(!style.set_color(Tool::Marker, color));
        assert_eq!(style.color_for_tool(Tool::Marker), color);
        assert_eq!(style.current_color, color);

        assert!(style.set_thickness(Tool::Marker, MAX_STROKE_THICKNESS + 20.0));
        assert_eq!(style.thickness_for_tool(Tool::Marker), MAX_STROKE_THICKNESS);
        assert_eq!(style.current_thickness, MAX_STROKE_THICKNESS);
    }

    #[test]
    fn drawing_style_normalizes_bounded_values_and_reports_noops() {
        let mut style = default_style();

        assert!(style.set_marker_opacity(f64::INFINITY));
        assert_eq!(style.marker_opacity, ToolbarSliderSpec::MARKER_OPACITY.max);
        assert!(!style.set_marker_opacity(f64::INFINITY));

        assert!(style.set_polygon_sides(u8::MAX));
        assert_eq!(style.polygon_sides, crate::draw::REGULAR_POLYGON_MAX_SIDES);
        assert!(!style.set_polygon_sides(u8::MAX));
    }

    #[test]
    fn drawing_style_owns_recent_color_order_deduplication_and_capacity() {
        let mut style = default_style();
        let colors = (0..=RECENT_COLORS_CAP)
            .map(|index| Color::new(index as f64 / 10.0, 0.0, 0.0, 1.0))
            .collect::<Vec<_>>();

        for color in &colors {
            style.record_recent_color(*color);
        }
        style.record_recent_color(colors[2]);

        assert_eq!(style.recent_colors.len(), RECENT_COLORS_CAP);
        assert_eq!(style.recent_colors[0], colors[2]);
        assert_eq!(
            style
                .recent_colors
                .iter()
                .filter(|color| **color == colors[2])
                .count(),
            1
        );
    }

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
