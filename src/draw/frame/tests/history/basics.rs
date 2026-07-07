use crate::draw::frame::{Frame, ImageBoundsSnapshot, UndoAction};
use crate::draw::{EmbeddedImage, Shape, color::BLACK};

#[test]
fn undo_and_redo_cycle_shapes() {
    let mut frame = Frame::new();
    let shape = Shape::Line {
        x1: 0,
        y1: 0,
        x2: 10,
        y2: 10,
        color: BLACK,
        thick: 2.0,
    };

    let id = frame.add_shape(shape.clone());
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(
                frame.shapes.len().saturating_sub(1),
                frame.shape(id).unwrap().clone(),
            )],
        },
        10,
    );
    assert_eq!(frame.shapes.len(), 1);

    let undone = frame.undo_last();
    assert!(undone.is_some());
    assert_eq!(frame.shapes.len(), 0);

    let redone = frame.redo_last();
    assert!(redone.is_some());
    assert_eq!(frame.shapes.len(), 1);
}

#[test]
fn adding_new_shape_clears_redo_stack() {
    let mut frame = Frame::new();
    let first = Shape::Rect {
        x: 0,
        y: 0,
        w: 10,
        h: 10,
        fill: false,
        color: BLACK,
        thick: 2.0,
    };
    let id = frame.add_shape(first);
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(
                frame.shapes.len().saturating_sub(1),
                frame.shape(id).unwrap().clone(),
            )],
        },
        10,
    );
    frame.undo_last();
    assert_eq!(frame.shapes.len(), 0);

    let second = Shape::Rect {
        x: 10,
        y: 10,
        w: 15,
        h: 15,
        fill: false,
        color: BLACK,
        thick: 2.0,
    };
    frame.add_shape(second);
    assert_eq!(frame.redo_stack_len(), 0);
}

#[test]
fn modify_image_bounds_undo_redo_changes_geometry_without_replacing_payload() {
    let mut frame = Frame::new();
    let id = frame.add_shape(Shape::Image {
        x: 0,
        y: 0,
        w: 10,
        h: 8,
        data: EmbeddedImage {
            mime_type: "image/png".to_string(),
            width: 10,
            height: 8,
            bytes: vec![1, 2, 3, 4],
        },
    });
    if let Shape::Image { x, y, w, h, .. } = &mut frame.shape_mut(id).unwrap().shape {
        *x = 20;
        *y = 30;
        *w = 40;
        *h = 32;
    }
    frame.shape_mut(id).unwrap().invalidate_bounds();
    frame.push_undo_action(
        UndoAction::ModifyImageBounds {
            shape_id: id,
            before: ImageBoundsSnapshot {
                x: 0,
                y: 0,
                w: 10,
                h: 8,
                locked: false,
            },
            after: ImageBoundsSnapshot {
                x: 20,
                y: 30,
                w: 40,
                h: 32,
                locked: false,
            },
        },
        10,
    );

    frame.undo_last();
    match &frame.shape(id).unwrap().shape {
        Shape::Image { x, y, w, h, data } => {
            assert_eq!((*x, *y, *w, *h), (0, 0, 10, 8));
            assert_eq!(data.bytes, vec![1, 2, 3, 4]);
        }
        _ => panic!("expected image"),
    }

    frame.redo_last();
    match &frame.shape(id).unwrap().shape {
        Shape::Image { x, y, w, h, data } => {
            assert_eq!((*x, *y, *w, *h), (20, 30, 40, 32));
            assert_eq!(data.bytes, vec![1, 2, 3, 4]);
        }
        _ => panic!("expected image"),
    }
}
