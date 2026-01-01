use crate::draw::frame::{DrawnShape, Frame, UndoAction};
use crate::draw::{Shape, color::BLACK};

#[test]
fn undo_stack_respects_limit() {
    let mut frame = Frame::new();
    for i in 0..5 {
        let shape = Shape::Line {
            x1: i,
            y1: 0,
            x2: i + 10,
            y2: 10,
            color: BLACK,
            thick: 2.0,
        };
        let id = frame.add_shape(shape);
        let index = frame.find_index(id).unwrap();
        let snapshot = frame.shape(id).unwrap().clone();
        frame.push_undo_action(
            UndoAction::Create {
                shapes: vec![(index, snapshot)],
            },
            3,
        );
    }

    assert_eq!(frame.undo_stack_len(), 3);
}

#[test]
fn clamp_history_depth_clears_both_stacks() {
    let mut frame = Frame::new();
    for i in 0..3 {
        let shape = Shape::Line {
            x1: i,
            y1: 0,
            x2: i + 10,
            y2: 10,
            color: BLACK,
            thick: 2.0,
        };
        let id = frame.add_shape(shape);
        let index = frame.find_index(id).unwrap();
        let snapshot = frame.shape(id).unwrap().clone();
        frame.push_undo_action(
            UndoAction::Create {
                shapes: vec![(
                    index,
                    DrawnShape {
                        id,
                        shape: snapshot.shape,
                        created_at: snapshot.created_at,
                        locked: snapshot.locked,
                    },
                )],
            },
            10,
        );
    }

    frame.undo_last();
    frame.undo_last();
    assert_eq!(frame.undo_stack_len(), 1);
    assert_eq!(frame.redo_stack_len(), 2);

    let stats = frame.clamp_history_depth(0);
    assert_eq!(stats.undo_removed, 1);
    assert_eq!(stats.redo_removed, 2);
    assert_eq!(frame.undo_stack_len(), 0);
    assert_eq!(frame.redo_stack_len(), 0);
}
