use crate::draw::{
    ArrowLabel, ArrowStyle, FontDescriptor, RED, Shape, ShapeId, StepMarkerLabel, TextMeasurer,
};
use crate::input::InputState;
use crate::input::state::core::base::SelectionHandle;
use crate::util::Rect;

fn fixtures() -> Vec<(Shape, (i32, i32))> {
    vec![
        (
            Shape::Text {
                x: 100,
                y: 200,
                text: "Wide wrapping words wrap more".into(),
                size: 24.0,
                color: RED,
                font_descriptor: FontDescriptor::default(),
                background_enabled: true,
                wrap_width: Some(150),
            },
            (105, 195),
        ),
        (
            Shape::StickyNote {
                x: 100,
                y: 200,
                text: "Wide wrapping words wrap more".into(),
                size: 24.0,
                background: RED,
                font_descriptor: FontDescriptor::default(),
                wrap_width: Some(150),
            },
            (105, 195),
        ),
        (
            Shape::StepMarker {
                x: 100,
                y: 200,
                color: RED,
                label: StepMarkerLabel {
                    value: 8888,
                    size: 24.0,
                    font_descriptor: FontDescriptor::default(),
                },
            },
            (100, 200),
        ),
        (
            Shape::Arrow {
                x1: 100,
                y1: 200,
                x2: 200,
                y2: 200,
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
            (150, 215),
        ),
    ]
}

fn state_with(shape: Shape) -> (InputState, ShapeId, ShapeId) {
    let mut state = crate::input::state::test_support::make_test_input_state();
    state.view.set_screen_dimensions(1000, 800);
    state.set_hit_test_threshold(1);
    state.set_hit_test_tolerance(1.0);
    let mut locked = shape.clone();
    locked.translate(500, 0);
    let frame = state.boards.active_frame_mut();
    let id = frame.add_shape(shape);
    let locked_id = frame.add_shape(locked);
    frame.shape_mut(locked_id).unwrap().locked = true;
    state.set_selection(vec![id, locked_id]);
    state.take_dirty_regions();
    (state, id, locked_id)
}

fn bounds(state: &InputState, owner: &TextMeasurer, id: ShapeId) -> Rect {
    state
        .boards
        .active_frame()
        .shape(id)
        .unwrap()
        .bounding_box_with(owner)
        .unwrap()
}

fn assert_dirty_covers(dirty: &[Rect], expected: Rect) {
    for (x, y) in [
        (expected.x, expected.y),
        (
            expected.x + expected.width - 1,
            expected.y + expected.height - 1,
        ),
    ] {
        assert!(
            dirty.iter().any(|rect| rect.contains(x, y)),
            "missing dirty point ({x}, {y}) for {expected:?}: {dirty:?}"
        );
    }
    assert!(
        !dirty.iter().any(|rect| rect.contains(999, 799)),
        "fixture should exercise local damage rather than full damage"
    );
}

#[test]
fn explicit_translation_restores_decorated_shapes_and_keeps_locked_geometry() {
    for (shape, probe) in fixtures() {
        let owner = TextMeasurer::default();
        let (mut state, id, locked_id) = state_with(shape);
        let original = bounds(&state, &owner, id);
        let locked = bounds(&state, &owner, locked_id);
        let snapshots = state.capture_movable_selection_snapshots();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(state.hit_test_at_with(&owner, probe.0, probe.1), Some(id));
        assert!(state.has_spatial_index());
        assert!(state.translate_selection_with_undo_with(&owner, 80, 60));
        let moved = bounds(&state, &owner, id);
        assert_eq!(
            (moved.x, moved.y, moved.width, moved.height),
            (
                original.x + 80,
                original.y + 60,
                original.width,
                original.height
            )
        );
        assert_eq!(bounds(&state, &owner, locked_id), locked);
        assert_eq!(state.hit_test_at_with(&owner, probe.0, probe.1), None);
        assert_eq!(
            state.hit_test_at_with(&owner, probe.0 + 80, probe.1 + 60),
            Some(id)
        );
        let dirty = state.take_dirty_regions();
        assert_dirty_covers(&dirty, original);
        assert_dirty_covers(&dirty, moved);
        state.restore_selection_from_snapshots_with(&owner, snapshots);
        assert_eq!(bounds(&state, &owner, id), original);
        assert_eq!(state.hit_test_at_with(&owner, probe.0, probe.1), Some(id));
        let dirty = state.take_dirty_regions();
        assert_dirty_covers(&dirty, moved);
        assert_dirty_covers(&dirty, original);
    }
}

#[test]
fn explicit_resize_and_restore_use_previous_live_damage() {
    for (shape, _) in fixtures() {
        let moves_content_only = matches!(
            shape,
            Shape::Text { .. } | Shape::StickyNote { .. } | Shape::StepMarker { .. }
        );
        let owner = TextMeasurer::default();
        let (mut state, id, locked_id) = state_with(shape);
        state.set_selection(vec![id]);
        let original = state.selection_bounds_with(&owner).unwrap();
        let locked = bounds(&state, &owner, locked_id);
        let snapshots = state.capture_resize_selection_snapshots();
        state.ensure_spatial_index_for_active_frame_with(&owner);
        state.apply_selection_resize_with(
            &owner,
            SelectionHandle::BottomRight,
            &original,
            100,
            60,
            &snapshots,
        );
        let first_resize = bounds(&state, &owner, id);
        assert_ne!(first_resize, original);
        if moves_content_only {
            assert_eq!(
                (first_resize.width, first_resize.height),
                (original.width, original.height)
            );
            assert_ne!((first_resize.x, first_resize.y), (original.x, original.y));
        } else {
            assert!(first_resize.width > original.width);
        }
        state.take_dirty_regions();
        state.apply_selection_resize_with(
            &owner,
            SelectionHandle::BottomRight,
            &original,
            20,
            10,
            &snapshots,
        );
        let second_resize = bounds(&state, &owner, id);
        assert_ne!(second_resize, first_resize);
        if moves_content_only {
            assert_eq!(
                (second_resize.width, second_resize.height),
                (original.width, original.height)
            );
            assert_ne!(
                (second_resize.x, second_resize.y),
                (first_resize.x, first_resize.y)
            );
        } else {
            assert!(second_resize.width < first_resize.width);
        }
        let dirty = state.take_dirty_regions();
        assert_dirty_covers(&dirty, first_resize);
        assert_dirty_covers(&dirty, second_resize);
        state.restore_resize_from_snapshots_with(&owner, &snapshots);
        assert_eq!(bounds(&state, &owner, id), original);
        assert_eq!(bounds(&state, &owner, locked_id), locked);
        let dirty = state.take_dirty_regions();
        assert_dirty_covers(&dirty, second_resize);
        assert_dirty_covers(&dirty, original);
    }
}

#[test]
fn explicit_text_wrap_handles_and_screen_bounds_follow_live_geometry() {
    for (shape, _) in fixtures().into_iter().take(2) {
        let owner = TextMeasurer::default();
        let (mut state, id, locked_id) = state_with(shape);
        state.set_selection(vec![id]);
        let before = bounds(&state, &owner, id);
        let (_, handle) = state.selected_text_resize_handle_with(&owner).unwrap();
        assert_eq!(
            state.hit_text_resize_handle_with(
                &owner,
                handle.x + handle.width / 2,
                handle.y + handle.height / 2
            ),
            Some(id)
        );
        state.ensure_spatial_index_for_active_frame_with(&owner);
        assert!(state.update_text_wrap_width_with(&owner, id, 60));
        let after = bounds(&state, &owner, id);
        assert!(after.height > before.height);
        assert!(!state.update_text_wrap_width_with(&owner, locked_id, 60));
        let dirty = state.take_dirty_regions();
        assert_dirty_covers(&dirty, before);
        assert_dirty_covers(&dirty, after);
        assert_ne!(
            state.selected_text_resize_handle_with(&owner).unwrap().1,
            handle
        );
        assert!(state.shape_ids_in_rect_with(&owner, after).contains(&id));
        assert_eq!(
            state.selection_bounds_with(&owner),
            state.selection_bounding_box_with(&owner, &[id])
        );
        state.view.set_zoom_status(true, false, 2.0, (20.0, 10.0));
        assert_eq!(
            state.selection_screen_bounding_box_with(&owner, &[id]),
            Rect::new(
                (after.x - 20) * 2,
                (after.y - 10) * 2,
                after.width * 2,
                after.height * 2
            )
        );
        assert_eq!(state.selection_bounds(), Some(after));
    }
}

#[test]
fn explicit_lock_delete_and_sampled_erase_preserve_locked_shapes() {
    for (shape, probe) in fixtures() {
        let owner = TextMeasurer::default();
        let (mut state, id, locked_id) = state_with(shape);
        let original = bounds(&state, &owner, id);
        state.ensure_spatial_index_for_active_frame_with(&owner);
        assert!(state.set_selection_locked_with(&owner, true));
        assert!(!state.delete_selection_with(&owner));
        assert_eq!(state.boards.active_frame().shapes.len(), 2);
        state.set_selection(vec![id]);
        assert!(state.set_selection_locked_with(&owner, false));
        state.take_dirty_regions();
        // A sparse path crosses the decorated hit region; sampling and both
        // hit/deletion stages must use the supplied owner.
        assert!(state.erase_strokes_by_points_with(
            &owner,
            &[(probe.0 - 80, probe.1), (probe.0 + 80, probe.1)]
        ));
        assert!(state.boards.active_frame().shape(id).is_none());
        assert!(state.boards.active_frame().shape(locked_id).is_some());
        assert_eq!(state.hit_test_at_with(&owner, probe.0, probe.1), None);
        assert_dirty_covers(&state.take_dirty_regions(), original);
    }
}

#[test]
fn explicit_arrow_and_spotlight_drag_chains_refresh_geometry_and_index() {
    use crate::input::DrawingState;
    let owner = TextMeasurer::default();
    let (mut arrow, _) = fixtures().pop().unwrap();
    if let Shape::Arrow { style, .. } = &mut arrow {
        *style = ArrowStyle::Curved;
    }
    let (mut state, id, _) = state_with(arrow);
    state.set_selection(vec![id]);
    let before = bounds(&state, &owner, id);
    state.ensure_spatial_index_for_active_frame_with(&owner);
    let generation = state.canvas_content_generation();
    state.state = DrawingState::BendingArrow {
        shape_id: id,
        snapshot: state.shape_snapshot(id).unwrap(),
    };
    assert!(state.drag_arrow_bend_to_with(&owner, 150, 150, false));
    let after = bounds(&state, &owner, id);
    assert_ne!(before, after);
    assert!(state.canvas_content_generation() > generation);
    let dirty = state.take_dirty_regions();
    assert_dirty_covers(&dirty, before);
    assert_dirty_covers(&dirty, after);

    let spotlight = Shape::Spotlight {
        cx: 150,
        cy: 200,
        rx: 60,
        ry: 40,
        magnification: 1.0,
    };
    let (mut state, id, _) = state_with(spotlight);
    state.set_selection(vec![id]);
    let control = state.selected_spotlight_control_with(&owner).unwrap();
    state.state = DrawingState::AdjustingSpotlightMagnification {
        shape_id: id,
        snapshot: state.shape_snapshot(id).unwrap(),
    };
    let generation = state.canvas_content_generation();
    let track = control.track.track;
    assert!(
        state
            .hit_spotlight_magnification_track_with(
                &owner,
                track.x + track.width / 2,
                track.y + track.height / 2
            )
            .is_some()
    );
    assert!(state.drag_spotlight_magnification_to_with(&owner, track.x + track.width));
    assert!(
        state
            .selected_spotlight_control_with(&owner)
            .unwrap()
            .magnification
            > 1.0
    );
    assert!(state.canvas_content_generation() > generation);
}

mod mutations;
