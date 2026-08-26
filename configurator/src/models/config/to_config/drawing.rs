use super::super::draft::ConfigDraft;
use super::super::parse::{
    parse_field_in_range, parse_u8_in_range, parse_usize_at_least, parse_usize_in_range,
};
use crate::models::error::FormError;
use wayscriber::config::Config;
use wayscriber::draw::{MAX_PEN_SMOOTHING, REGULAR_POLYGON_MAX_SIDES, REGULAR_POLYGON_MIN_SIDES};
use wayscriber::input::state::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};
use wayscriber::input::{DragBindableTool, DragTool};

impl ConfigDraft {
    pub(super) fn apply_drawing(&self, config: &mut Config, errors: &mut Vec<FormError>) {
        let materialize_drag_tools = config.drawing.drag_tools.is_some()
            || config.drawing.effective_drag_tools() != self.drawing_drag_tools;
        match self.drawing_color.to_color_spec() {
            Ok(color) => config.drawing.default_color = color,
            Err(err) => errors.push(err),
        }
        self.drawing_quick_colors
            .apply_to_config(&mut config.drawing.quick_colors, errors);
        parse_field_in_range(
            &self.drawing_default_thickness,
            "drawing.default_thickness",
            MIN_STROKE_THICKNESS,
            MAX_STROKE_THICKNESS,
            errors,
            |value| config.drawing.default_thickness = value,
        );
        parse_field_in_range(
            &self.drawing_default_eraser_size,
            "drawing.default_eraser_size",
            MIN_STROKE_THICKNESS,
            MAX_STROKE_THICKNESS,
            errors,
            |value| config.drawing.default_eraser_size = value,
        );
        config.drawing.default_eraser_mode = self.drawing_default_eraser_mode.to_mode();
        parse_field_in_range(
            &self.drawing_default_font_size,
            "drawing.default_font_size",
            8.0,
            72.0,
            errors,
            |value| config.drawing.default_font_size = value,
        );
        parse_u8_in_range(
            &self.drawing_polygon_sides,
            "drawing.polygon_sides",
            REGULAR_POLYGON_MIN_SIDES,
            REGULAR_POLYGON_MAX_SIDES,
            errors,
            |value| config.drawing.polygon_sides = value,
        );
        parse_u8_in_range(
            &self.drawing_pen_smoothing,
            "drawing.pen_smoothing",
            0,
            MAX_PEN_SMOOTHING,
            errors,
            |value| config.drawing.pen_smoothing = value,
        );
        parse_field_in_range(
            &self.drawing_marker_opacity,
            "drawing.marker_opacity",
            0.05,
            0.9,
            errors,
            |value| config.drawing.marker_opacity = value,
        );
        // A list in, a list out. Nothing to parse and nothing a family name can
        // contain that would break the round trip.
        config.drawing.font_cycle = self.drawing_font_cycle.entries().to_vec();
        config.drawing.font_family = self.drawing_font_family.clone();
        config.drawing.font_weight = self.drawing_font_weight.clone();
        config.drawing.font_style = self.drawing_font_style.clone();
        config.drawing.text_background_enabled = self.drawing_text_background_enabled;
        config.drawing.text_halo_enabled = self.drawing_text_halo_enabled;
        config.drawing.default_fill_enabled = self.drawing_default_fill_enabled;
        config.drawing.drag_tool = legacy_tool(
            self.drawing_drag_tools.left.drag_tool,
            DragBindableTool::Pen,
        );
        config.drawing.shift_drag_tool = legacy_tool(
            self.drawing_drag_tools.left.shift_drag_tool,
            DragBindableTool::Line,
        );
        config.drawing.ctrl_drag_tool = legacy_tool(
            self.drawing_drag_tools.left.ctrl_drag_tool,
            DragBindableTool::Rect,
        );
        config.drawing.ctrl_shift_drag_tool = legacy_tool(
            self.drawing_drag_tools.left.ctrl_shift_drag_tool,
            DragBindableTool::Arrow,
        );
        config.drawing.tab_drag_tool = legacy_tool(
            self.drawing_drag_tools.left.tab_drag_tool,
            DragBindableTool::Ellipse,
        );
        config.drawing.drag_tools = materialize_drag_tools.then(|| self.drawing_drag_tools.clone());
        parse_field_in_range(
            &self.drawing_hit_test_tolerance,
            "drawing.hit_test_tolerance",
            1.0,
            20.0,
            errors,
            |value| config.drawing.hit_test_tolerance = value,
        );
        parse_usize_at_least(
            &self.drawing_hit_test_linear_threshold,
            "drawing.hit_test_linear_threshold",
            1,
            errors,
            |value| config.drawing.hit_test_linear_threshold = value,
        );
        parse_usize_in_range(
            &self.drawing_undo_stack_limit,
            "drawing.undo_stack_limit",
            10,
            1000,
            errors,
            |value| config.drawing.undo_stack_limit = value,
        );

        parse_field_in_range(
            &self.arrow_length,
            "arrow.length",
            5.0,
            50.0,
            errors,
            |value| config.arrow.length = value,
        );
        parse_field_in_range(
            &self.arrow_angle,
            "arrow.angle_degrees",
            15.0,
            60.0,
            errors,
            |value| config.arrow.angle_degrees = value,
        );
        config.arrow.head_at_end = self.arrow_head_at_end;
        config.arrow.style = self.arrow_style.to_style();
    }
}

fn legacy_tool(tool: DragTool, fallback: DragBindableTool) -> DragBindableTool {
    DragBindableTool::from_drag_tool(tool).unwrap_or(fallback)
}
