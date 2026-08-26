use crate::draw::shape::{
    bounding_box_for_blur, bounding_box_for_eraser, bounding_box_for_points, smooth_path,
    smooth_pressure_path,
};
use crate::draw::{
    ArrowLabel, ArrowStyle, BlurRectParams, BlurStyle, Color, EraserBrush, EraserKind, Shape,
};
use crate::input::tool::{
    EraserMode, Tool, ToolDrawingBehavior, ToolPathKind, ToolPressureBehavior,
};
use crate::util::{self, Rect};

pub(crate) const PROVISIONAL_POLYGON_DAMAGE_PADDING: i32 = 2;

/// Bend a freshly drawn arrow starts with.
///
/// Only `Curved` gets one: a curved arrow created dead straight would look
/// exactly like a standard one, so choosing the style would appear to do
/// nothing until the bend handle was found. Every other style ignores the
/// field, and storing zero there keeps a later switch to `Curved` from
/// inheriting a bend the user never asked for.
fn initial_arrow_bend(style: ArrowStyle) -> f64 {
    if style.is_curved() {
        util::DEFAULT_ARROW_BEND
    } else {
        0.0
    }
}

mod polygon;

/// Immutable inputs needed to turn one completed drag into an app-level outcome.
pub(crate) struct ToolStrokeSnapshot {
    pub(crate) tool: Tool,
    pub(crate) start: (i32, i32),
    pub(crate) end: (i32, i32),
    pub(crate) points: Vec<(i32, i32)>,
    pub(crate) point_thicknesses: Vec<f32>,
    pub(crate) color: Color,
    pub(crate) size: f64,
    pub(crate) marker_opacity: f64,
    pub(crate) fill_enabled: bool,
    pub(crate) blur_style: BlurStyle,
    pub(crate) spotlight_magnification: f64,
    pub(crate) arrow_length: f64,
    pub(crate) arrow_angle: f64,
    pub(crate) arrow_head_at_end: bool,
    pub(crate) arrow_style: ArrowStyle,
    pub(crate) arrow_label: Option<ArrowLabel>,
    pub(crate) step_marker_label: crate::draw::StepMarkerLabel,
    pub(crate) eraser_mode: EraserMode,
    pub(crate) eraser_size: f64,
    pub(crate) eraser_kind: EraserKind,
    pub(crate) pressure_variation_threshold: f64,
    /// Release-time smoothing passes for path tools. 0 keeps the exact path.
    pub(crate) pen_smoothing: u8,
}

/// Immutable inputs needed to turn one completed polygon drag into a shape.
pub(crate) struct PolygonStrokeSnapshot {
    pub(crate) tool: Tool,
    pub(crate) start: (i32, i32),
    pub(crate) end: (i32, i32),
    pub(crate) color: Color,
    pub(crate) size: f64,
    pub(crate) fill_enabled: bool,
    pub(crate) regular_sides: u8,
}

pub(crate) enum FinishedToolStroke {
    Shape { shape: Shape, usage: ToolUsage },
    EraseStroke { path: Vec<(i32, i32)> },
    Noop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ToolUsage {
    pub(crate) bump_arrow_label: bool,
    pub(crate) bump_step_marker: bool,
}

/// Borrowed inputs needed to classify and render the current live preview.
pub(crate) struct ProvisionalToolSnapshot<'a> {
    pub(crate) tool: Tool,
    pub(crate) start: (i32, i32),
    pub(crate) current: (i32, i32),
    pub(crate) points: &'a [(i32, i32)],
    pub(crate) point_thicknesses: &'a [f32],
    pub(crate) color: Color,
    pub(crate) size: f64,
    pub(crate) eraser_size: f64,
    pub(crate) marker_opacity: f64,
    pub(crate) fill_enabled: bool,
    pub(crate) blur_style: BlurStyle,
    pub(crate) spotlight_magnification: f64,
    pub(crate) arrow_length: f64,
    pub(crate) arrow_angle: f64,
    pub(crate) arrow_head_at_end: bool,
    pub(crate) arrow_style: ArrowStyle,
    pub(crate) arrow_label: Option<ArrowLabel>,
    pub(crate) step_marker_label: Option<crate::draw::StepMarkerLabel>,
}

/// Borrowed inputs needed to render the current live polygon preview.
pub(crate) struct PolygonProvisionalSnapshot {
    pub(crate) tool: Tool,
    pub(crate) start: (i32, i32),
    pub(crate) current: (i32, i32),
    pub(crate) color: Color,
    pub(crate) size: f64,
    pub(crate) fill_enabled: bool,
    pub(crate) regular_sides: u8,
}

pub(crate) enum ProvisionalToolStroke<'a> {
    BorrowedFreehand {
        points: &'a [(i32, i32)],
        color: Color,
        size: f64,
    },
    BorrowedPressureFreehand {
        points: &'a [(i32, i32)],
        point_thicknesses: &'a [f32],
        color: Color,
    },
    BorrowedMarker {
        points: &'a [(i32, i32)],
        color: Color,
        size: f64,
    },
    EraserPreview {
        points: &'a [(i32, i32)],
        size: f64,
    },
    Shape(Shape),
    BlurReplayPreview(BlurRectParams),
    None,
}

impl Tool {
    pub(crate) fn finish_stroke(self, snapshot: ToolStrokeSnapshot) -> FinishedToolStroke {
        debug_assert_eq!(self, snapshot.tool);
        let usage = ToolUsage::default();
        match self.drawing_behavior() {
            ToolDrawingBehavior::None => FinishedToolStroke::Noop,
            ToolDrawingBehavior::Path { kind, pressure } => {
                finish_path_stroke(snapshot, kind, pressure, usage)
            }
            ToolDrawingBehavior::Line => finish_shape(snapshot, usage, |snapshot| Shape::Line {
                x1: snapshot.start.0,
                y1: snapshot.start.1,
                x2: snapshot.end.0,
                y2: snapshot.end.1,
                color: snapshot.color,
                thick: snapshot.size,
            }),
            ToolDrawingBehavior::Rect => {
                let Some((x, y, w, h)) = normalized_drag_bounds(snapshot.start, snapshot.end)
                else {
                    return FinishedToolStroke::Noop;
                };
                finish_shape(snapshot, usage, |snapshot| Shape::Rect {
                    x,
                    y,
                    w,
                    h,
                    fill: snapshot.fill_enabled,
                    color: snapshot.color,
                    thick: snapshot.size,
                })
            }
            ToolDrawingBehavior::Ellipse => finish_shape(snapshot, usage, |snapshot| {
                let (cx, cy, rx, ry) = util::ellipse_bounds(
                    snapshot.start.0,
                    snapshot.start.1,
                    snapshot.end.0,
                    snapshot.end.1,
                );
                Shape::Ellipse {
                    cx,
                    cy,
                    rx,
                    ry,
                    fill: snapshot.fill_enabled,
                    color: snapshot.color,
                    thick: snapshot.size,
                }
            }),
            ToolDrawingBehavior::Polygon(_) => {
                debug_assert!(false, "polygon strokes require PolygonStrokeSnapshot");
                FinishedToolStroke::Noop
            }
            ToolDrawingBehavior::Arrow => {
                let usage = ToolUsage {
                    bump_arrow_label: snapshot.arrow_label.is_some(),
                    ..usage
                };
                finish_shape(snapshot, usage, |snapshot| Shape::Arrow {
                    x1: snapshot.start.0,
                    y1: snapshot.start.1,
                    x2: snapshot.end.0,
                    y2: snapshot.end.1,
                    color: snapshot.color,
                    thick: snapshot.size,
                    arrow_length: snapshot.arrow_length,
                    arrow_angle: snapshot.arrow_angle,
                    head_at_end: snapshot.arrow_head_at_end,
                    style: snapshot.arrow_style,
                    bend: initial_arrow_bend(snapshot.arrow_style),
                    label: snapshot.arrow_label,
                })
            }
            ToolDrawingBehavior::BlurRect => {
                let Some((x, y, w, h)) = normalized_drag_bounds(snapshot.start, snapshot.end)
                else {
                    return FinishedToolStroke::Noop;
                };
                finish_shape(snapshot, usage, |snapshot| Shape::BlurRect {
                    x,
                    y,
                    w,
                    h,
                    strength: snapshot.size,
                    style: snapshot.blur_style,
                })
            }
            ToolDrawingBehavior::Spotlight => finish_shape(snapshot, usage, |snapshot| {
                let (cx, cy, rx, ry) = util::ellipse_bounds(
                    snapshot.start.0,
                    snapshot.start.1,
                    snapshot.end.0,
                    snapshot.end.1,
                );
                Shape::Spotlight {
                    cx,
                    cy,
                    rx,
                    ry,
                    magnification: crate::draw::normalize_spotlight_magnification(
                        snapshot.spotlight_magnification,
                    ),
                }
            }),
            ToolDrawingBehavior::StepMarker => {
                let usage = ToolUsage {
                    bump_step_marker: true,
                    ..usage
                };
                finish_shape(snapshot, usage, |snapshot| Shape::StepMarker {
                    x: snapshot.end.0,
                    y: snapshot.end.1,
                    color: snapshot.color,
                    label: snapshot.step_marker_label,
                })
            }
            ToolDrawingBehavior::Eraser => finish_eraser(snapshot),
        }
    }

    pub(crate) fn provisional_stroke<'a>(
        self,
        snapshot: ProvisionalToolSnapshot<'a>,
    ) -> ProvisionalToolStroke<'a> {
        debug_assert_eq!(self, snapshot.tool);
        match self.drawing_behavior() {
            ToolDrawingBehavior::None => ProvisionalToolStroke::None,
            ToolDrawingBehavior::Path {
                kind: ToolPathKind::Freehand,
                pressure: ToolPressureBehavior::OptionalPressureStroke,
            } => {
                if !snapshot.point_thicknesses.is_empty()
                    && snapshot.point_thicknesses.len() == snapshot.points.len()
                {
                    ProvisionalToolStroke::BorrowedPressureFreehand {
                        points: snapshot.points,
                        point_thicknesses: snapshot.point_thicknesses,
                        color: snapshot.color,
                    }
                } else {
                    ProvisionalToolStroke::BorrowedFreehand {
                        points: snapshot.points,
                        color: snapshot.color,
                        size: snapshot.size,
                    }
                }
            }
            ToolDrawingBehavior::Path {
                kind: ToolPathKind::Freehand,
                pressure: ToolPressureBehavior::None,
            } => ProvisionalToolStroke::BorrowedFreehand {
                points: snapshot.points,
                color: snapshot.color,
                size: snapshot.size,
            },
            ToolDrawingBehavior::Path {
                kind: ToolPathKind::Marker,
                ..
            } => ProvisionalToolStroke::BorrowedMarker {
                points: snapshot.points,
                color: marker_color_with_opacity(snapshot.color, snapshot.marker_opacity),
                size: snapshot.size,
            },
            ToolDrawingBehavior::Line => ProvisionalToolStroke::Shape(Shape::Line {
                x1: snapshot.start.0,
                y1: snapshot.start.1,
                x2: snapshot.current.0,
                y2: snapshot.current.1,
                color: snapshot.color,
                thick: snapshot.size,
            }),
            ToolDrawingBehavior::Rect => {
                let Some((x, y, w, h)) = normalized_drag_bounds(snapshot.start, snapshot.current)
                else {
                    return ProvisionalToolStroke::None;
                };
                ProvisionalToolStroke::Shape(Shape::Rect {
                    x,
                    y,
                    w,
                    h,
                    fill: snapshot.fill_enabled,
                    color: snapshot.color,
                    thick: snapshot.size,
                })
            }
            ToolDrawingBehavior::Ellipse => {
                let (cx, cy, rx, ry) = util::ellipse_bounds(
                    snapshot.start.0,
                    snapshot.start.1,
                    snapshot.current.0,
                    snapshot.current.1,
                );
                ProvisionalToolStroke::Shape(Shape::Ellipse {
                    cx,
                    cy,
                    rx,
                    ry,
                    fill: snapshot.fill_enabled,
                    color: snapshot.color,
                    thick: snapshot.size,
                })
            }
            ToolDrawingBehavior::Polygon(_) => {
                debug_assert!(false, "polygon previews require PolygonProvisionalSnapshot");
                ProvisionalToolStroke::None
            }
            ToolDrawingBehavior::Arrow => ProvisionalToolStroke::Shape(Shape::Arrow {
                x1: snapshot.start.0,
                y1: snapshot.start.1,
                x2: snapshot.current.0,
                y2: snapshot.current.1,
                color: snapshot.color,
                thick: snapshot.size,
                arrow_length: snapshot.arrow_length,
                arrow_angle: snapshot.arrow_angle,
                head_at_end: snapshot.arrow_head_at_end,
                style: snapshot.arrow_style,
                bend: initial_arrow_bend(snapshot.arrow_style),
                label: snapshot.arrow_label,
            }),
            ToolDrawingBehavior::BlurRect => {
                let Some((x, y, w, h)) = normalized_drag_bounds(snapshot.start, snapshot.current)
                else {
                    return ProvisionalToolStroke::None;
                };
                ProvisionalToolStroke::BlurReplayPreview(BlurRectParams {
                    x,
                    y,
                    w,
                    h,
                    strength: snapshot.size,
                    style: snapshot.blur_style,
                    cacheable: false,
                })
            }
            ToolDrawingBehavior::Spotlight => {
                let (cx, cy, rx, ry) = util::ellipse_bounds(
                    snapshot.start.0,
                    snapshot.start.1,
                    snapshot.current.0,
                    snapshot.current.1,
                );
                ProvisionalToolStroke::Shape(Shape::Spotlight {
                    cx,
                    cy,
                    rx,
                    ry,
                    magnification: crate::draw::normalize_spotlight_magnification(
                        snapshot.spotlight_magnification,
                    ),
                })
            }
            ToolDrawingBehavior::StepMarker => ProvisionalToolStroke::Shape(Shape::StepMarker {
                x: snapshot.current.0,
                y: snapshot.current.1,
                color: snapshot.color,
                label: match snapshot.step_marker_label {
                    Some(label) => label,
                    None => return ProvisionalToolStroke::None,
                },
            }),
            ToolDrawingBehavior::Eraser => ProvisionalToolStroke::EraserPreview {
                points: snapshot.points,
                size: snapshot.eraser_size,
            },
        }
    }
}

impl<'a> ProvisionalToolStroke<'a> {
    pub(crate) fn bounds(&self) -> Option<Rect> {
        match self {
            Self::BorrowedFreehand { points, size, .. } => bounding_box_for_points(points, *size),
            Self::BorrowedPressureFreehand {
                points,
                point_thicknesses,
                ..
            } => {
                let max_thick = point_thicknesses.iter().fold(0.0f32, |a, &b| a.max(b)) as f64;
                bounding_box_for_points(points, max_thick)
            }
            Self::BorrowedMarker { points, size, .. } => {
                let inflated = (*size * 1.35).max(*size + 1.0);
                bounding_box_for_points(points, inflated)
            }
            Self::EraserPreview { points, size } => bounding_box_for_eraser(points, *size),
            Self::Shape(shape) => {
                let bounds = shape.bounding_box();
                if matches!(shape, Shape::Polygon { .. }) {
                    bounds.and_then(|rect| rect.inflated(PROVISIONAL_POLYGON_DAMAGE_PADDING))
                } else {
                    bounds
                }
            }
            Self::BlurReplayPreview(params) => {
                bounding_box_for_blur(params.x, params.y, params.w, params.h)
            }
            Self::None => None,
        }
    }
}

pub(crate) fn marker_color_with_opacity(color: Color, marker_opacity: f64) -> Color {
    let alpha = (color.a * marker_opacity).clamp(0.05, 0.9);
    Color { a: alpha, ..color }
}

fn finish_path_stroke(
    snapshot: ToolStrokeSnapshot,
    kind: ToolPathKind,
    pressure: ToolPressureBehavior,
    usage: ToolUsage,
) -> FinishedToolStroke {
    match kind {
        ToolPathKind::Freehand => {
            if matches!(pressure, ToolPressureBehavior::OptionalPressureStroke)
                && pressure_data_varies(
                    &snapshot.point_thicknesses,
                    snapshot.points.len(),
                    snapshot.pressure_variation_threshold,
                )
            {
                let points: Vec<_> = snapshot
                    .points
                    .into_iter()
                    .zip(snapshot.point_thicknesses)
                    .map(|((x, y), t)| (x, y, t))
                    .collect();
                return FinishedToolStroke::Shape {
                    shape: Shape::FreehandPressure {
                        points: smooth_pressure_path(&points, snapshot.pen_smoothing),
                        color: snapshot.color,
                    },
                    usage,
                };
            }

            FinishedToolStroke::Shape {
                shape: Shape::Freehand {
                    points: smooth_path(&snapshot.points, snapshot.pen_smoothing),
                    color: snapshot.color,
                    thick: snapshot.size,
                },
                usage,
            }
        }
        ToolPathKind::Marker => FinishedToolStroke::Shape {
            shape: Shape::MarkerStroke {
                points: smooth_path(&snapshot.points, snapshot.pen_smoothing),
                color: marker_color_with_opacity(snapshot.color, snapshot.marker_opacity),
                thick: snapshot.size,
            },
            usage,
        },
    }
}

fn finish_eraser(snapshot: ToolStrokeSnapshot) -> FinishedToolStroke {
    if snapshot.eraser_mode == EraserMode::Stroke {
        let mut path = snapshot.points;
        if path.last().copied() != Some(snapshot.end) {
            path.push(snapshot.end);
        }
        return FinishedToolStroke::EraseStroke { path };
    }

    FinishedToolStroke::Shape {
        shape: Shape::EraserStroke {
            points: snapshot.points,
            brush: EraserBrush {
                size: snapshot.eraser_size,
                kind: snapshot.eraser_kind,
            },
        },
        usage: ToolUsage::default(),
    }
}

fn finish_shape(
    snapshot: ToolStrokeSnapshot,
    usage: ToolUsage,
    shape_builder: impl FnOnce(ToolStrokeSnapshot) -> Shape,
) -> FinishedToolStroke {
    FinishedToolStroke::Shape {
        shape: shape_builder(snapshot),
        usage,
    }
}

fn pressure_data_varies(point_thicknesses: &[f32], point_count: usize, threshold: f64) -> bool {
    if point_thicknesses.len() != point_count {
        return false;
    }
    let min_t = point_thicknesses
        .iter()
        .fold(f32::INFINITY, |a, &b| a.min(b));
    let max_t = point_thicknesses
        .iter()
        .fold(f32::NEG_INFINITY, |a, &b| a.max(b));
    (max_t - min_t).abs() > threshold as f32
}

fn normalized_axis(start: i32, end: i32) -> Option<(i32, i32)> {
    let length = i32::try_from(start.abs_diff(end)).ok()?;
    Some((start.min(end), length))
}

fn normalized_drag_bounds(start: (i32, i32), end: (i32, i32)) -> Option<(i32, i32, i32, i32)> {
    let (x, width) = normalized_axis(start.0, end.0)?;
    let (y, height) = normalized_axis(start.1, end.1)?;
    Some((x, y, width, height))
}

#[cfg(test)]
mod tests {
    use super::{normalized_axis, normalized_drag_bounds};

    #[test]
    fn normalized_axis_is_order_independent() {
        assert_eq!(normalized_axis(10, 25), Some((10, 15)));
        assert_eq!(normalized_axis(25, 10), Some((10, 15)));
    }

    #[test]
    fn normalized_axis_accepts_the_largest_representable_span() {
        assert_eq!(normalized_axis(i32::MIN, -1), Some((i32::MIN, i32::MAX)));
        assert_eq!(normalized_axis(-1, i32::MIN), Some((i32::MIN, i32::MAX)));
    }

    #[test]
    fn normalized_axis_rejects_unrepresentable_spans() {
        assert_eq!(normalized_axis(i32::MIN, i32::MAX), None);
        assert_eq!(normalized_axis(i32::MAX, i32::MIN), None);
    }

    #[test]
    fn normalized_drag_bounds_reject_either_unrepresentable_axis() {
        assert_eq!(normalized_drag_bounds((i32::MIN, 0), (i32::MAX, 10)), None);
        assert_eq!(normalized_drag_bounds((0, i32::MIN), (10, i32::MAX)), None);
    }
}
