use crate::draw::frame::{Frame, UndoAction};
use crate::draw::{Shape, color::BLACK};

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
