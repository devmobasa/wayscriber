use super::*;
use crate::draw::{
    ArrowLabel, ArrowStyle, Color, DrawnShape, FontDescriptor, Shape, StepMarkerLabel,
};

fn text_shapes() -> [Shape; 4] {
    let font_descriptor = FontDescriptor::default();
    let color = Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    [
        Shape::Text {
            x: 40,
            y: 60,
            text: "abc אבג 你好 wrapping words".into(),
            color,
            size: 18.0,
            font_descriptor: font_descriptor.clone(),
            background_enabled: true,
            wrap_width: Some(100),
        },
        Shape::StickyNote {
            x: 40,
            y: 60,
            text: "a note with wrapping words".into(),
            background: color,
            size: 18.0,
            font_descriptor: font_descriptor.clone(),
            wrap_width: Some(100),
        },
        Shape::Arrow {
            x1: 20,
            y1: 40,
            x2: 90,
            y2: 60,
            color,
            thick: 3.0,
            arrow_length: 12.0,
            arrow_angle: 30.0,
            head_at_end: true,
            style: ArrowStyle::Curved,
            bend: 0.3,
            label: Some(ArrowLabel {
                value: 8888,
                size: 24.0,
                font_descriptor: font_descriptor.clone(),
            }),
        },
        Shape::StepMarker {
            x: 50,
            y: 60,
            color,
            label: StepMarkerLabel {
                value: 8888,
                size: 24.0,
                font_descriptor,
            },
        },
    ]
}

#[test]
fn decorated_bounds_use_supplied_owner_across_memo_clone_and_serde() {
    for shape in text_shapes() {
        let owner = TextMeasurer::default();
        let independent = TextMeasurer::default();
        let drawn = DrawnShape::with_metadata(7, shape, 123, false);
        let cold_clone = drawn.clone();
        let encoded = serde_json::to_string(&drawn).unwrap();
        let restored: DrawnShape = serde_json::from_str(&encoded).unwrap();
        assert!(owner.context.borrow().is_none());
        assert!(owner.cache.borrow().entries.is_empty());
        let expected = drawn.bounding_box_with(&owner).expect("decorated bounds");
        assert!(owner.context.borrow().is_some());
        assert_eq!(
            owner.cache.borrow().entries.len(),
            1,
            "each shape measures its text with the supplied owner"
        );
        assert!(independent.context.borrow().is_none());
        assert_eq!(drawn.bounding_box_with(&owner), Some(expected));
        assert_eq!(cold_clone.bounding_box_with(&independent), Some(expected));
        assert_eq!(restored.bounding_box_with(&independent), Some(expected));
        assert_eq!(
            drawn.clone().bounding_box_with(&independent),
            Some(expected)
        );
        assert_eq!(drawn.bounding_box(), Some(expected));
        assert_eq!(drawn.shape.bounding_box(), Some(expected));
        assert_eq!(
            serde_json::to_string(&drawn).unwrap(),
            encoded,
            "measurement resources must not change persisted values"
        );
    }
}

#[test]
fn text_mutations_invalidate_memo_and_select_new_measurement_keys() {
    for shape in text_shapes().into_iter().take(2) {
        let owner = TextMeasurer::default();
        let mut drawn = DrawnShape::with_metadata(1, shape, 0, false);
        let before = drawn.bounding_box_with(&owner).unwrap();
        match &mut drawn.shape {
            Shape::Text { wrap_width, .. } | Shape::StickyNote { wrap_width, .. } => {
                *wrap_width = Some(45)
            }
            _ => unreachable!(),
        }
        drawn.invalidate_bounds();
        let wrapped = drawn.bounding_box_with(&owner).unwrap();
        assert!(wrapped.height > before.height);
        assert_eq!(owner.cache.borrow().entries.len(), 2);
        match &mut drawn.shape {
            Shape::Text {
                size,
                font_descriptor,
                ..
            }
            | Shape::StickyNote {
                size,
                font_descriptor,
                ..
            } => {
                *size = 32.0;
                font_descriptor.weight = "bold".into();
                font_descriptor.style = "italic".into();
            }
            _ => unreachable!(),
        }
        drawn.invalidate_bounds();
        let styled = drawn.bounding_box_with(&owner).unwrap();
        assert_ne!(styled, wrapped);
        assert_eq!(owner.cache.borrow().entries.len(), 3);
        assert_eq!(
            Some(styled),
            drawn.shape.bounding_box_with(&TextMeasurer::default())
        );
    }
}

#[test]
fn numeric_and_empty_bounds_do_not_initialize_text_resources() {
    let owner = TextMeasurer::default();
    let mut shapes = text_shapes();
    for shape in &mut shapes[..2] {
        match shape {
            Shape::Text { text, .. } | Shape::StickyNote { text, .. } => text.clear(),
            _ => unreachable!(),
        }
        let drawn = DrawnShape::with_metadata(1, shape.clone(), 0, false);
        assert_eq!(drawn.bounding_box_with(&owner), None);
        assert_eq!(drawn.bounding_box_with(&owner), None);
        assert_eq!(drawn.clone().bounding_box_with(&owner), None);
    }
    let numeric = Shape::BlurRect {
        x: 10,
        y: 20,
        w: 30,
        h: 40,
        strength: 5.0,
        style: Default::default(),
    };
    assert!(numeric.bounding_box_with(&owner).is_some());
    assert!(owner.context.borrow().is_none());
    assert!(owner.cache.borrow().entries.is_empty());
}

#[test]
fn dirty_and_provisional_bounds_consume_the_supplied_owner() {
    use crate::draw::DirtyTracker;
    use crate::input::tool::ProvisionalToolStroke;

    let shape = text_shapes().into_iter().next().unwrap();
    let expected = shape.bounding_box().unwrap();
    let dirty_owner = TextMeasurer::default();
    let mut dirty = DirtyTracker::default();
    dirty.mark_shape_with(&shape, &dirty_owner);
    assert_eq!(dirty_owner.cache.borrow().entries.len(), 1);
    assert_eq!(dirty.take_regions(800, 600), vec![expected]);

    let preview_owner = TextMeasurer::default();
    let preview = ProvisionalToolStroke::Shape(shape);
    assert_eq!(preview.bounds_with(&preview_owner), Some(expected));
    assert_eq!(preview_owner.cache.borrow().entries.len(), 1);
    assert_eq!(preview.bounds_with(&preview_owner), Some(expected));

    dirty.mark_shape_with(
        &Shape::Freehand {
            points: vec![],
            color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thick: 2.0,
        },
        &dirty_owner,
    );
    let full = dirty.take_regions(800, 600);
    assert_eq!(full.len(), 1);
    assert_eq!(
        (full[0].x, full[0].y, full[0].width, full[0].height),
        (0, 0, 800, 600)
    );
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "stale DrawnShape bounds cache")]
fn known_empty_bounds_still_validate_after_untracked_text_mutation() {
    let owner = TextMeasurer::default();
    let mut shape = text_shapes().into_iter().next().unwrap();
    if let Shape::Text { text, .. } = &mut shape {
        text.clear();
    }
    let mut drawn = DrawnShape::with_metadata(1, shape, 0, false);
    assert_eq!(drawn.bounding_box_with(&owner), None);
    if let Shape::Text { text, .. } = &mut drawn.shape {
        *text = "new drawable text".into();
    }
    drawn.bounding_box_with(&owner);
}

#[test]
fn changed_number_labels_refresh_cached_shape_bounds() {
    for mut shape in text_shapes().into_iter().skip(2) {
        let owner = TextMeasurer::default();
        let mut drawn = DrawnShape::with_metadata(1, shape.clone(), 0, false);
        let before = drawn.bounding_box_with(&owner).unwrap();
        match &mut shape {
            Shape::Arrow {
                label: Some(label), ..
            } => {
                label.value = 888888;
                label.size = 48.0;
                label.font_descriptor.weight = "bold".into();
            }
            Shape::StepMarker { label, .. } => {
                label.value = 888888;
                label.size = 48.0;
                label.font_descriptor.weight = "bold".into();
            }
            _ => unreachable!(),
        }
        drawn.set_shape(shape);
        let after = drawn.bounding_box_with(&owner).unwrap();
        assert_ne!(before, after);
        assert_eq!(owner.cache.borrow().entries.len(), 2);
        assert_eq!(
            Some(after),
            drawn.shape.bounding_box_with(&TextMeasurer::default())
        );
    }
}
