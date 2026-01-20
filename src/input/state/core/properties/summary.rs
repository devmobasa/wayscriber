use crate::draw::{Color, Frame, Shape, ShapeId};

#[derive(Debug)]
pub(super) struct PropertySummary<T> {
    pub(super) applicable: bool,
    pub(super) editable: bool,
    pub(super) mixed: bool,
    pub(super) value: Option<T>,
}

pub(super) fn summarize_property<T, F, Eq>(
    frame: &Frame,
    ids: &[ShapeId],
    mut extract: F,
    mut eq: Eq,
) -> PropertySummary<T>
where
    T: Clone,
    F: FnMut(&Shape) -> Option<T>,
    Eq: FnMut(&T, &T) -> bool,
{
    let mut values = Vec::new();
    let mut applicable = 0;
    for id in ids {
        let Some(drawn) = frame.shape(*id) else {
            continue;
        };
        let Some(value) = extract(&drawn.shape) else {
            continue;
        };
        applicable += 1;
        if drawn.locked {
            continue;
        }
        values.push(value);
    }

    if applicable == 0 {
        return PropertySummary {
            applicable: false,
            editable: false,
            mixed: false,
            value: None,
        };
    }

    if values.is_empty() {
        return PropertySummary {
            applicable: true,
            editable: false,
            mixed: false,
            value: None,
        };
    }

    let first = values[0].clone();
    let mixed = values.iter().skip(1).any(|value| !eq(&first, value));

    PropertySummary {
        applicable: true,
        editable: true,
        mixed,
        value: Some(first),
    }
}

pub(super) fn shape_color(shape: &Shape) -> Option<Color> {
    match shape {
        Shape::Freehand { color, .. }
        | Shape::FreehandPressure { color, .. }
        | Shape::Line { color, .. }
        | Shape::Rect { color, .. }
        | Shape::Ellipse { color, .. }
        | Shape::Arrow { color, .. }
        | Shape::Text { color, .. } => Some(*color),
        Shape::MarkerStroke { color, .. } => Some(Color { a: 1.0, ..*color }),
        Shape::StickyNote { background, .. } => Some(*background),
        _ => None,
    }
}

pub(super) fn shape_thickness(shape: &Shape) -> Option<f64> {
    match shape {
        Shape::Freehand { thick, .. }
        | Shape::Line { thick, .. }
        | Shape::Rect { thick, .. }
        | Shape::Ellipse { thick, .. }
        | Shape::Arrow { thick, .. }
        | Shape::MarkerStroke { thick, .. } => Some(*thick),
        _ => None,
    }
}

pub(super) fn shape_fill(shape: &Shape) -> Option<bool> {
    match shape {
        Shape::Rect { fill, .. } | Shape::Ellipse { fill, .. } => Some(*fill),
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
