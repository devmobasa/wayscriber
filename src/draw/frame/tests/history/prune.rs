use std::collections::HashSet;

use crate::draw::frame::{DrawnShape, Frame, ShapeSnapshot, UndoAction};
use crate::draw::{Shape, color::BLACK};

#[test]
fn prune_history_for_removed_ids_prunes_shapes_and_actions() {
    let base_shape = Shape::Rect {
        x: 0,
        y: 0,
        w: 10,
        h: 10,
        fill: false,
        color: BLACK,
        thick: 1.0,
    };

    let shape1 = DrawnShape {
        id: 1,
        shape: base_shape.clone(),
        created_at: 0,
        locked: false,
    };
    let shape2 = DrawnShape {
        id: 2,
        shape: base_shape.clone(),
        created_at: 0,
        locked: false,
    };

    let create_both = UndoAction::Create {
        shapes: vec![(0, shape1), (1, shape2.clone())],
    };

    let modify_second = UndoAction::Modify {
        shape_id: 2,
        before: ShapeSnapshot {
            shape: base_shape.clone(),
            locked: false,
        },
        after: ShapeSnapshot {
            shape: base_shape,
            locked: false,
        },
    };

    let mut frame = Frame::new();
    frame.undo_stack = vec![create_both, modify_second];

    let mut removed = HashSet::new();
    removed.insert(2);

    let stats = frame.prune_history_for_removed_ids(&removed);
    assert_eq!(frame.undo_stack_len(), 1, "modify action should be removed");
    assert_eq!(
        stats.undo_removed, 1,
        "one action should be removed completely"
    );

    match &frame.undo_stack[0] {
        UndoAction::Create { shapes } => {
            assert_eq!(shapes.len(), 1);
            assert_eq!(shapes[0].1.id, 1);
        }
        other => panic!("expected create action, got {:?}", other),
    }
}

#[test]
fn prune_history_against_shapes_drops_actions_for_missing_ids() {
    let mut frame = Frame::new();

    // Add a single visible shape to the frame.
    let id_existing = frame.add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 10,
        y2: 10,
        color: BLACK,
        thick: 1.0,
    });

    let existing_snapshot = ShapeSnapshot {
        shape: Shape::Line {
            x1: 0,
            y1: 0,
            x2: 10,
            y2: 10,
            color: BLACK,
            thick: 1.0,
        },
        locked: false,
    };

    let missing_snapshot = ShapeSnapshot {
        shape: Shape::Line {
            x1: 5,
            y1: 5,
            x2: 15,
            y2: 15,
            color: BLACK,
            thick: 1.0,
        },
        locked: false,
    };

    let modify_existing = UndoAction::Modify {
        shape_id: id_existing,
        before: existing_snapshot.clone(),
        after: existing_snapshot,
    };

    let modify_missing = UndoAction::Modify {
        shape_id: 9999,
        before: missing_snapshot.clone(),
        after: missing_snapshot,
    };

    frame.undo_stack = vec![modify_existing, modify_missing];

    let stats = frame.prune_history_against_shapes();
    assert_eq!(frame.undo_stack_len(), 1);
    assert_eq!(stats.undo_removed, 1);

    match &frame.undo_stack[0] {
        UndoAction::Modify { shape_id, .. } => {
            assert_eq!(*shape_id, id_existing);
        }
        other => panic!("expected modify action, got {:?}", other),
    }
}
