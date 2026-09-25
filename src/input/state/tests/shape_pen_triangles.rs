use super::*;
use crate::draw::PolygonKind;
use crate::input::tool::ProvisionalToolStroke;

/// A quick hand-drawn triangle from the apex: wobbly sides, an overshoot at
/// the right corner, and a rounded left corner.
const HAND_DRAWN: [(i32, i32); 23] = [
    (101, 21),
    (110, 37),
    (121, 54),
    (130, 72),
    (143, 91),
    (153, 106),
    (162, 124),
    (173, 143),
    (169, 141),
    (148, 142),
    (131, 142),
    (112, 139),
    (91, 140),
    (73, 141),
    (54, 138),
    (31, 137),
    (41, 119),
    (50, 103),
    (62, 89),
    (70, 70),
    (81, 55),
    (91, 40),
    (99, 22),
];

fn draw_path(state: &mut InputState, path: &[(i32, i32)]) {
    let (first, rest) = path.split_first().expect("path has points");
    state.on_mouse_press(MouseButton::Left, first.0, first.1);
    for &(x, y) in rest {
        state.on_mouse_motion(x, y);
    }
}

fn release_at_end(state: &mut InputState, path: &[(i32, i32)]) {
    let last = path.last().expect("path has points");
    state.on_mouse_release(MouseButton::Left, last.0, last.1);
}

/// Equilateral triangle with each corner rounded off along `cut` of both
/// sides beside it.
fn rounded_triangle(cut: f64) -> Vec<(i32, i32)> {
    let corners = [(200.0, 120.0), (270.0, 241.0), (130.0, 241.0)];
    let toward = |from: (f64, f64), to: (f64, f64), t: f64| {
        (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t)
    };

    let mut path = Vec::new();
    for index in 0..3 {
        let corner = corners[index];
        let before = toward(corner, corners[(index + 2) % 3], cut);
        let after = toward(corner, corners[(index + 1) % 3], cut);
        for step in 0..8 {
            let t = f64::from(step) / 8.0;
            path.push(toward(
                toward(before, corner, t),
                toward(corner, after, t),
                t,
            ));
        }
        let side_end = toward(corners[(index + 1) % 3], corner, cut);
        for step in 0..8 {
            path.push(toward(after, side_end, f64::from(step) / 8.0));
        }
    }
    path.push(path[0]);

    path.into_iter()
        .map(|(x, y)| (x.round() as i32, y.round() as i32))
        .collect()
}

#[test]
fn live_shape_previews_and_commits_hand_drawn_triangles() {
    let mut state = create_test_input_state();
    assert!(state.set_tool_override(Some(Tool::LiveShape)));

    draw_path(&mut state, &HAND_DRAWN);
    assert!(matches!(
        state.provisional_tool_stroke(99, 22),
        ProvisionalToolStroke::Shape(Shape::Polygon {
            kind: PolygonKind::Triangle,
            ..
        })
    ));
    release_at_end(&mut state, &HAND_DRAWN);

    let frame = state.boards.active_frame();
    assert_eq!(frame.shapes.len(), 1);
    assert_eq!(frame.undo_stack_len(), 2, "the ink, then the recognition");
    let Shape::Polygon {
        kind: PolygonKind::Triangle,
        points,
        fill: false,
        ..
    } = &frame.shapes[0].shape
    else {
        panic!("expected a triangle, got {:?}", frame.shapes[0].shape);
    };
    assert_eq!(points.len(), 3);
    for (x, y) in [(100, 20), (170, 140), (30, 140)] {
        assert!(
            points
                .iter()
                .any(|&(px, py)| (px - x).abs() <= 5 && (py - y).abs() <= 5),
            "no vertex near ({x}, {y}) in {points:?}"
        );
    }

    let mut base: Vec<_> = points.iter().filter(|&&(_, y)| y > 100).collect();
    base.sort();
    assert_eq!(base.len(), 2);
    assert_eq!(base[0].1, base[1].1, "the base should be levelled");
}

#[test]
fn live_shape_sensitivity_controls_rounded_triangles() {
    let path = rounded_triangle(0.2);

    for (sensitivity, expected_kind) in [(0, "Freehand"), (4, "Triangle")] {
        let mut state = create_test_input_state();
        state.style.shape_recognition_sensitivity = sensitivity;
        assert!(state.set_tool_override(Some(Tool::LiveShape)));

        draw_path(&mut state, &path);
        release_at_end(&mut state, &path);

        assert_eq!(
            state.boards.active_frame().shapes[0].shape.kind_name(),
            expected_kind,
            "sensitivity {sensitivity}"
        );
    }
}
