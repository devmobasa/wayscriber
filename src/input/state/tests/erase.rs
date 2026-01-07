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
                label: None,
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

/// Tests that the spatial grid path works correctly with add/move/delete operations.
/// Uses a low threshold to force the grid path to be exercised.
#[test]
fn spatial_grid_eraser_hits_after_add_move_delete() {
    let mut state = create_test_input_state();
    state.eraser_size = 10.0;
    state.eraser_mode = EraserMode::Stroke;
    // Lower threshold to force spatial grid usage with fewer shapes
    state.set_hit_test_threshold(2);

    // Add enough shapes to trigger spatial grid (> threshold)
    let mut shape_ids = Vec::new();
    for i in 0..5 {
        let id = state.canvas_set.active_frame_mut().add_shape(Shape::Line {
            x1: i * 100,
            y1: 0,
            x2: i * 100 + 50,
            y2: 50,
            color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thick: 2.0,
        });
        shape_ids.push(id);
    }

    // Force spatial index build
    state.ensure_spatial_index_for_active_frame();
    assert!(
        state.has_spatial_index(),
        "spatial index should be built with {} shapes",
        shape_ids.len()
    );

    // Test hit on first shape using the grid
    let hit = state.hit_test_at(25, 25);
    assert_eq!(hit, Some(shape_ids[0]), "should hit first shape via grid");

    // Delete a shape and verify grid updates correctly
    state
        .canvas_set
        .active_frame_mut()
        .remove_shape_by_id(shape_ids[2]);
    state.invalidate_hit_cache_for(shape_ids[2]);

    // Test hit on shape after deleted one still works
    let hit = state.hit_test_at(325, 25);
    assert_eq!(
        hit,
        Some(shape_ids[3]),
        "should hit fourth shape after deletion"
    );

    // Modify a shape's position and verify grid updates
    if let Some(drawn) = state.canvas_set.active_frame_mut().shape_mut(shape_ids[4])
        && let Shape::Line {
            ref mut x1,
            ref mut y1,
            ref mut x2,
            ref mut y2,
            ..
        } = drawn.shape
    {
        *x1 = 0;
        *y1 = 200;
        *x2 = 50;
        *y2 = 250;
    }
    state.invalidate_hit_cache_for(shape_ids[4]);

    // Test hit at new position
    let hit = state.hit_test_at(25, 225);
    assert_eq!(hit, Some(shape_ids[4]), "should hit moved shape at new pos");

    // Test eraser with spatial grid using large tolerance
    state.set_hit_test_tolerance(20.0);
    let erased = state.erase_strokes_by_points(&[(25, 25)]);
    assert!(erased, "eraser should hit first shape with large tolerance");
    assert!(
        state
            .canvas_set
            .active_frame()
            .shape(shape_ids[0])
            .is_none()
    );
}

/// Tests that tolerance larger than cell size still finds shapes.
#[test]
fn spatial_grid_large_tolerance_finds_distant_shapes() {
    let mut state = create_test_input_state();
    state.set_hit_test_threshold(2);
    state.set_hit_test_tolerance(100.0); // Larger than cell size (64)

    // Add shapes spread apart
    for i in 0..5 {
        state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
            x: i * 200,
            y: 0,
            w: 10,
            h: 10,
            fill: true,
            color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thick: 1.0,
        });
    }

    state.ensure_spatial_index_for_active_frame();
    assert!(state.has_spatial_index());

    // Query point is 80 pixels away from shape, but tolerance is 100
    // Without tolerance-aware query, this would miss the shape
    let hit = state.hit_test_at(90, 5); // 80 pixels from rect at x=0..10
    assert!(hit.is_some(), "should find shape within large tolerance");
}
