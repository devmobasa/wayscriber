//! Input-to-committed-ink precision, sampling, and fractional-scale proof.
use super::*;

fn curve(x: f64) -> f64 {
    300.2 + 0.10 * x + 4.0 * (x / 30.0).sin()
}

fn samples(step: f64) -> Vec<(f64, f64)> {
    let mut points: Vec<_> = (0..=(180.0 / step).floor() as usize)
        .map(|i| {
            let x = i as f64 * step;
            (300.2 + x, curve(x))
        })
        .collect();
    let end = (480.2, curve(180.0));
    if points.last() != Some(&end) {
        points.push(end);
    }
    points
}

fn draw(points: &[(f64, f64)], level: u8, tool: Tool) -> Shape {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(tool));
    state.set_pen_smoothing(level);
    let first = points[0];
    state.on_mouse_press_with_canvas(MouseButton::Left, 300, 300, first.0, first.1);
    for &(x, y) in &points[1..] {
        state.on_mouse_motion_with_canvas(300, 300, x, y);
    }
    let last = *points.last().unwrap();
    state.on_mouse_release_with_canvas(MouseButton::Left, 300, 300, last.0, last.1);
    state
        .boards
        .active_frame()
        .shapes
        .last()
        .unwrap()
        .shape
        .clone()
}

fn ink(shape: &Shape) -> &[(f64, f64)] {
    match shape {
        Shape::Freehand { points, .. } | Shape::MarkerStroke { points, .. } => points,
        _ => panic!("expected ink"),
    }
}

fn rms(points: &[(f64, f64)]) -> f64 {
    (points
        .iter()
        .map(|&(x, y)| (y - curve(x - 300.2)).powi(2))
        .sum::<f64>()
        / points.len() as f64)
        .sqrt()
        * 5.0
        / 3.0
}

fn interpolated_y(points: &[(f64, f64)], x: f64) -> f64 {
    let i = points
        .partition_point(|p| p.0 < x)
        .clamp(1, points.len() - 1);
    let (a, b) = (points[i - 1], points[i]);
    a.1 + (b.1 - a.1) * (x - a.0) / (b.0 - a.0)
}

#[test]
fn slow_and_fast_subpixel_strokes_keep_similar_spatial_smoothing() {
    // Unequal, non-aligned event intervals exercise spacing without assuming
    // that a fast event coincides with each retained slow event.
    let slow = samples(0.047);
    let fast = samples(0.83);
    for tool in [Tool::Pen, Tool::Marker] {
        for level in 0..=6 {
            let slow_shape = draw(&slow, level, tool);
            let fast_shape = draw(&fast, level, tool);
            let (a, b) = (ink(&slow_shape), ink(&fast_shape));
            for path in [a, b] {
                assert_eq!(path.first(), slow.first(), "start is pinned");
                assert_eq!(path.last(), slow.last(), "end is pinned");
                assert!(rms(path) < 0.02, "{tool:?} level {level}: {}", rms(path));
                for w in path.windows(2) {
                    assert_ne!(w[0], w[1], "no duplicate positions");
                    if w[0].1 == w[1].1 {
                        assert!(
                            (curve(w[1].0 - 300.2) - curve(w[0].0 - 300.2)).abs() < 0.002,
                            "flat segments only near the curve's natural turning points"
                        );
                    }
                }
            }
            let max_difference = (0..=720)
                .map(|i| {
                    let x = 300.2 + i as f64 / 4.0;
                    (interpolated_y(a, x) - interpolated_y(b, x)).abs() * 5.0 / 3.0
                })
                .fold(0.0, f64::max);
            assert!(
                max_difference < 0.02,
                "{tool:?} level {level}: {max_difference}"
            );
        }
    }
}

#[test]
fn smoothed_commits_compact_interior_precision_but_preserve_endpoints_and_off() {
    let raw = [
        (300.123456, 300.234567),
        (301.345678, 301.876543),
        (302.456789, 300.654321),
        (303.765432, 301.987654),
    ];
    for tool in [Tool::Pen, Tool::Marker] {
        assert_eq!(ink(&draw(&raw, 0, tool)), raw);
        let shape = draw(&raw, 1, tool);
        let points = ink(&shape);
        assert_eq!(points[0], raw[0]);
        assert_eq!(points[3], raw[3]);
        assert_eq!(points[1], (301.318, 301.16));
        assert_eq!(points[2], (302.506, 301.293));
        let saved = serde_json::to_string(&shape).unwrap();
        let restored: Shape = serde_json::from_str(&saved).unwrap();
        assert_eq!(ink(&restored), points, "saved and in-memory ink agree");
    }
}

#[test]
fn thinning_keeps_pressure_with_its_sample_and_level_zero_keeps_positions() {
    let mut state = create_test_input_state();
    state.set_pen_smoothing(0);
    state.style.pressure_variation_threshold = 0.0;
    state.on_mouse_press_with_canvas(MouseButton::Left, 300, 300, 300.25, 300.5);
    for (x, width) in [
        (300.5, 9.0),
        (301.0, 4.0),
        (301.0, 12.0),
        (301.2, 6.0),
        (301.75, 8.0),
    ] {
        state.style.tool_settings.get_mut(Tool::Pen).thickness = width;
        state.on_mouse_motion_with_canvas(300, 300, x, 300.5);
    }
    let DrawingState::Drawing {
        points,
        point_thicknesses,
        ..
    } = &state.state
    else {
        panic!("drawing");
    };
    assert_eq!(points, &[(300.25, 300.5), (301.0, 300.5), (301.75, 300.5)]);
    assert_eq!(point_thicknesses, &[9.0, 12.0, 8.0]);
    let initial_width = point_thicknesses[0];
    state.on_mouse_release_with_canvas(MouseButton::Left, 300, 300, 301.9, 300.6);
    let Shape::FreehandPressure { points, .. } =
        &state.boards.active_frame().shapes.last().unwrap().shape
    else {
        panic!("pressure stroke");
    };
    assert_eq!(
        points,
        &[
            (300.25, 300.5, initial_width),
            (301.0, 300.5, 12.0),
            (301.75, 300.5, 8.0),
            (301.9, 300.6, 8.0)
        ]
    );
}

#[test]
fn invalid_fractional_pointer_samples_do_not_enter_a_stroke() {
    for x in [f64::NAN, f64::INFINITY, 1e30] {
        let mut state = create_test_input_state();
        state.on_mouse_press_with_canvas(MouseButton::Left, 300, 300, x, 300.5);
        assert!(matches!(state.state, DrawingState::Idle));

        state.set_pen_smoothing(0);
        state.on_mouse_press_with_canvas(MouseButton::Left, 300, 300, 300.25, 300.5);
        state.on_mouse_motion_with_canvas(300, 300, x, 300.5);
        state.on_mouse_release_with_canvas(MouseButton::Left, 300, 300, x, 300.5);
        assert!(matches!(state.state, DrawingState::Drawing { .. }));
        state.on_mouse_release_with_canvas(MouseButton::Left, 300, 300, 301.25, 300.75);
        assert_eq!(
            ink(&state.boards.active_frame().shapes.last().unwrap().shape),
            &[(300.25, 300.5), (301.25, 300.75)]
        );
    }
}
