use crate::draw::frame::{DrawnShape, Frame, UndoAction};
use crate::draw::{Shape, color::BLACK};

#[test]
fn validate_history_drops_actions_exceeding_compound_depth() {
    let base_shape = Shape::Line {
        x1: 0,
        y1: 0,
        x2: 10,
        y2: 10,
        color: BLACK,
        thick: 1.0,
    };

    let shallow_drawn = DrawnShape {
        id: 1,
        shape: base_shape.clone(),
        created_at: 0,
        locked: false,
    };
    let deep_drawn = DrawnShape {
        id: 2,
        shape: base_shape,
        created_at: 0,
        locked: false,
    };

    let shallow = UndoAction::Compound(vec![UndoAction::Create {
        shapes: vec![(0, shallow_drawn)],
    }]);

    // Nested compound to create depth 3.
    let deep = UndoAction::Compound(vec![UndoAction::Compound(vec![UndoAction::Create {
        shapes: vec![(0, deep_drawn)],
    }])]);

    let mut frame = Frame::new();
    frame.undo_stack = vec![shallow, deep];

    let stats = frame.validate_history(2);
    assert_eq!(frame.undo_stack_len(), 1, "deep action should be dropped");
    assert_eq!(stats.undo_removed, 1);

    match &frame.undo_stack[0] {
        UndoAction::Compound(actions) => {
            assert_eq!(actions.len(), 1, "shallow compound should be preserved");
        }
        other => panic!("expected compound action, got {:?}", other),
    }
}
