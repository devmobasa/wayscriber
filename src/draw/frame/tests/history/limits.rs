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
                    DrawnShape::with_metadata(
                        id,
                        snapshot.shape,
                        snapshot.created_at,
                        snapshot.locked,
                    ),
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
fn limited_clone_matches_clamp_and_preserves_live_history() {
    let mut frame = Frame::new();
    frame.set_page_name(Some("History page".into()));
    frame.set_view_offset(15, -20);
    for i in 0..8 {
        let id = frame.add_shape(Shape::Line {
            x1: i,
            y1: 0,
            x2: i + 10,
            y2: 10,
            color: BLACK,
            thick: 2.0,
        });
        let snapshot = frame.shape(id).unwrap().clone();
        frame.push_undo_action(
            UndoAction::Compound {
                actions: vec![UndoAction::Create {
                    shapes: vec![(i as usize, snapshot)],
                }],
            },
            0,
        );
    }
    frame.undo_last();
    frame.undo_last();
    let live = serde_json::to_value(&frame).unwrap();
    for limit in [0, 1, 3, usize::MAX] {
        let mut expected = frame.clone();
        expected.clamp_history_depth(limit);
        let mut actual = frame.clone_with_history_limit(limit);
        assert_eq!(
            serde_json::to_value(&actual).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
        for _ in 0..8 {
            expected.undo_last();
            actual.undo_last();
            assert_eq!(
                serde_json::to_value(&actual).unwrap(),
                serde_json::to_value(&expected).unwrap()
            );
        }
        for _ in 0..8 {
            expected.redo_last();
            actual.redo_last();
            assert_eq!(
                serde_json::to_value(&actual).unwrap(),
                serde_json::to_value(&expected).unwrap()
            );
        }
        assert_eq!(serde_json::to_value(&frame).unwrap(), live);
    }
}

#[test]
#[ignore = "release measurement: cargo test --release limited_history_copy_cost -- --ignored --nocapture"]
fn limited_history_copy_cost() {
    use std::{hint::black_box, time::Instant};
    let mut frame = Frame::new();
    let shape = Shape::Freehand {
        points: (0..4096).map(|i| (i, i % 100)).collect(),
        color: BLACK,
        thick: 2.0,
    };
    for _ in 0..200 {
        let id = frame.add_shape(shape.clone());
        frame.push_undo_action(
            UndoAction::Create {
                shapes: vec![(frame.len() - 1, frame.shape(id).unwrap().clone())],
            },
            0,
        );
    }
    for _ in 0..50 {
        frame.undo_last();
    }
    let pages = vec![frame; 4];
    for limit in [0, 10, usize::MAX] {
        let start = Instant::now();
        for _ in 0..20 {
            for page in &pages {
                let mut copy = page.clone();
                copy.clamp_history_depth(limit);
                black_box(copy);
            }
        }
        let before = start.elapsed();
        let start = Instant::now();
        for _ in 0..20 {
            for page in &pages {
                black_box(page.clone_with_history_limit(limit));
            }
        }
        eprintln!(
            "4 pages, 4096-point strokes, 150 undo + 50 redo, 20 captures; limit={limit}: clone/clamp={before:?}, retained copy={:?}",
            start.elapsed()
        );
    }
}
