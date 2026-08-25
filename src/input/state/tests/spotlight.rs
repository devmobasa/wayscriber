//! The spotlight tool: drag geometry, region collection, and damage behavior.

use super::*;
use crate::input::state::{SpotlightWheelClaim, SpotlightWheelOutcome};

fn only_shape(state: &InputState) -> &Shape {
    &state.boards.active_frame().shapes[0].shape
}

#[test]
fn dragging_the_spotlight_tool_commits_an_elliptical_region() {
    let mut state = create_test_input_state();
    state.spotlight_magnification = 2.25;
    state.set_tool_override(Some(Tool::Spotlight));
    assert!(
        !state.take_pending_frozen_toggle(),
        "selecting Spotlight must not capture the screen automatically"
    );

    state.on_mouse_press(MouseButton::Left, 100, 100);
    state.on_mouse_motion(200, 160);
    state.on_mouse_release(MouseButton::Left, 200, 160);
    assert!(state.take_pending_spotlight_magnifier_feedback());
    assert!(
        !state.take_pending_frozen_toggle(),
        "committing a magnified Spotlight must not capture automatically"
    );

    match only_shape(&state) {
        Shape::Spotlight {
            cx,
            cy,
            rx,
            ry,
            magnification,
        } => {
            assert_eq!((*cx, *cy), (150, 130), "centre is the drag box centre");
            assert_eq!((*rx, *ry), (50, 30));
            assert_eq!(*magnification, 2.25);
        }
        other => panic!("expected a spotlight, got {other:?}"),
    }
    assert_eq!(
        state.spotlight_frame_regions(Some((0, 0))).regions[0].magnification,
        2.25
    );
}

#[test]
fn committed_spotlights_are_collected_for_the_render_pass() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Spotlight));

    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_release(MouseButton::Left, 100, 80);
    state.clear_selection();
    state.on_mouse_press(MouseButton::Left, 300, 300);
    state.on_mouse_release(MouseButton::Left, 400, 380);

    let regions = state.spotlight_frame_regions(Some((0, 0))).regions;
    assert_eq!(regions.len(), 2, "both spotlights must reach the pass");
    assert!(regions.iter().any(|r| (r.cx - 50.0).abs() < 0.5));
    assert!(regions.iter().any(|r| (r.cx - 350.0).abs() < 0.5));
}

#[test]
fn the_in_progress_drag_is_included_so_dimming_follows_the_cursor() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Spotlight));

    state.on_mouse_press(MouseButton::Left, 40, 40);
    state.on_mouse_motion(140, 120);

    let regions = state.spotlight_frame_regions(Some((140, 120))).regions;
    assert_eq!(regions.len(), 1, "the live drag should already dim");
    assert!((regions[0].cx - 90.0).abs() < 0.5);
    assert!((regions[0].rx - 50.0).abs() < 0.5);
}

#[test]
fn an_in_progress_drag_dims_but_does_not_count_as_something_the_page_holds() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Spotlight));
    assert!(state.set_spotlight_magnification(2.5));

    state.on_mouse_press(MouseButton::Left, 40, 40);
    state.on_mouse_motion(140, 120);

    // The drag dims immediately, but nothing is committed yet: a warning that
    // describes the page must not fire for an ellipse that cancelling erases.
    let collected = state.spotlight_frame_regions(Some((140, 120)));
    assert_eq!(
        collected.regions.len(),
        1,
        "the live drag should already dim"
    );
    assert!(
        !collected.committed_magnified,
        "a drag under the pointer is not yet something the page holds"
    );

    state.on_mouse_release(MouseButton::Left, 140, 120);
    let collected = state.spotlight_frame_regions(None);
    assert_eq!(collected.regions.len(), 1);
    assert!(
        collected.committed_magnified,
        "once committed, the page does hold a magnified Spotlight"
    );
}

#[test]
fn suppressing_transients_collects_committed_regions_only() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Spotlight));

    state.on_mouse_press(MouseButton::Left, 40, 40);
    state.on_mouse_motion(140, 120);

    assert!(
        state.spotlight_frame_regions(None).regions.is_empty(),
        "a frame that shows no transients draws no in-progress drag"
    );
}

#[test]
fn a_live_drag_of_another_tool_contributes_no_region() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Rect));

    state.on_mouse_press(MouseButton::Left, 40, 40);
    state.on_mouse_motion(140, 120);

    assert!(
        state
            .spotlight_frame_regions(Some((140, 120)))
            .regions
            .is_empty()
    );
    assert!(!state.has_spotlight());
}

#[test]
fn has_spotlight_reports_both_committed_and_in_progress_regions() {
    let mut state = create_test_input_state();
    assert!(!state.has_spotlight(), "empty page dims nothing");

    state.set_tool_override(Some(Tool::Spotlight));
    state.on_mouse_press(MouseButton::Left, 10, 10);
    assert!(
        state.has_spotlight(),
        "a drag in flight already dims the screen"
    );

    state.on_mouse_release(MouseButton::Left, 90, 70);
    assert!(
        state.has_spotlight(),
        "the committed spotlight keeps dimming"
    );
}

#[test]
fn deleting_the_spotlight_stops_the_dimming() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Spotlight));
    state.on_mouse_press(MouseButton::Left, 10, 10);
    state.on_mouse_release(MouseButton::Left, 90, 70);
    assert!(state.has_spotlight());

    state.boards.active_frame_mut().shapes.clear();
    assert!(!state.has_spotlight());
    assert!(
        state
            .spotlight_frame_regions(Some((0, 0)))
            .regions
            .is_empty()
    );
}

#[test]
fn a_spotlight_is_selectable_anywhere_inside_its_opening() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Spotlight));
    state.on_mouse_press(MouseButton::Left, 100, 100);
    state.on_mouse_release(MouseButton::Left, 200, 200);
    state.clear_selection();

    // The shape paints nothing, so the whole opening has to be clickable.
    let id = state.boards.active_frame().shapes[0].id;
    assert_eq!(
        state.hit_test_at(150, 150),
        Some(id),
        "the centre of the opening should select the spotlight"
    );
    assert_eq!(
        state.hit_test_at(400, 400),
        None,
        "a point outside the opening should not select it"
    );
}

#[test]
fn spotlight_exposes_a_readable_kind_name() {
    let shape = Shape::Spotlight {
        cx: 0,
        cy: 0,
        rx: 10,
        ry: 10,
        magnification: crate::draw::DEFAULT_SPOTLIGHT_MAGNIFICATION,
    };
    assert_eq!(shape.kind_name(), "Spotlight");
}

#[test]
fn spotlight_bounds_cover_its_opening() {
    let shape = Shape::Spotlight {
        cx: 100,
        cy: 100,
        rx: 40,
        ry: 20,
        magnification: crate::draw::DEFAULT_SPOTLIGHT_MAGNIFICATION,
    };
    let bounds = shape.bounding_box().expect("spotlight has area");
    assert!(bounds.x <= 60 && bounds.y <= 80);
    assert!(bounds.x + bounds.width >= 140);
    assert!(bounds.y + bounds.height >= 120);
}

fn spotlight_state_with_one_loupe(magnification: f64) -> (InputState, crate::draw::ShapeId) {
    let mut state = create_test_input_state();
    // A real viewport, so the on-canvas control is placed under the same
    // clamping rules it gets on screen.
    state.screen_width = 1920;
    state.screen_height = 1080;
    let id = state.boards.active_frame_mut().add_shape(Shape::Spotlight {
        cx: 200,
        cy: 200,
        rx: 60,
        ry: 40,
        magnification,
    });
    (state, id)
}

fn magnification_of(state: &InputState, id: crate::draw::ShapeId) -> f64 {
    match state.boards.active_frame().shape(id).expect("loupe").shape {
        Shape::Spotlight { magnification, .. } => magnification,
        ref other => panic!("expected a spotlight, got {other:?}"),
    }
}

/// Every control that edits this property must leave the shape on the 0.25
/// grid, or the toolbar would show a value the shape does not hold.
fn assert_on_the_step_grid(value: f64) {
    let steps = (value - crate::draw::MIN_SPOTLIGHT_MAGNIFICATION)
        / crate::draw::SPOTLIGHT_MAGNIFICATION_STEP;
    assert!(
        (steps - steps.round()).abs() < 1e-9,
        "{value} is not a whole number of 0.25 steps above 1x"
    );
}

#[test]
fn every_pixel_of_the_track_snaps_to_the_same_grid_the_toolbar_uses() {
    let (mut state, id) = spotlight_state_with_one_loupe(1.0);
    state.set_selection(vec![id]);
    let track = state
        .selected_spotlight_control()
        .expect("control")
        .track
        .track;

    // Sweeping the whole track must never land between steps: a continuous
    // drag used to produce values like 2.19x that no other control could show.
    for offset in 0..=track.width {
        let value = state
            .selected_spotlight_control()
            .expect("control")
            .track
            .magnification_at(track.x + offset);
        assert_on_the_step_grid(value);
    }
}

#[test]
fn a_wheel_tick_pulls_an_off_grid_loupe_back_onto_the_grid() {
    // A factor from an older session or a hand-edited file. One tick should
    // land on a real step rather than carrying the offset forever.
    let (mut state, id) = spotlight_state_with_one_loupe(2.19);

    assert_eq!(
        state.nudge_spotlight_magnification_at(200, 200, 1),
        SpotlightWheelOutcome::Adjusted
    );
    let value = magnification_of(&state, id);
    assert_on_the_step_grid(value);
    assert_eq!(value, 2.5);
}

#[test]
fn the_wheel_over_a_loupe_steps_its_own_magnification() {
    let (mut state, id) = spotlight_state_with_one_loupe(2.0);

    // Inside the ellipse: the wheel claims the event and the shape follows.
    assert_eq!(
        state.nudge_spotlight_magnification_at(200, 200, 1),
        SpotlightWheelOutcome::Adjusted
    );
    assert_eq!(magnification_of(&state, id), 2.25);

    // Outside it: the wheel keeps its usual meaning for the caller.
    assert_eq!(
        state.nudge_spotlight_magnification_at(400, 400, 1),
        SpotlightWheelOutcome::NotOverLoupe
    );
    assert_eq!(magnification_of(&state, id), 2.25);
}

#[test]
fn a_wheel_burst_over_one_loupe_undoes_as_a_single_step() {
    let (mut state, id) = spotlight_state_with_one_loupe(2.0);

    for _ in 0..4 {
        assert_eq!(
            state.nudge_spotlight_magnification_at(200, 200, 1),
            SpotlightWheelOutcome::Adjusted
        );
    }
    assert_eq!(magnification_of(&state, id), 3.0);

    // Undo flushes the in-flight gesture first, so the whole burst is one
    // entry rather than four.
    state.handle_action(Action::Undo);
    assert_eq!(magnification_of(&state, id), 2.0);
}

#[test]
fn the_wheel_stops_at_the_end_of_the_range_without_opening_a_gesture() {
    let (mut state, id) = spotlight_state_with_one_loupe(crate::draw::MAX_SPOTLIGHT_MAGNIFICATION);

    // Still the loupe's event, so the wheel must not fall through and resize
    // a brush behind the user's back.
    assert_eq!(
        state.nudge_spotlight_magnification_at(200, 200, 1),
        SpotlightWheelOutcome::AtRangeEnd,
        "an end of the range is not the same as the pointer being elsewhere"
    );
    assert_eq!(
        magnification_of(&state, id),
        crate::draw::MAX_SPOTLIGHT_MAGNIFICATION
    );
    state.handle_action(Action::Undo);
    assert_eq!(
        magnification_of(&state, id),
        crate::draw::MAX_SPOTLIGHT_MAGNIFICATION,
        "a refused step must not have left an undo entry behind"
    );
}

#[test]
fn a_locked_loupe_claims_the_wheel_without_changing() {
    let (mut state, id) = spotlight_state_with_one_loupe(2.0);
    let index = state
        .boards
        .active_frame()
        .find_index(id)
        .expect("spotlight index");
    state.boards.active_frame_mut().shapes[index].locked = true;

    assert_eq!(
        state.nudge_spotlight_magnification_at(200, 200, 1),
        SpotlightWheelOutcome::Locked
    );
    assert_eq!(magnification_of(&state, id), 2.0);
}

#[test]
fn a_locked_topmost_loupe_hides_an_unlocked_loupe_from_the_wheel() {
    let (mut state, lower_id) = spotlight_state_with_one_loupe(2.0);
    let upper_id = state.boards.active_frame_mut().add_shape(Shape::Spotlight {
        cx: 200,
        cy: 200,
        rx: 60,
        ry: 40,
        magnification: 3.0,
    });
    let upper_index = state
        .boards
        .active_frame()
        .find_index(upper_id)
        .expect("topmost spotlight index");
    state.boards.active_frame_mut().shapes[upper_index].locked = true;

    assert_eq!(
        state.nudge_spotlight_magnification_at(200, 200, 1),
        SpotlightWheelOutcome::Locked
    );
    assert_eq!(magnification_of(&state, lower_id), 2.0);
    assert_eq!(magnification_of(&state, upper_id), 3.0);
}

#[test]
fn the_on_canvas_knob_only_appears_for_one_unlocked_selected_loupe() {
    let (mut state, id) = spotlight_state_with_one_loupe(2.0);
    assert!(
        state.selected_spotlight_control().is_none(),
        "nothing selected, nothing to adjust"
    );

    state.set_selection(vec![id]);
    let control = state
        .selected_spotlight_control()
        .expect("a selected loupe carries the control");
    let track = control.track;
    assert_eq!(control.shape_id, id);
    // Above the opening, clear of the bounding box the resize handles sit on.
    assert!(track.track.y + track.track.height < 160);

    let index = state
        .boards
        .active_frame()
        .find_index(id)
        .expect("shape index");
    state.boards.active_frame_mut().shapes[index].locked = true;
    assert!(
        state.selected_spotlight_control().is_none(),
        "a locked loupe is not adjustable"
    );
}

#[test]
fn the_control_only_shows_on_a_page_that_already_forces_full_damage() {
    // The control is drawn well outside the loupe's bounds, above it. It is
    // never clipped away because a page holding any Spotlight repaints in full
    // (`render_force_full_damage_reason`), and the control cannot appear
    // without one being selected. This pins the implication that guarantee
    // rests on.
    let (mut state, id) = spotlight_state_with_one_loupe(2.0);
    state.set_selection(vec![id]);

    assert!(state.selected_spotlight_control().is_some());
    assert!(
        state.has_spotlight(),
        "a visible control implies a spotlight, which implies full-frame damage"
    );

    state.clear_selection();
    assert!(state.selected_spotlight_control().is_none());
}

#[test]
fn a_wheel_gesture_never_commits_against_a_different_page() {
    // Shape ids restart per frame, so a snapshot flushed after a page change
    // would attach to an unrelated shape and corrupt that page's history.
    let (mut state, id) = spotlight_state_with_one_loupe(2.0);
    assert_eq!(
        state.nudge_spotlight_magnification_at(200, 200, 1),
        SpotlightWheelOutcome::Adjusted
    );
    assert_eq!(magnification_of(&state, id), 2.25);

    // The page switch goes through a real action, which flushes first, so the
    // entry lands on the page it belongs to.
    state.handle_action(Action::PageNew);
    assert_ne!(
        state.boards.active_page_index(),
        0,
        "the test needs an actual page switch"
    );
    state.handle_action(Action::Undo);

    // Back on the source page, the burst is still undoable there: the entry
    // was neither discarded nor written to the page the user moved to.
    state.handle_action(Action::PagePrev);
    assert_eq!(magnification_of(&state, id), 2.25);
    state.handle_action(Action::Undo);
    assert_eq!(
        magnification_of(&state, id),
        2.0,
        "the source page kept its own undo entry across the switch"
    );
}

#[test]
fn a_changed_page_set_never_looks_like_the_frame_a_gesture_started_on() {
    // Deleting a page and landing a different one on the same index must not
    // look like the page the gesture started on. Board and page generations
    // are what separate them; the index alone would alias, and the old
    // snapshot would attach to whatever now holds that id.
    let (mut state, _id) = spotlight_state_with_one_loupe(2.0);
    let started_on = crate::input::state::spotlight::FrameIdentity::of(&state.boards);

    state.boards.new_page();
    assert_ne!(
        crate::input::state::spotlight::FrameIdentity::of(&state.boards),
        started_on,
        "a different page must not compare equal to the gesture's own"
    );

    // Deliberately strict: coming back to the same index after the page set
    // changed still does not match. Every real transition flushes before it
    // happens, so this branch only runs when something slipped past that, and
    // dropping the entry is the safe answer there.
    state.boards.prev_page();
    assert_eq!(state.boards.active_page_index(), 0);
    assert_ne!(
        crate::input::state::spotlight::FrameIdentity::of(&state.boards),
        started_on,
        "a changed page set invalidates the gesture rather than risking an alias"
    );
}

#[test]
fn moving_off_a_loupe_ends_its_wheel_gesture() {
    let (mut state, id) = spotlight_state_with_one_loupe(2.0);
    assert_eq!(
        state.nudge_spotlight_magnification_at(200, 200, 1),
        SpotlightWheelOutcome::Adjusted
    );

    // Two visits separated by the pointer leaving are two undo entries, not
    // one merged burst that only unwinds completely.
    state.on_mouse_motion(600, 600);
    assert_eq!(
        state.nudge_spotlight_magnification_at(200, 200, 1),
        SpotlightWheelOutcome::Adjusted
    );
    assert_eq!(magnification_of(&state, id), 2.5);

    state.handle_action(Action::Undo);
    assert_eq!(magnification_of(&state, id), 2.25);
    state.handle_action(Action::Undo);
    assert_eq!(magnification_of(&state, id), 2.0);
}

#[test]
fn moving_off_a_loupe_discards_its_partial_wheel_step() {
    let (mut state, id) = spotlight_state_with_one_loupe(2.0);
    assert_eq!(
        state.claim_spotlight_wheel_axis_at(200, 200, -60, 0, 0.0),
        SpotlightWheelClaim::Adjustable(0)
    );

    state.on_mouse_motion(600, 600);

    assert_eq!(
        state.claim_spotlight_wheel_axis_at(200, 200, -60, 0, 0.0),
        SpotlightWheelClaim::Adjustable(0),
        "returning to the loupe starts a new logical wheel step"
    );
    assert_eq!(
        magnification_of(&state, id),
        2.0,
        "partial units from separate visits must not combine"
    );
}

#[test]
fn a_toolbar_page_switch_closes_the_wheel_gesture_too() {
    // Toolbar events never reach `handle_action`, so this is a separate path
    // to the same requirement: the burst must be recorded against the page it
    // happened on, before the switch.
    let (mut state, id) = spotlight_state_with_one_loupe(2.0);
    assert_eq!(
        state.nudge_spotlight_magnification_at(200, 200, 1),
        SpotlightWheelOutcome::Adjusted
    );

    state.apply_toolbar_event(crate::ui::toolbar::ToolbarEvent::PageNew);
    assert_ne!(state.boards.active_page_index(), 0);

    state.apply_toolbar_event(crate::ui::toolbar::ToolbarEvent::PagePrev);
    assert_eq!(magnification_of(&state, id), 2.25);
    state.handle_action(Action::Undo);
    assert_eq!(
        magnification_of(&state, id),
        2.0,
        "the toolbar switch left the source page undoable"
    );
}

#[test]
fn a_panned_board_still_places_the_control_where_the_user_can_reach_it() {
    // The clamp is in canvas coordinates, so it has to follow the pan. A loupe
    // at the top of the *visible* area needs the control flipped below it even
    // though its canvas y is far from zero.
    let (mut state, _) = spotlight_state_with_one_loupe(2.0);
    state.boards.active_frame_mut().shapes.clear();
    assert!(state.boards.active_frame_mut().set_view_offset(4000, 3000));
    state.sync_canvas_pointer_to_current_transform();

    let visible = state.visible_canvas_rect();
    let loupe = state.boards.active_frame_mut().add_shape(Shape::Spotlight {
        cx: visible.x + 200,
        cy: visible.y + 10,
        rx: 60,
        ry: 30,
        magnification: 2.0,
    });
    state.set_selection(vec![loupe]);

    let track = state
        .selected_spotlight_control()
        .expect("a panned loupe still has a reachable control")
        .track
        .track;
    assert!(
        track.y >= visible.y,
        "the control must stay inside the visible canvas, got y={} for visible y={}",
        track.y,
        visible.y
    );
    assert!(track.y + track.height <= visible.y + visible.height);
    assert!(track.x >= visible.x && track.x + track.width <= visible.x + visible.width);
}

#[test]
fn maximum_persisted_view_offsets_do_not_overflow_the_selected_control_path() {
    let mut state = create_test_input_state();
    state.switch_board(crate::input::BOARD_ID_WHITEBOARD);
    state.screen_width = 1920;
    state.screen_height = 1080;
    let id = state.boards.active_frame_mut().add_shape(Shape::Spotlight {
        cx: 200,
        cy: 200,
        rx: 60,
        ry: 40,
        magnification: 2.0,
    });
    state.set_selection(vec![id]);
    assert!(
        state
            .boards
            .active_frame_mut()
            .set_view_offset(i32::MAX, i32::MAX)
    );

    let visible = state.visible_canvas_rect();
    assert_eq!((visible.x, visible.y), (i32::MAX, i32::MAX));
    assert_eq!((visible.width, visible.height), (1, 1));
    let _ = state.selected_spotlight_control();
}

#[test]
fn an_edge_loupe_keeps_its_control_on_screen() {
    let (mut state, _) = spotlight_state_with_one_loupe(2.0);
    state.boards.active_frame_mut().shapes.clear();

    // Hard against the top-left corner: the control would sit above and to the
    // left of the screen if it were placed without clamping.
    let corner = state.boards.active_frame_mut().add_shape(Shape::Spotlight {
        cx: 10,
        cy: 10,
        rx: 40,
        ry: 30,
        magnification: 2.0,
    });
    state.set_selection(vec![corner]);
    let track = state
        .selected_spotlight_control()
        .expect("an edge loupe still has a reachable control")
        .track;
    assert!(track.track.x >= 0, "the left end must stay grabbable");
    assert!(
        track.track.y >= 0 && track.track.y + track.track.height <= 1080,
        "the track must stay on screen, got y={}",
        track.track.y
    );
    assert!(
        track.track.y > 10,
        "with no room above, the control belongs under the opening"
    );

    // Hard against the right edge: the far end of the track must not run off.
    state.boards.active_frame_mut().shapes.clear();
    let right = state.boards.active_frame_mut().add_shape(Shape::Spotlight {
        cx: 1910,
        cy: 540,
        rx: 40,
        ry: 30,
        magnification: 4.0,
    });
    state.set_selection(vec![right]);
    let track = state.selected_spotlight_control().expect("control").track;
    assert!(
        track.track.x + track.track.width <= 1920,
        "the right end must stay grabbable, got x={}",
        track.track.x
    );
    assert!(track.knob.x + track.knob.width <= 1920);
}

#[test]
fn extreme_loupe_coordinates_do_not_overflow_the_hit_test_or_the_track() {
    let mut state = create_test_input_state();
    let id = state.boards.active_frame_mut().add_shape(Shape::Spotlight {
        cx: i32::MIN + 4,
        cy: i32::MAX - 4,
        rx: i32::MAX,
        ry: i32::MAX,
        magnification: 2.0,
    });

    // Persisted coordinates near the i32 extremes must not panic in debug or
    // wrap in release; failing to place the control is the correct outcome.
    let _ = state.spotlight_at(i32::MAX, i32::MIN);
    state.set_selection(vec![id]);
    let _ = state.selected_spotlight_control();
    let _ = state.hit_spotlight_magnification_track(0, 0);
}

#[test]
fn dragging_the_knob_magnifies_live_and_commits_one_undo_entry() {
    let (mut state, id) = spotlight_state_with_one_loupe(1.0);
    state.set_selection(vec![id]);
    let track = state
        .selected_spotlight_control()
        .expect("control")
        .track
        .track;

    // Pressing anywhere on the track is itself an adjustment.
    state.on_mouse_press(MouseButton::Left, track.x + track.width / 2, track.y + 6);
    assert!(matches!(
        state.state,
        DrawingState::AdjustingSpotlightMagnification { .. }
    ));
    let midpoint = magnification_of(&state, id);
    assert!(
        midpoint > 2.0 && midpoint < 3.0,
        "the middle of the track is the middle of the range, got {midpoint}"
    );
    assert_on_the_step_grid(midpoint);

    // Dragging keeps the loupe following the pointer.
    state.on_mouse_motion(track.x + track.width, track.y + 6);
    assert_eq!(
        magnification_of(&state, id),
        crate::draw::MAX_SPOTLIGHT_MAGNIFICATION
    );

    state.on_mouse_release(MouseButton::Left, track.x + track.width, track.y + 6);
    assert!(matches!(state.state, DrawingState::Idle));

    state.handle_action(Action::Undo);
    assert_eq!(
        magnification_of(&state, id),
        1.0,
        "the whole drag undoes in one step"
    );
}

#[test]
fn cancelling_a_knob_drag_restores_the_factor_it_started_from() {
    let (mut state, id) = spotlight_state_with_one_loupe(2.0);
    state.set_selection(vec![id]);
    let track = state
        .selected_spotlight_control()
        .expect("control")
        .track
        .track;

    state.on_mouse_press(MouseButton::Left, track.x + track.width, track.y + 6);
    assert_eq!(
        magnification_of(&state, id),
        crate::draw::MAX_SPOTLIGHT_MAGNIFICATION
    );

    state.cancel_active_interaction();
    assert!(matches!(state.state, DrawingState::Idle));
    assert_eq!(magnification_of(&state, id), 2.0);
}
