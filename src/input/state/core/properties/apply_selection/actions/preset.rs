//! Tool presets and selected shapes: a preset's style applied to what is
//! selected, and a selected shape's style saved as a preset.

use super::color::{RecolorOpacity, recolored, set_opacity_to, shape_color_mut};
use super::stroke::set_thickness;
use crate::config::{ColorSpec, ToolPresetConfig};
use crate::draw::{Color, PolygonKind, Shape, TextMeasurer};
use crate::input::Tool;
use crate::input::state::core::base::{InputState, MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};
use crate::input::state::core::properties::apply_selection::constants::{
    MAX_ARROW_ANGLE, MAX_ARROW_LENGTH, MAX_FONT_SIZE, MIN_ARROW_ANGLE, MIN_ARROW_LENGTH,
    MIN_FONT_SIZE,
};

/// Applies the parts of `preset` that `shape` has: its color (a marker
/// keeping its own translucency unless the preset sets one), its size as a
/// stroke width, fill, font size, text background, and arrow head.
fn apply_preset_style(shape: &mut Shape, preset: &ToolPresetConfig) -> bool {
    let mut changed = false;

    if let Some((current, next)) = recolored(shape, preset.preview_color(), RecolorOpacity::Exact)
        && current != next
        && let Some(color) = shape_color_mut(shape)
    {
        *color = next;
        changed = true;
    }
    if !preset.tool.uses_eraser_size() {
        let size = preset
            .preview_size()
            .clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
        changed |= set_thickness(shape, size).unwrap_or(false);
    }
    if let (Shape::MarkerStroke { .. }, Some(opacity)) = (&*shape, preset.marker_opacity) {
        changed |= set_opacity_to(shape, opacity).unwrap_or(false);
    }

    match shape {
        Shape::Rect {
            fill, fill_color, ..
        }
        | Shape::Ellipse {
            fill, fill_color, ..
        }
        | Shape::Polygon {
            fill, fill_color, ..
        } => {
            // A preset with a fill setting carries the whole fill: its own
            // color, or none, which fills with the border color the preset
            // just set.
            if let Some(enabled) = preset.fill_enabled {
                changed |= replace(fill, enabled);
                let preset_fill = preset.fill_color.as_ref().map(ColorSpec::to_color);
                changed |= replace(fill_color, preset_fill);
            }
        }
        Shape::Text {
            size,
            background_enabled,
            ..
        } => {
            if let Some(font_size) = preset.font_size {
                changed |= replace(size, font_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE));
            }
            if let Some(enabled) = preset.text_background_enabled {
                changed |= replace(background_enabled, enabled);
            }
        }
        Shape::Arrow {
            arrow_length,
            arrow_angle,
            head_at_end,
            ..
        } => {
            if let Some(length) = preset.arrow_length {
                changed |= replace(
                    arrow_length,
                    length.clamp(MIN_ARROW_LENGTH, MAX_ARROW_LENGTH),
                );
            }
            if let Some(angle) = preset.arrow_angle {
                changed |= replace(arrow_angle, angle.clamp(MIN_ARROW_ANGLE, MAX_ARROW_ANGLE));
            }
            if let Some(at_end) = preset.arrow_head_at_end {
                changed |= replace(head_at_end, at_end);
            }
        }
        _ => {}
    }
    changed
}

fn replace<T: PartialEq>(slot: &mut T, value: T) -> bool {
    let changed = *slot != value;
    *slot = value;
    changed
}

/// The preset a tool would need to draw `shape` as it is, or `None` for a
/// shape no tool draws with a color and a size.
fn preset_from_shape(shape: &Shape) -> Option<ToolPresetConfig> {
    let mut preset = ToolPresetConfig {
        name: None,
        tool: Tool::Pen,
        color: ColorSpec::from(Color::new(0.0, 0.0, 0.0, 1.0)),
        size: 1.0,
        tool_settings: None,
        eraser_kind: None,
        eraser_mode: None,
        marker_opacity: None,
        fill_enabled: None,
        fill_color: None,
        font_size: None,
        text_background_enabled: None,
        arrow_length: None,
        arrow_angle: None,
        arrow_head_at_end: None,
        polygon_sides: None,
        show_status_bar: None,
        drag_tools: None,
    };
    let (tool, color, size) = match shape {
        Shape::Freehand { color, thick, .. } => (Tool::Pen, *color, *thick),
        Shape::Line { color, thick, .. } => (Tool::Line, *color, *thick),
        Shape::Rect {
            color,
            thick,
            fill,
            fill_color,
            ..
        } => {
            preset.fill_enabled = Some(*fill);
            preset.fill_color = fill_color.map(ColorSpec::from);
            (Tool::Rect, *color, *thick)
        }
        Shape::Ellipse {
            color,
            thick,
            fill,
            fill_color,
            ..
        } => {
            preset.fill_enabled = Some(*fill);
            preset.fill_color = fill_color.map(ColorSpec::from);
            (Tool::Ellipse, *color, *thick)
        }
        Shape::Polygon {
            kind,
            color,
            thick,
            fill,
            fill_color,
            ..
        } => {
            preset.fill_enabled = Some(*fill);
            preset.fill_color = fill_color.map(ColorSpec::from);
            let tool = match kind {
                PolygonKind::Triangle => Tool::Triangle,
                PolygonKind::Parallelogram => Tool::Parallelogram,
                PolygonKind::Rhombus => Tool::Rhombus,
                PolygonKind::Regular { sides } => {
                    preset.polygon_sides = Some(*sides);
                    Tool::RegularPolygon
                }
                PolygonKind::Freeform => Tool::FreeformPolygon,
            };
            (tool, *color, *thick)
        }
        Shape::Arrow {
            color,
            thick,
            arrow_length,
            arrow_angle,
            head_at_end,
            ..
        } => {
            preset.arrow_length = Some(*arrow_length);
            preset.arrow_angle = Some(*arrow_angle);
            preset.arrow_head_at_end = Some(*head_at_end);
            (Tool::Arrow, *color, *thick)
        }
        Shape::MarkerStroke { color, thick, .. } => {
            preset.marker_opacity = Some(color.a);
            (Tool::Marker, Color { a: 1.0, ..*color }, *thick)
        }
        _ => return None,
    };
    preset.tool = tool;
    preset.color = ColorSpec::from(color);
    preset.size = size;
    Some(preset)
}

impl InputState {
    /// Styles every editable selected shape after preset `slot`, as one undo
    /// entry. The tool in use stays as it is: this restyles shapes, it does
    /// not pick up the preset's tool.
    pub(in crate::input::state::core::properties) fn apply_preset_to_selection_with(
        &mut self,
        measurer: &TextMeasurer,
        slot: usize,
    ) -> bool {
        let Some(preset) = self.preset_slots.preset(slot) else {
            return false;
        };

        self.finish_active_arrow_bend();
        let result = self.apply_selection_change_with(measurer, shape_supports_presets, |shape| {
            apply_preset_style(shape, &preset)
        });

        self.report_selection_apply_result(result, "preset")
    }

    /// The preset for the first editable selected shape a tool can draw.
    pub(in crate::input::state::core::properties) fn selection_preset_source(
        &self,
    ) -> Option<ToolPresetConfig> {
        let frame = self.boards.active_frame();
        self.selected_shape_ids()
            .iter()
            .filter_map(|id| frame.shape(*id))
            .filter(|drawn| !drawn.locked)
            .find_map(|drawn| preset_from_shape(&drawn.shape))
    }
}

fn shape_supports_presets(shape: &Shape) -> bool {
    !matches!(
        shape,
        Shape::Image { .. } | Shape::EraserStroke { .. } | Shape::Spotlight { .. }
    )
}
