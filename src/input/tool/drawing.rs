use crate::draw::shape::{
    PolygonTemplate, bounding_box_for_blur, bounding_box_for_eraser, bounding_box_for_points,
    generated_points, has_minimum_distinct_points,
};
use crate::draw::{ArrowLabel, BlurRectParams, Color, EraserBrush, EraserKind, Shape};
use crate::input::tool::{
    EraserMode, Tool, ToolDrawingBehavior, ToolPathKind, ToolPressureBehavior,
};
use crate::util::{self, Rect};

pub(crate) const PROVISIONAL_POLYGON_DAMAGE_PADDING: i32 = 2;

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
    pub(crate) arrow_length: f64,
    pub(crate) arrow_angle: f64,
    pub(crate) arrow_head_at_end: bool,
    pub(crate) arrow_label: Option<ArrowLabel>,
    pub(crate) step_marker_label: crate::draw::StepMarkerLabel,
    pub(crate) eraser_mode: EraserMode,
    pub(crate) eraser_size: f64,
    pub(crate) eraser_kind: EraserKind,
    pub(crate) pressure_variation_threshold: f64,
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
    pub(crate) arrow_length: f64,
    pub(crate) arrow_angle: f64,
    pub(crate) arrow_head_at_end: bool,
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
            ToolDrawingBehavior::Rect => finish_shape(snapshot, usage, |snapshot| {
                let (x, w) = normalized_axis(snapshot.start.0, snapshot.end.0);
                let (y, h) = normalized_axis(snapshot.start.1, snapshot.end.1);
                Shape::Rect {
                    x,
                    y,
                    w,
                    h,
                    fill: snapshot.fill_enabled,
                    color: snapshot.color,
                    thick: snapshot.size,
                }
            }),
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
                    label: snapshot.arrow_label,
                })
            }
            ToolDrawingBehavior::BlurRect => finish_shape(snapshot, usage, |snapshot| {
                let (x, w) = normalized_axis(snapshot.start.0, snapshot.end.0);
                let (y, h) = normalized_axis(snapshot.start.1, snapshot.end.1);
                Shape::BlurRect {
                    x,
                    y,
                    w,
                    h,
                    strength: snapshot.size,
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
                let (x, w) = normalized_axis(snapshot.start.0, snapshot.current.0);
                let (y, h) = normalized_axis(snapshot.start.1, snapshot.current.1);
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
                label: snapshot.arrow_label,
            }),
            ToolDrawingBehavior::BlurRect => {
                let (x, w) = normalized_axis(snapshot.start.0, snapshot.current.0);
                let (y, h) = normalized_axis(snapshot.start.1, snapshot.current.1);
                ProvisionalToolStroke::BlurReplayPreview(BlurRectParams {
                    x,
                    y,
                    w,
                    h,
                    strength: snapshot.size,
                    cacheable: false,
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

    pub(crate) fn polygon_template(self) -> Option<PolygonTemplate> {
        match self.drawing_behavior() {
            ToolDrawingBehavior::Polygon(template) => Some(template),
            _ => None,
        }
    }

    pub(crate) fn finish_polygon_stroke(
        self,
        snapshot: PolygonStrokeSnapshot,
    ) -> FinishedToolStroke {
        debug_assert_eq!(self, snapshot.tool);
        let Some(template) = self.polygon_template() else {
            debug_assert!(false, "non-polygon tool cannot finish a polygon stroke");
            return FinishedToolStroke::Noop;
        };
        finish_polygon(snapshot, ToolUsage::default(), template)
    }

    pub(crate) fn provisional_polygon_stroke(
        self,
        snapshot: PolygonProvisionalSnapshot,
    ) -> ProvisionalToolStroke<'static> {
        debug_assert_eq!(self, snapshot.tool);
        let Some(template) = self.polygon_template() else {
            debug_assert!(false, "non-polygon tool cannot preview a polygon stroke");
            return ProvisionalToolStroke::None;
        };
        provisional_polygon(snapshot, template)
    }
}

fn finish_polygon(
    snapshot: PolygonStrokeSnapshot,
    usage: ToolUsage,
    template: PolygonTemplate,
) -> FinishedToolStroke {
    let points = generated_points(
        template,
        snapshot.start,
        snapshot.end,
        snapshot.regular_sides,
    );
    if !has_minimum_distinct_points(&points) {
        return FinishedToolStroke::Noop;
    }

    FinishedToolStroke::Shape {
        shape: Shape::Polygon {
            kind: template.kind(snapshot.regular_sides),
            points,
            fill: snapshot.fill_enabled,
            color: snapshot.color,
            thick: snapshot.size,
        },
        usage,
    }
}

fn provisional_polygon(
    snapshot: PolygonProvisionalSnapshot,
    template: PolygonTemplate,
) -> ProvisionalToolStroke<'static> {
    let points = generated_points(
        template,
        snapshot.start,
        snapshot.current,
        snapshot.regular_sides,
    );
    ProvisionalToolStroke::Shape(Shape::Polygon {
        kind: template.kind(snapshot.regular_sides),
        points,
        fill: snapshot.fill_enabled,
        color: snapshot.color,
        thick: snapshot.size,
    })
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
                let points = snapshot
                    .points
                    .into_iter()
                    .zip(snapshot.point_thicknesses)
                    .map(|((x, y), t)| (x, y, t))
                    .collect();
                return FinishedToolStroke::Shape {
                    shape: Shape::FreehandPressure {
                        points,
                        color: snapshot.color,
                    },
                    usage,
                };
            }

            FinishedToolStroke::Shape {
                shape: Shape::Freehand {
                    points: snapshot.points,
                    color: snapshot.color,
                    thick: snapshot.size,
                },
                usage,
            }
        }
        ToolPathKind::Marker => FinishedToolStroke::Shape {
            shape: Shape::MarkerStroke {
                points: snapshot.points,
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

fn normalized_axis(start: i32, end: i32) -> (i32, i32) {
    if end >= start {
        (start, end - start)
    } else {
        (end, start - end)
    }
}
