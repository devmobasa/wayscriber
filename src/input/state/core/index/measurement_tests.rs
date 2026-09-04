use super::*;
use crate::draw::{ArrowLabel, ArrowStyle, FontDescriptor, RED, Shape, StepMarkerLabel};
use crate::input::hit_test;

fn scene(threshold: usize) -> (InputState, Vec<ShapeId>) {
    let mut state = crate::input::state::test_support::make_test_input_state();
    state.set_hit_test_threshold(threshold);
    state.set_hit_test_tolerance(1.0);
    let shapes = [
        Shape::Text {
            x: 100,
            y: 120,
            text: "Wide text".into(),
            size: 24.0,
            color: RED,
            font_descriptor: FontDescriptor::default(),
            background_enabled: true,
            wrap_width: Some(150),
        },
        Shape::StickyNote {
            x: 300,
            y: 120,
            text: "A note".into(),
            size: 24.0,
            background: RED,
            font_descriptor: FontDescriptor::default(),
            wrap_width: Some(100),
        },
        Shape::StepMarker {
            x: 500,
            y: 120,
            color: RED,
            label: StepMarkerLabel {
                value: 8888,
                size: 24.0,
                font_descriptor: FontDescriptor::default(),
            },
        },
        Shape::Arrow {
            x1: 100,
            y1: 300,
            x2: 200,
            y2: 300,
            color: RED,
            thick: 2.0,
            arrow_length: 12.0,
            arrow_angle: 30.0,
            head_at_end: true,
            style: ArrowStyle::Standard,
            bend: 0.0,
            label: Some(ArrowLabel {
                value: 8888,
                size: 24.0,
                font_descriptor: FontDescriptor::default(),
            }),
        },
    ];
    let ids = shapes
        .into_iter()
        .map(|shape| state.boards.active_frame_mut().add_shape(shape))
        .collect();
    (state, ids)
}

#[test]
fn cold_decorated_hits_match_linear_grid_and_public_entry_points() {
    let probes = [(105, 115), (305, 115), (500, 120), (150, 315)];
    for threshold in [usize::MAX, 1] {
        let measurer = TextMeasurer::default();
        let (mut state, ids) = scene(threshold);
        for (&id, &point) in ids.iter().zip(&probes) {
            assert_eq!(
                state.hit_test_at_with(&measurer, point.0, point.1),
                Some(id)
            );
            assert_eq!(
                state.hit_test_all_for_points_with(&measurer, &[point], 1.0),
                vec![id]
            );
            assert_eq!(
                state.hit_test_all_for_points_cached_with(&measurer, &[point], 1.0),
                vec![id]
            );
            let drawn = state.boards.active_frame().shape(id).unwrap();
            assert!(hit_test::hit_test_with(&measurer, drawn, point, 1.0));
            assert!(hit_test::hit_test_for_point_targeting_with(
                &measurer, drawn, point, 1.0
            ));
            assert!(
                hit_test::compute_hit_bounds_with(&measurer, drawn, 1.0)
                    .unwrap()
                    .contains(point.0, point.1)
            );
            assert_eq!(
                hit_test::compute_hit_bounds_with(&measurer, drawn, 1.0),
                hit_test::compute_hit_bounds(drawn, 1.0)
            );
            assert_eq!(state.hit_test_at(point.0, point.1), Some(id));
        }
        assert_eq!(state.has_spatial_index(), threshold == 1);
        for bad in [f64::NAN, f64::INFINITY, -1.0, f64::MAX] {
            assert!(
                state
                    .hit_test_all_for_points_with(&measurer, &probes, bad)
                    .is_empty()
            );
            let drawn = state.boards.active_frame().shape(ids[0]).unwrap();
            assert_eq!(
                hit_test::compute_hit_bounds_with(&measurer, drawn, bad),
                None
            );
            assert!(!hit_test::hit_test_with(&measurer, drawn, probes[0], bad));
            assert!(!hit_test::hit_test_for_point_targeting_with(
                &measurer, drawn, probes[0], bad
            ));
        }
    }
}

#[test]
fn explicit_index_refreshes_text_replacement_and_z_order_without_count_change() {
    let measurer = TextMeasurer::default();
    let (mut state, ids) = scene(1);
    assert_eq!(state.hit_test_at_with(&measurer, 105, 115), Some(ids[0]));
    let mut moved = state
        .boards
        .active_frame()
        .shape(ids[0])
        .unwrap()
        .shape
        .clone();
    if let Shape::Text {
        x,
        text,
        wrap_width,
        font_descriptor,
        ..
    } = &mut moved
    {
        *x = 700;
        *text = "Different wrapped text".into();
        *wrap_width = Some(70);
        font_descriptor.weight = "bold".into();
    }
    state
        .boards
        .active_frame_mut()
        .shape_mut(ids[0])
        .unwrap()
        .set_shape(moved.clone());
    state.invalidate_hit_cache_for_with(&measurer, ids[0]);
    assert_eq!(state.hit_test_at_with(&measurer, 105, 115), None);
    assert_eq!(state.hit_test_at_with(&measurer, 705, 115), Some(ids[0]));
    state
        .boards
        .active_frame_mut()
        .shape_mut(ids[1])
        .unwrap()
        .set_shape(moved);
    state.invalidate_hit_cache_for_with(&measurer, ids[1]);
    assert_eq!(state.hit_test_at_with(&measurer, 705, 115), Some(ids[1]));
    state.boards.active_frame_mut().move_shape(1, 0).unwrap();
    let shapes = &state.boards.active_frame().shapes;
    assert_eq!((shapes[0].id, shapes[1].id), (ids[1], ids[0]));
    assert_eq!(state.hit_test_at_with(&measurer, 705, 115), Some(ids[0]));
}

#[test]
fn explicit_point_targeting_keeps_fill_interiors_out_of_stroke_erasing() {
    let measurer = TextMeasurer::default();
    let (mut state, _) = scene(1);
    let id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 700,
        y: 300,
        w: 100,
        h: 100,
        fill: true,
        color: RED,
        thick: 2.0,
    });
    assert_eq!(state.hit_test_at_with(&measurer, 750, 350), Some(id));
    assert!(
        state
            .hit_test_all_for_points_with(&measurer, &[(750, 350)], 1.0)
            .is_empty()
    );
}
