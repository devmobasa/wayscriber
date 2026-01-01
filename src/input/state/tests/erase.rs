use super::*;

#[test]
fn erase_stroke_samples_sparse_path() {
    let mut state = create_test_input_state();
    state.eraser_size = 4.0;
    state.eraser_mode = EraserMode::Stroke;

    let line_id = state.canvas_set.active_frame_mut().add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 100,
        y2: 0,
        color: Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 1.0,
    });

    let erased = state.erase_strokes_by_points(&[(0, -10), (100, 10)]);
    assert!(erased, "stroke eraser should remove intersected line");
    assert!(state.canvas_set.active_frame().shape(line_id).is_none());
}

#[test]
fn erase_stroke_includes_release_segment() {
    let mut state = create_test_input_state();
    state.eraser_size = 4.0;
    state.eraser_mode = EraserMode::Stroke;
    state.set_tool_override(Some(Tool::Eraser));

    let line_id = state.canvas_set.active_frame_mut().add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 100,
        y2: 0,
        color: Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 1.0,
    });

    state.on_mouse_press(MouseButton::Left, 0, -10);
    state.on_mouse_release(MouseButton::Left, 100, 10);

    assert!(state.canvas_set.active_frame().shape(line_id).is_none());
}

#[test]
fn erase_stroke_skips_locked_shapes() {
    let mut state = create_test_input_state();
    state.eraser_size = 4.0;
    state.eraser_mode = EraserMode::Stroke;

    let locked_id = state.canvas_set.active_frame_mut().add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 100,
        y2: 0,
        color: Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 1.0,
    });
    let unlocked_id = state.canvas_set.active_frame_mut().add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 100,
        y2: 0,
        color: Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 1.0,
    });

    if let Some(index) = state.canvas_set.active_frame().find_index(locked_id) {
        state.canvas_set.active_frame_mut().shapes[index].locked = true;
    }

    let erased = state.erase_strokes_by_points(&[(0, -10), (100, 10)]);
    assert!(erased, "eraser should remove unlocked shapes");
    assert!(state.canvas_set.active_frame().shape(unlocked_id).is_none());
    assert!(state.canvas_set.active_frame().shape(locked_id).is_some());
}

#[test]
fn erase_stroke_samples_randomized_crossings() {
    fn next_unit(seed: &mut u64) -> f64 {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let value = ((*seed >> 33) as u32) as f64;
        value / (u32::MAX as f64)
    }

    let mut seed = 0x1234_5678_9abc_def0u64;
    for _ in 0..16 {
        let mut state = create_test_input_state();
        state.eraser_size = 4.0;
        state.eraser_mode = EraserMode::Stroke;

        let line_id = state.canvas_set.active_frame_mut().add_shape(Shape::Line {
            x1: 0,
            y1: 0,
            x2: 100,
            y2: 0,
            color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thick: 1.0,
        });

        let unit = next_unit(&mut seed);
        let angle = std::f64::consts::PI * (0.35 + unit * 0.3);
        let dx = angle.cos();
        let dy = angle.sin();
        let length = 80.0;
        let x0 = 50.0 - dx * length;
        let y0 = 0.0 - dy * length;
        let x1 = 50.0 + dx * length;
        let y1 = 0.0 + dy * length;

        let erased = state.erase_strokes_by_points(&[
            (x0.round() as i32, y0.round() as i32),
            (x1.round() as i32, y1.round() as i32),
        ]);

        assert!(
            erased,
            "stroke eraser should remove line at angle {}",
            angle
        );
        assert!(state.canvas_set.active_frame().shape(line_id).is_none());
    }
}

#[test]
fn erase_stroke_hits_various_shapes() {
    let cases = vec![
        (
            Shape::Rect {
                x: 10,
                y: 10,
                w: 40,
                h: 20,
                fill: false,
                color: Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
                thick: 1.0,
            },
            vec![(0, 10), (100, 10)],
        ),
        (
            Shape::Ellipse {
                cx: 50,
                cy: 50,
                rx: 20,
                ry: 10,
                fill: false,
                color: Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
                thick: 1.0,
            },
            vec![(0, 40), (100, 40)],
        ),
        (
            Shape::Arrow {
                x1: 10,
                y1: 90,
                x2: 90,
                y2: 90,
                color: Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
                thick: 1.0,
                arrow_length: 20.0,
                arrow_angle: 30.0,
                head_at_end: true,
            },
            vec![(0, 90), (100, 90)],
        ),
    ];

    for (shape, path) in cases {
        let mut state = create_test_input_state();
        state.eraser_size = 4.0;
        state.eraser_mode = EraserMode::Stroke;
        let shape_id = state.canvas_set.active_frame_mut().add_shape(shape);

        let erased = state.erase_strokes_by_points(&path);
        assert!(erased, "stroke eraser should remove intersected shape");
        assert!(state.canvas_set.active_frame().shape(shape_id).is_none());
    }
}
