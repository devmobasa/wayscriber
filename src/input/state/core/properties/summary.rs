use std::collections::HashMap;

use crate::draw::{ArrowStyle, Color, DrawnShape, Frame, Shape, ShapeId};

#[derive(Debug)]
pub(super) struct PropertySummary<T> {
    pub(super) applicable: bool,
    pub(super) editable: bool,
    pub(super) mixed: bool,
    pub(super) value: Option<T>,
}

/// Resolve IDs with one frame pass while retaining selection order.
/// Missing IDs are omitted; callers can compare lengths when absence matters.
pub(super) fn resolve_selected_shapes<'a>(
    frame: &'a Frame,
    ids: &[ShapeId],
) -> Vec<&'a DrawnShape> {
    if ids.is_empty() {
        return Vec::new();
    }

    // Tiny selections need only a few comparisons per shape. Avoid hashing the
    // whole frame for the common one-shape selection; the bound stays constant.
    if ids.len() <= 4 {
        let mut selected = vec![None; ids.len()];
        let mut remaining = ids.len();
        for drawn in &frame.shapes {
            for (id, slot) in ids.iter().zip(&mut selected) {
                if *id == drawn.id {
                    if slot.is_none() {
                        remaining -= 1;
                    }
                    *slot = Some(drawn);
                }
            }
            if remaining == 0 {
                break;
            }
        }
        return selected.into_iter().flatten().collect();
    }

    let mut selected: HashMap<_, Option<&DrawnShape>> = ids.iter().map(|id| (*id, None)).collect();
    for drawn in &frame.shapes {
        if let Some(slot) = selected.get_mut(&drawn.id) {
            *slot = Some(drawn);
        }
    }

    ids.iter().filter_map(|id| selected[id]).collect()
}

pub(super) fn summarize_property<T, F, Eq>(
    selected: &[&DrawnShape],
    mut extract: F,
    mut eq: Eq,
) -> PropertySummary<T>
where
    F: FnMut(&Shape) -> Option<T>,
    Eq: FnMut(&T, &T) -> bool,
{
    let mut applicable = false;
    let mut first = None;
    let mut mixed = false;
    for drawn in selected {
        let Some(value) = extract(&drawn.shape) else {
            continue;
        };
        applicable = true;
        if drawn.locked {
            continue;
        }
        match &first {
            Some(first) => mixed |= !eq(first, &value),
            None => first = Some(value),
        }
    }

    PropertySummary {
        applicable,
        editable: first.is_some(),
        mixed,
        value: first,
    }
}

pub(super) fn shape_color(shape: &Shape) -> Option<Color> {
    match shape {
        Shape::Freehand { color, .. }
        | Shape::FreehandPressure { color, .. }
        | Shape::Line { color, .. }
        | Shape::Rect { color, .. }
        | Shape::Ellipse { color, .. }
        | Shape::Polygon { color, .. }
        | Shape::Arrow { color, .. }
        | Shape::Text { color, .. }
        | Shape::StepMarker { color, .. } => Some(*color),
        Shape::MarkerStroke { color, .. } => Some(Color { a: 1.0, ..*color }),
        Shape::StickyNote { background, .. } => Some(*background),
        _ => None,
    }
}

/// The opacity a shape is drawn at: its color's alpha, a marker's own
/// translucency included.
pub(super) fn shape_opacity(shape: &Shape) -> Option<f64> {
    match shape {
        Shape::MarkerStroke { color, .. } => Some(color.a),
        _ => shape_color(shape).map(|color| color.a),
    }
}

pub(super) fn shape_thickness(shape: &Shape) -> Option<f64> {
    match shape {
        Shape::Freehand { thick, .. }
        | Shape::Line { thick, .. }
        | Shape::Rect { thick, .. }
        | Shape::Ellipse { thick, .. }
        | Shape::Polygon { thick, .. }
        | Shape::Arrow { thick, .. }
        | Shape::BlurRect {
            strength: thick, ..
        }
        | Shape::MarkerStroke { thick, .. } => Some(*thick),
        _ => None,
    }
}

/// What a closed shape is filled with: `None` for no fill, or the fill's
/// own color, which falls back to the border's.
pub(super) fn shape_fill_paint(shape: &Shape) -> Option<Option<Color>> {
    match shape {
        Shape::Rect {
            fill,
            fill_color,
            color,
            ..
        }
        | Shape::Ellipse {
            fill,
            fill_color,
            color,
            ..
        }
        | Shape::Polygon {
            fill,
            fill_color,
            color,
            ..
        } => Some(fill.then(|| fill_color.unwrap_or(*color))),
        _ => None,
    }
}

pub(super) fn shape_font_size(shape: &Shape) -> Option<f64> {
    match shape {
        Shape::Text { size, .. } => Some(*size),
        _ => None,
    }
}

pub(super) fn shape_arrow_head(shape: &Shape) -> Option<bool> {
    match shape {
        Shape::Arrow { head_at_end, .. } => Some(*head_at_end),
        _ => None,
    }
}

pub(super) fn shape_arrow_style(shape: &Shape) -> Option<ArrowStyle> {
    match shape {
        Shape::Arrow { style, .. } => Some(*style),
        _ => None,
    }
}

pub(super) fn shape_arrow_length(shape: &Shape) -> Option<f64> {
    match shape {
        Shape::Arrow { arrow_length, .. } => Some(*arrow_length),
        _ => None,
    }
}

pub(super) fn shape_arrow_angle(shape: &Shape) -> Option<f64> {
    match shape {
        Shape::Arrow { arrow_angle, .. } => Some(*arrow_angle),
        _ => None,
    }
}

pub(super) fn shape_text_background(shape: &Shape) -> Option<bool> {
    match shape {
        Shape::Text {
            background_enabled, ..
        } => Some(*background_enabled),
        _ => None,
    }
}

pub(super) fn shape_spotlight_magnification(shape: &Shape) -> Option<f64> {
    match shape {
        Shape::Spotlight { magnification, .. } => Some(*magnification),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::FontDescriptor;
    use crate::input::state::core::properties::utils::color_eq;

    fn rect(color: Color, fill: bool, thick: f64) -> Shape {
        Shape::Rect {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
            fill,
            fill_color: None,
            color,
            thick,
        }
    }

    #[test]
    fn summarize_property_returns_not_applicable_when_no_shapes_support_it() {
        let mut frame = Frame::new();
        let text_id = frame.add_shape(Shape::Text {
            x: 10,
            y: 20,
            text: "hello".to_string(),
            color: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            size: 16.0,
            font_descriptor: FontDescriptor::default(),
            background_enabled: false,
            wrap_width: None,
        });

        let summary = summarize_property(
            &resolve_selected_shapes(&frame, &[text_id]),
            shape_fill_paint,
            |a, b| a == b,
        );

        assert!(!summary.applicable);
        assert!(!summary.editable);
        assert!(!summary.mixed);
        assert!(summary.value.is_none());
    }

    #[test]
    fn summarize_property_reports_locked_applicable_shapes_as_not_editable() {
        let mut frame = Frame::new();
        let id = frame.add_shape(rect(
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            false,
            2.0,
        ));
        frame.shape_mut(id).expect("locked shape").locked = true;

        let summary = summarize_property(
            &resolve_selected_shapes(&frame, &[id]),
            shape_color,
            color_eq,
        );

        assert!(summary.applicable);
        assert!(!summary.editable);
        assert!(!summary.mixed);
        assert!(summary.value.is_none());
    }

    #[test]
    fn summarize_property_reports_mixed_when_unlocked_values_differ() {
        let mut frame = Frame::new();
        let first = frame.add_shape(rect(
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            false,
            2.0,
        ));
        let second = frame.add_shape(rect(
            Color {
                r: 0.0,
                g: 0.0,
                b: 1.0,
                a: 1.0,
            },
            false,
            2.0,
        ));

        let summary = summarize_property(
            &resolve_selected_shapes(&frame, &[first, second]),
            shape_color,
            color_eq,
        );

        assert!(summary.applicable);
        assert!(summary.editable);
        assert!(summary.mixed);
        assert_eq!(
            summary.value,
            Some(Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            })
        );
    }

    #[test]
    fn summarize_property_ignores_locked_shapes_when_computing_value() {
        let mut frame = Frame::new();
        let unlocked = frame.add_shape(rect(
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            false,
            2.0,
        ));
        let locked = frame.add_shape(rect(
            Color {
                r: 0.0,
                g: 0.0,
                b: 1.0,
                a: 1.0,
            },
            false,
            2.0,
        ));
        frame.shape_mut(locked).expect("locked shape").locked = true;

        let summary = summarize_property(
            &resolve_selected_shapes(&frame, &[unlocked, locked]),
            shape_color,
            color_eq,
        );

        assert!(summary.applicable);
        assert!(summary.editable);
        assert!(!summary.mixed);
        assert_eq!(
            summary.value,
            Some(Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            })
        );
    }

    #[test]
    fn shape_color_uses_sticky_note_background_and_marker_opaque_alpha() {
        let sticky = Shape::StickyNote {
            x: 10,
            y: 20,
            text: "note".to_string(),
            background: Color {
                r: 0.2,
                g: 0.3,
                b: 0.4,
                a: 1.0,
            },
            size: 18.0,
            font_descriptor: FontDescriptor::default(),
            wrap_width: None,
        };
        let marker = Shape::MarkerStroke {
            points: vec![(0, 0), (5, 5)],
            color: Color {
                r: 1.0,
                g: 0.5,
                b: 0.0,
                a: 0.25,
            },
            thick: 6.0,
        };

        assert_eq!(
            shape_color(&sticky),
            Some(Color {
                r: 0.2,
                g: 0.3,
                b: 0.4,
                a: 1.0,
            })
        );
        assert_eq!(
            shape_color(&marker),
            Some(Color {
                r: 1.0,
                g: 0.5,
                b: 0.0,
                a: 1.0,
            })
        );
    }
}
