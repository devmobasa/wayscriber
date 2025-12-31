use super::*;
use crate::draw::{BLACK, RED, Shape, frame::UndoAction};

#[test]
fn test_initial_mode_is_transparent() {
    let canvas_set = CanvasSet::new();
    assert_eq!(
        canvas_set.active_mode(),
        crate::input::BoardMode::Transparent
    );
}

#[test]
fn test_frame_created_on_first_mutable_access() {
    let mut canvas_set = CanvasSet::new();

    // Switch to whiteboard
    canvas_set.switch_mode(crate::input::BoardMode::Whiteboard);

    // Access the frame (this should create it via lazy initialization)
    let frame = canvas_set.active_frame_mut();

    // Frame should be empty initially
    assert_eq!(frame.shapes.len(), 0);
}

#[test]
fn test_frame_isolation() {
    let mut canvas_set = CanvasSet::new();

    // Add shape to transparent frame
    let frame = canvas_set.active_frame_mut();
    let id = frame.add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 100,
        y2: 100,
        color: RED,
        thick: 3.0,
    });
    let index = frame.find_index(id).unwrap();
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(index, frame.shape(id).unwrap().clone())],
        },
        10,
    );
    assert_eq!(canvas_set.active_frame().shapes.len(), 1);

    // Switch to whiteboard
    canvas_set.switch_mode(crate::input::BoardMode::Whiteboard);
    assert_eq!(canvas_set.active_frame().shapes.len(), 0); // Empty frame

    // Add shape to whiteboard frame
    let frame = canvas_set.active_frame_mut();
    let id = frame.add_shape(Shape::Rect {
        x: 10,
        y: 10,
        w: 50,
        h: 50,
        fill: false,
        color: BLACK,
        thick: 2.0,
    });
    let index = frame.find_index(id).unwrap();
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(index, frame.shape(id).unwrap().clone())],
        },
        10,
    );
    assert_eq!(canvas_set.active_frame().shapes.len(), 1);

    // Switch back to transparent
    canvas_set.switch_mode(crate::input::BoardMode::Transparent);
    assert_eq!(canvas_set.active_frame().shapes.len(), 1); // Original shape still there

    // Verify whiteboard still has its shape
    canvas_set.switch_mode(crate::input::BoardMode::Whiteboard);
    assert_eq!(canvas_set.active_frame().shapes.len(), 1);
}

#[test]
fn test_undo_isolation() {
    let mut canvas_set = CanvasSet::new();

    // Add and undo in transparent mode
    let frame = canvas_set.active_frame_mut();
    let id = frame.add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 100,
        y2: 100,
        color: RED,
        thick: 3.0,
    });
    let index = frame.find_index(id).unwrap();
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(index, frame.shape(id).unwrap().clone())],
        },
        10,
    );
    let _ = canvas_set.active_frame_mut().undo_last();
    assert_eq!(canvas_set.active_frame().shapes.len(), 0);

    // Switch to whiteboard and add shape
    canvas_set.switch_mode(crate::input::BoardMode::Whiteboard);
    let frame = canvas_set.active_frame_mut();
    let id = frame.add_shape(Shape::Rect {
        x: 10,
        y: 10,
        w: 50,
        h: 50,
        fill: false,
        color: BLACK,
        thick: 2.0,
    });
    let index = frame.find_index(id).unwrap();
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(index, frame.shape(id).unwrap().clone())],
        },
        10,
    );

    // Undo should only affect whiteboard frame
    let _ = canvas_set.active_frame_mut().undo();
    assert_eq!(canvas_set.active_frame().shapes.len(), 0);

    // Transparent frame should still be empty (undo happened there earlier)
    canvas_set.switch_mode(crate::input::BoardMode::Transparent);
    assert_eq!(canvas_set.active_frame().shapes.len(), 0);
}

#[test]
fn test_clear_active() {
    let mut canvas_set = CanvasSet::new();

    // Add shapes to transparent
    canvas_set.active_frame_mut().add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 100,
        y2: 100,
        color: RED,
        thick: 3.0,
    });

    // Add shapes to whiteboard
    canvas_set.switch_mode(crate::input::BoardMode::Whiteboard);
    canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 10,
        w: 50,
        h: 50,
        fill: false,
        color: BLACK,
        thick: 2.0,
    });

    // Clear whiteboard only
    canvas_set.clear_active();
    assert_eq!(canvas_set.active_frame().shapes.len(), 0);

    // Transparent should still have its shape
    canvas_set.switch_mode(crate::input::BoardMode::Transparent);
    assert_eq!(canvas_set.active_frame().shapes.len(), 1);
}

#[test]
fn test_immutable_access_to_nonexistent_frame() {
    let canvas_set = CanvasSet::new();

    // Accessing a non-existent board frame immutably should work
    // (returns empty frame reference, doesn't create it)
    // This test demonstrates the static EMPTY_PAGES pattern
    assert_eq!(canvas_set.active_frame().shapes.len(), 0);
}

#[test]
fn test_page_navigation_and_delete() {
    let mut canvas_set = CanvasSet::new();
    assert_eq!(
        canvas_set.page_count(crate::input::BoardMode::Transparent),
        1
    );
    assert_eq!(
        canvas_set.active_page_index(crate::input::BoardMode::Transparent),
        0
    );
    assert!(!canvas_set.next_page(crate::input::BoardMode::Transparent));

    canvas_set.new_page(crate::input::BoardMode::Transparent);
    assert_eq!(
        canvas_set.page_count(crate::input::BoardMode::Transparent),
        2
    );
    assert_eq!(
        canvas_set.active_page_index(crate::input::BoardMode::Transparent),
        1
    );
    assert!(canvas_set.prev_page(crate::input::BoardMode::Transparent));
    assert_eq!(
        canvas_set.active_page_index(crate::input::BoardMode::Transparent),
        0
    );

    canvas_set.duplicate_page(crate::input::BoardMode::Transparent);
    assert_eq!(
        canvas_set.page_count(crate::input::BoardMode::Transparent),
        3
    );
    assert_eq!(
        canvas_set.active_page_index(crate::input::BoardMode::Transparent),
        2
    );

    let outcome = canvas_set.delete_page(crate::input::BoardMode::Transparent);
    assert_eq!(outcome, PageDeleteOutcome::Removed);
    assert_eq!(
        canvas_set.page_count(crate::input::BoardMode::Transparent),
        2
    );
}

#[test]
fn test_delete_last_page_clears() {
    let mut canvas_set = CanvasSet::new();
    canvas_set.active_frame_mut().add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 10,
        y2: 10,
        color: RED,
        thick: 2.0,
    });

    let outcome = canvas_set.delete_page(crate::input::BoardMode::Transparent);
    assert_eq!(outcome, PageDeleteOutcome::Cleared);
    assert_eq!(canvas_set.active_frame().shapes.len(), 0);
}
