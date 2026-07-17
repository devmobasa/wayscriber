use super::super::draft::ConfigDraft;
use super::super::parse::{parse_field, parse_u8_field, parse_usize_field};
use crate::models::error::FormError;
use wayscriber::config::Config;
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
        parse_field(
            &self.drawing_default_thickness,
            "drawing.default_thickness",
            errors,
            |value| config.drawing.default_thickness = value,
        );
        parse_field(
            &self.drawing_default_eraser_size,
            "drawing.default_eraser_size",
            errors,
            |value| config.drawing.default_eraser_size = value,
        );
        config.drawing.default_eraser_mode = self.drawing_default_eraser_mode.to_mode();
        parse_field(
            &self.drawing_default_font_size,
            "drawing.default_font_size",
            errors,
            |value| config.drawing.default_font_size = value,
        );
        parse_u8_field(
            &self.drawing_polygon_sides,
            "drawing.polygon_sides",
            errors,
            |value| config.drawing.polygon_sides = value,
        );
        parse_field(
            &self.drawing_marker_opacity,
            "drawing.marker_opacity",
            errors,
            |value| config.drawing.marker_opacity = value,
        );
        config.drawing.font_family = self.drawing_font_family.clone();
        config.drawing.font_weight = self.drawing_font_weight.clone();
        config.drawing.font_style = self.drawing_font_style.clone();
        config.drawing.text_background_enabled = self.drawing_text_background_enabled;
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
        parse_field(
            &self.drawing_hit_test_tolerance,
            "drawing.hit_test_tolerance",
            errors,
            |value| config.drawing.hit_test_tolerance = value,
        );
        parse_usize_field(
            &self.drawing_hit_test_linear_threshold,
            "drawing.hit_test_linear_threshold",
            errors,
            |value| config.drawing.hit_test_linear_threshold = value,
        );
        parse_usize_field(
            &self.drawing_undo_stack_limit,
            "drawing.undo_stack_limit",
            errors,
            |value| config.drawing.undo_stack_limit = value,
        );

        parse_field(&self.arrow_length, "arrow.length", errors, |value| {
            config.arrow.length = value
        });
        parse_field(&self.arrow_angle, "arrow.angle_degrees", errors, |value| {
            config.arrow.angle_degrees = value
        });
        config.arrow.head_at_end = self.arrow_head_at_end;
    }
}

fn legacy_tool(tool: DragTool, fallback: DragBindableTool) -> DragBindableTool {
    DragBindableTool::from_drag_tool(tool).unwrap_or(fallback)
}
