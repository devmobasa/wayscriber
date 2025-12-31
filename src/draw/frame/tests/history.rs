use crate::draw::frame::{DrawnShape, Frame, ShapeSnapshot, UndoAction};
use crate::draw::{Shape, color::BLACK};
use std::collections::HashSet;

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
