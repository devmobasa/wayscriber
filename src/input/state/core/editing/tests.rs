use super::*;

fn rectangle(x: i32) -> Shape {
    Shape::Rect {
        x,
        y: 10,
        w: 20,
        h: 20,
        fill: false,
        color: crate::draw::WHITE,
        thick: 2.0,
    }
}

#[test]
fn rollback_restores_preview_without_touching_history_or_locked_shapes() {
    let mut frame = Frame::new();
    let first = frame.add_shape(rectangle(10));
    let locked = frame.add_shape(rectangle(50));
    frame.shape_mut(locked).unwrap().locked = true;
    let history = frame.undo_stack_len();
    let measurer = TextMeasurer::default();
    let edit = CanvasEdit::capture(&frame, &[first, locked]);
    let effects = edit.preview(&mut frame, &measurer, |shape, _| {
        shape.translate(25, 0);
        true
    });
    assert_eq!(effects.regions.len(), 1);
    assert!(!effects.committed);
    assert_eq!(frame.shape(first).unwrap().shape, rectangle(35));
    assert_eq!(frame.shape(locked).unwrap().shape, rectangle(50));
    let effects = edit.rollback(&mut frame, &measurer);
    assert_eq!(effects.regions.len(), 1);
    assert!(!effects.committed);
    assert_ne!(effects.regions[0].1, effects.regions[0].2);
    assert_eq!(frame.shape(first).unwrap().shape, rectangle(10));
    assert!(frame.shape(locked).unwrap().locked);
    assert_eq!(frame.undo_stack_len(), history);
}

#[test]
fn returning_to_original_geometry_keeps_redo_and_history() {
    let mut frame = Frame::new();
    let id = frame.add_shape(rectangle(10));
    let second = frame.add_shape(rectangle(50));
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(1, frame.shape(second).unwrap().clone())],
        },
        100,
    );
    frame.undo_last().unwrap();
    let history = frame.undo_stack_len();
    let measurer = TextMeasurer::default();
    let edit = CanvasEdit::capture(&frame, &[id]);
    let _ = edit.preview(&mut frame, &measurer, |shape, _| {
        shape.translate(20, 0);
        true
    });
    let _ = edit.preview(&mut frame, &measurer, |shape, _| {
        shape.translate(-20, 0);
        true
    });
    assert!(!edit.commit(&mut frame, 100).committed);
    assert_eq!(frame.undo_stack_len(), history);
    assert!(frame.redo_last().is_some());
    assert_eq!(frame.shapes.len(), 2);
}

#[test]
fn deletion_preserves_locked_shapes_and_restores_original_order_in_one_undo() {
    let mut frame = Frame::new();
    let first = frame.add_shape(rectangle(10));
    let locked = frame.add_shape(rectangle(50));
    let last = frame.add_shape(rectangle(90));
    frame.shape_mut(locked).unwrap().locked = true;
    let effects = CanvasEdit::delete(
        &mut frame,
        &[first, locked, last].into_iter().collect(),
        &TextMeasurer::default(),
        100,
    );
    assert!(effects.committed);
    assert_eq!(effects.regions.len(), 2);
    assert_eq!(
        frame
            .shapes
            .iter()
            .map(|shape| shape.id)
            .collect::<Vec<_>>(),
        vec![locked]
    );
    frame.undo_last().unwrap();
    assert_eq!(
        frame
            .shapes
            .iter()
            .map(|shape| shape.id)
            .collect::<Vec<_>>(),
        vec![first, locked, last]
    );
    assert!(frame.shape(locked).unwrap().locked);
    frame.redo_last().unwrap();
    assert_eq!(
        frame
            .shapes
            .iter()
            .map(|shape| shape.id)
            .collect::<Vec<_>>(),
        vec![locked]
    );
}
