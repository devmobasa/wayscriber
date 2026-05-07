use crate::draw::frame::{Frame, ImageBoundsSnapshot, UndoAction};
use crate::draw::{EmbeddedImage, Shape, color::BLACK};
use base64::{Engine as _, engine::general_purpose};

#[test]
fn frame_serializes_history() {
    let mut frame = Frame::new();
    let first = frame.add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 10,
        y2: 10,
        color: BLACK,
        thick: 2.0,
    });
    let first_index = frame.find_index(first).unwrap();
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(first_index, frame.shape(first).unwrap().clone())],
        },
        100,
    );

    let second = frame.add_shape(Shape::Line {
        x1: 1,
        y1: 1,
        x2: 5,
        y2: 5,
        color: BLACK,
        thick: 2.0,
    });
    let second_index = frame.find_index(second).unwrap();
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(second_index, frame.shape(second).unwrap().clone())],
        },
        100,
    );
    // Move the second action to the redo stack.
    frame.undo_last();

    assert_eq!(frame.undo_stack_len(), 1);
    assert_eq!(frame.redo_stack_len(), 1);

    let json = serde_json::to_string(&frame).expect("serialize frame");
    let mut restored: Frame = serde_json::from_str(&json).expect("deserialize frame");
    assert_eq!(restored.undo_stack_len(), 1);
    assert_eq!(restored.redo_stack_len(), 1);

    let new_id = restored.add_shape(Shape::Line {
        x1: 2,
        y1: 2,
        x2: 6,
        y2: 6,
        color: BLACK,
        thick: 1.0,
    });
    assert!(new_id > second);
}

#[test]
fn frame_with_history_is_persistable_even_without_shapes() {
    let mut frame = Frame::new();
    let id = frame.add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 20,
        y2: 20,
        color: BLACK,
        thick: 2.0,
    });
    let index = frame.find_index(id).unwrap();
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(index, frame.shape(id).unwrap().clone())],
        },
        100,
    );

    // Undo to move the action into redo stack and clear the canvas.
    frame.undo_last();

    assert!(frame.shapes.is_empty());
    assert!(frame.has_persistable_data());
}

#[test]
fn frame_with_view_offset_is_persistable_even_without_shapes() {
    let mut frame = Frame::new();

    assert!(frame.set_view_offset(240, -180));
    assert!(frame.has_persistable_data());

    let json = serde_json::to_string(&frame).expect("serialize frame");
    let restored: Frame = serde_json::from_str(&json).expect("deserialize frame");
    assert_eq!(restored.view_offset(), (240, -180));
}

#[test]
fn try_add_shape_respects_limit() {
    let mut frame = Frame::new();
    assert!(frame.try_add_shape(
        Shape::Line {
            x1: 0,
            y1: 0,
            x2: 1,
            y2: 1,
            color: BLACK,
            thick: 2.0,
        },
        1
    ));
    assert!(!frame.try_add_shape(
        Shape::Line {
            x1: 1,
            y1: 1,
            x2: 2,
            y2: 2,
            color: BLACK,
            thick: 2.0,
        },
        1
    ));
}

#[test]
fn image_bounds_history_serializes_without_duplicate_image_payloads() {
    let bytes = vec![42u8; 256];
    let encoded = general_purpose::STANDARD.encode(&bytes);
    let mut frame = Frame::new();
    let id = frame.add_shape(Shape::Image {
        x: 0,
        y: 0,
        w: 16,
        h: 16,
        data: EmbeddedImage {
            mime_type: "image/png".to_string(),
            width: 16,
            height: 16,
            bytes,
        },
    });
    let index = frame.find_index(id).unwrap();
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(index, frame.shape(id).unwrap().clone())],
        },
        100,
    );
    frame.push_undo_action(
        UndoAction::ModifyImageBounds {
            shape_id: id,
            before: ImageBoundsSnapshot {
                x: 0,
                y: 0,
                w: 16,
                h: 16,
                locked: false,
            },
            after: ImageBoundsSnapshot {
                x: 4,
                y: 5,
                w: 32,
                h: 32,
                locked: false,
            },
        },
        100,
    );

    let json = serde_json::to_string(&frame).expect("serialize frame with image history");
    assert!(json.contains("\"kind\":\"modify_image_bounds\""));
    assert_eq!(
        json.matches(&encoded).count(),
        2,
        "visible shape plus create history should contain image bytes, bounds history should not"
    );
}
