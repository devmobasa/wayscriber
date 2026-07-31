use super::*;
use crate::draw::{EmbeddedImage, ShapeId};
use crate::input::{DragBinding, DragToolBindings};
use crate::util::Rect;
use std::time::{Duration, Instant};

const VIEW: Option<Rect> = Some(Rect {
    x: 0,
    y: 0,
    width: 200,
    height: 200,
});

/// 2x2 GIF, one solid frame per palette index in `frame_colors`, all frames
/// sharing `delay` (in 10 ms units).
fn test_gif(frame_colors: &[u8], delay: u16, repeat: Option<gif::Repeat>) -> Vec<u8> {
    let palette = [255, 0, 0, 0, 0, 255];
    let mut bytes = Vec::new();
    {
        let mut encoder = gif::Encoder::new(&mut bytes, 2, 2, &palette).unwrap();
        if let Some(repeat) = repeat {
            encoder.set_repeat(repeat).unwrap();
        }
        for &color in frame_colors {
            let frame = gif::Frame {
                width: 2,
                height: 2,
                buffer: vec![color; 4].into(),
                delay,
                dispose: gif::DisposalMethod::Keep,
                ..Default::default()
            };
            encoder.write_frame(&frame).unwrap();
        }
    }
    bytes
}

/// Adds a GIF large enough to host the on-canvas playback button.
fn add_large_gif_shape(state: &mut InputState, bytes: Vec<u8>) -> ShapeId {
    state.boards.active_frame_mut().add_shape(Shape::Image {
        x: 10,
        y: 10,
        w: 200,
        h: 200,
        data: EmbeddedImage {
            mime_type: "image/gif".to_string(),
            width: 2,
            height: 2,
            bytes,
        },
    })
}

fn add_gif_shape(state: &mut InputState, bytes: Vec<u8>) -> ShapeId {
    state.boards.active_frame_mut().add_shape(Shape::Image {
        x: 10,
        y: 10,
        w: 20,
        h: 20,
        data: EmbeddedImage {
            mime_type: "image/gif".to_string(),
            width: 2,
            height: 2,
            bytes,
        },
    })
}

#[test]
fn advance_creates_entries_lazily_and_steps_when_due() {
    let mut state = create_test_input_state();
    let id = add_gif_shape(
        &mut state,
        test_gif(&[0, 1], 5, Some(gif::Repeat::Infinite)),
    );
    let t0 = Instant::now();

    assert!(state.gif_frame_indices().is_empty());
    assert!(!state.advance_gif_animations(t0, VIEW, None));
    assert_eq!(state.gif_frame_indices().get(&id), Some(&0));
    assert!(
        !state.gif_frames_due(t0, VIEW),
        "fresh entry is not yet due"
    );

    let t1 = t0 + Duration::from_millis(60);
    assert!(state.gif_frames_due(t1, VIEW));
    assert!(state.advance_gif_animations(t1, VIEW, None));
    assert_eq!(state.gif_frame_indices().get(&id), Some(&1));

    let report = state.dirty_tracker.take_region_report(500, 500);
    assert!(
        report.regions.iter().any(|rect| rect.contains(15, 15)),
        "advancing a GIF must damage its display bbox; got {report:?}"
    );
}

#[test]
fn interval_floor_clamps_fast_gif_delays() {
    let mut state = create_test_input_state();
    add_gif_shape(
        &mut state,
        test_gif(&[0, 1], 5, Some(gif::Repeat::Infinite)),
    );
    let t0 = Instant::now();
    let floor = Some(Duration::from_millis(200));
    state.advance_gif_animations(t0, VIEW, floor);

    // 50 ms GIF delay clamped up to the 200 ms UI animation budget.
    assert!(!state.gif_frames_due(t0 + Duration::from_millis(120), VIEW));
    assert!(state.gif_frames_due(t0 + Duration::from_millis(210), VIEW));
}

#[test]
fn sweep_drops_entries_whose_shapes_are_gone() {
    let mut state = create_test_input_state();
    let id = add_gif_shape(
        &mut state,
        test_gif(&[0, 1], 5, Some(gif::Repeat::Infinite)),
    );
    let t0 = Instant::now();
    state.advance_gif_animations(t0, VIEW, None);
    assert_eq!(state.gif_frame_indices().get(&id), Some(&0));

    state.boards.active_frame_mut().remove_shape_by_id(id);
    state.advance_gif_animations(t0 + Duration::from_millis(60), VIEW, None);
    assert!(state.gif_frame_indices().is_empty());
    assert!(
        state
            .gif_frame_timeout(t0 + Duration::from_millis(120), VIEW)
            .is_none()
    );
}

#[test]
fn finite_loop_count_finishes_and_holds_the_last_frame() {
    let mut state = create_test_input_state();
    // Two frames, one loop: F0 -> F1 -> wrap exhausts the loop budget.
    let id = add_gif_shape(
        &mut state,
        test_gif(&[0, 1], 5, Some(gif::Repeat::Finite(1))),
    );
    let mut now = Instant::now();
    state.advance_gif_animations(now, VIEW, None);

    for _ in 0..4 {
        now += Duration::from_millis(60);
        state.advance_gif_animations(now, VIEW, None);
    }

    assert_eq!(
        state.gif_playback_running(id),
        Some(false),
        "loop budget exhausted; playback holds"
    );
    assert_eq!(
        state.gif_frame_indices().get(&id),
        Some(&1),
        "holds the last frame, not frame 0"
    );
    assert!(!state.gif_frames_due(now + Duration::from_secs(1), VIEW));
}

#[test]
fn toggle_pauses_resumes_and_restarts_finished_playback() {
    let mut state = create_test_input_state();
    let id = add_gif_shape(
        &mut state,
        test_gif(&[0, 1], 5, Some(gif::Repeat::Infinite)),
    );
    let t0 = Instant::now();
    state.advance_gif_animations(t0, VIEW, None);
    assert_eq!(state.gif_playback_running(id), Some(true));

    assert_eq!(state.toggle_gif_playback(id, t0), Some(false));
    assert!(!state.gif_frames_due(t0 + Duration::from_secs(1), VIEW));
    let t1 = t0 + Duration::from_secs(2);
    state.advance_gif_animations(t1, VIEW, None);
    assert_eq!(
        state.gif_frame_indices().get(&id),
        Some(&0),
        "paused playback does not advance"
    );

    assert_eq!(state.toggle_gif_playback(id, t1), Some(true));
    assert!(state.gif_frames_due(t1, VIEW), "resume is immediately due");
    state.advance_gif_animations(t1, VIEW, None);
    assert_eq!(state.gif_frame_indices().get(&id), Some(&1));
}

#[test]
fn offscreen_entries_freeze_without_deadlines() {
    let mut state = create_test_input_state();
    let id = add_gif_shape(
        &mut state,
        test_gif(&[0, 1], 5, Some(gif::Repeat::Infinite)),
    );
    let t0 = Instant::now();
    state.advance_gif_animations(t0, VIEW, None);

    let far_view = Some(Rect {
        x: 1000,
        y: 1000,
        width: 100,
        height: 100,
    });
    let t1 = t0 + Duration::from_millis(60);
    assert!(!state.gif_frames_due(t1, far_view));
    assert!(state.gif_frame_timeout(t1, far_view).is_none());
    state.advance_gif_animations(t1, far_view, None);
    assert_eq!(
        state.gif_frame_indices().get(&id),
        Some(&0),
        "offscreen clock is frozen"
    );

    // Back on screen: resumes from where it froze.
    assert!(state.gif_frames_due(t1, VIEW));
    state.advance_gif_animations(t1, VIEW, None);
    assert_eq!(state.gif_frame_indices().get(&id), Some(&1));
}

#[test]
fn single_frame_gif_never_gets_playback_or_disables_layer_caching() {
    let mut state = create_test_input_state();
    let id = add_gif_shape(&mut state, test_gif(&[0], 5, None));
    let now = Instant::now();
    state.advance_gif_animations(now, VIEW, None);

    assert_eq!(state.gif_playback_running(id), None);
    assert!(state.gif_frame_indices().is_empty());
    assert!(
        !state.gif_frames_due(now + Duration::from_secs(1), VIEW),
        "single-frame GIFs contribute no deadlines"
    );
    assert!(
        !state.active_frame_has_animated_gif(),
        "single-frame GIFs may use the canvas layer cache"
    );

    state.open_context_menu((0, 0), vec![id], ContextMenuKind::Shape, None);
    assert!(
        state
            .context_menu_entries()
            .iter()
            .all(|entry| !entry.label.contains("GIF")),
        "single-frame GIFs expose no playback controls"
    );
}

#[test]
fn non_gif_images_get_no_playback_entries() {
    let mut state = create_test_input_state();
    state.boards.active_frame_mut().add_shape(Shape::Image {
        x: 0,
        y: 0,
        w: 4,
        h: 4,
        data: EmbeddedImage {
            mime_type: "image/png".to_string(),
            width: 1,
            height: 1,
            bytes: vec![1, 2, 3, 4],
        },
    });
    let now = Instant::now();
    state.advance_gif_animations(now, VIEW, None);
    assert!(state.gif_frame_indices().is_empty());
    assert!(state.gif_frame_timeout(now, VIEW).is_none());
}

#[test]
fn resuming_a_finished_finite_loop_gif_replays_from_frame_zero() {
    let mut state = create_test_input_state();
    // NETSCAPE Finite(1) = one repeat after the first playthrough: two plays.
    let id = add_gif_shape(
        &mut state,
        test_gif(&[0, 1], 5, Some(gif::Repeat::Finite(1))),
    );
    let mut now = Instant::now();
    state.advance_gif_animations(now, VIEW, None);
    for _ in 0..6 {
        now += Duration::from_millis(60);
        state.advance_gif_animations(now, VIEW, None);
    }
    assert_eq!(state.gif_playback_running(id), Some(false));
    assert_eq!(state.gif_frame_indices().get(&id), Some(&1));

    // A later render observes the current animation budget even though the
    // finished clock itself has nothing to advance.
    let floor = Some(Duration::from_millis(200));
    state.advance_gif_animations(now, VIEW, floor);

    // Play rewinds to frame 0 and holds it for its own delay: renders always
    // advance before painting, so an immediate deadline would skip frame 0.
    // Its 50 ms raw delay must also retain the configured 200 ms floor.
    assert_eq!(state.toggle_gif_playback(id, now), Some(true));
    assert_eq!(state.gif_frame_indices().get(&id), Some(&0));
    assert!(
        !state.gif_frames_due(now, VIEW),
        "frame 0 must survive the render that follows the restart"
    );
    assert!(!state.gif_frames_due(now + Duration::from_millis(120), VIEW));
    assert!(state.gif_frames_due(now + Duration::from_millis(210), VIEW));
    state.advance_gif_animations(now, VIEW, floor);
    assert_eq!(state.gif_frame_indices().get(&id), Some(&0));

    // ...then the loop replays fully and finishes on the last frame again.
    let mut steps = 0;
    while state.gif_playback_running(id) == Some(true) && steps < 10 {
        now += Duration::from_millis(210);
        state.advance_gif_animations(now, VIEW, floor);
        steps += 1;
    }
    assert_eq!(state.gif_playback_running(id), Some(false));
    assert_eq!(state.gif_frame_indices().get(&id), Some(&1));
    assert!(
        steps >= 4,
        "a restart replays both playthroughs, got {steps} steps"
    );
}

#[test]
fn page_switch_never_leaks_playback_state_to_matching_shape_ids() {
    let mut state = create_test_input_state();
    let first_page_id = add_gif_shape(
        &mut state,
        test_gif(&[0, 1], 5, Some(gif::Repeat::Infinite)),
    );
    let mut now = Instant::now();
    state.advance_gif_animations(now, VIEW, None);
    now += Duration::from_millis(60);
    state.advance_gif_animations(now, VIEW, None);
    assert_eq!(state.gif_frame_indices().get(&first_page_id), Some(&1));
    state.toggle_gif_playback(first_page_id, now);
    assert_eq!(state.gif_playback_running(first_page_id), Some(false));

    // A fresh page assigns the same frame-local id to an unrelated GIF.
    state.boards.new_page();
    let second_page_id = add_gif_shape(
        &mut state,
        test_gif(&[1, 0], 5, Some(gif::Repeat::Infinite)),
    );
    assert_eq!(
        first_page_id, second_page_id,
        "ids restart per frame; this test needs the collision"
    );

    now += Duration::from_millis(60);
    state.advance_gif_animations(now, VIEW, None);
    assert_eq!(
        state.gif_playback_running(second_page_id),
        Some(true),
        "the new page's GIF must not inherit the old page's paused entry"
    );
    assert_eq!(
        state.gif_frame_indices().get(&second_page_id),
        Some(&0),
        "playback starts at frame 0, not the old page's frame"
    );
}

#[test]
fn board_deletion_never_leaks_playback_state_to_the_replacement_board() {
    let mut state = create_test_input_state();
    state.switch_board(crate::input::BOARD_ID_BLACKBOARD);
    let deleted_board_gif = add_gif_shape(
        &mut state,
        test_gif(&[0, 1], 5, Some(gif::Repeat::Infinite)),
    );
    let mut now = Instant::now();
    state.advance_gif_animations(now, VIEW, None);
    now += Duration::from_millis(60);
    state.advance_gif_animations(now, VIEW, None);
    state.toggle_gif_playback(deleted_board_gif, now);
    assert_eq!(state.gif_playback_running(deleted_board_gif), Some(false));

    // Deleting the active board slides another board into its place; board
    // indices are reused, so only the board *id* in the frame key protects us.
    state.delete_active_board();
    state.delete_active_board();
    assert_ne!(state.board_id(), crate::input::BOARD_ID_BLACKBOARD);

    let replacement_gif = add_gif_shape(
        &mut state,
        test_gif(&[1, 0], 5, Some(gif::Repeat::Infinite)),
    );
    assert_eq!(
        deleted_board_gif, replacement_gif,
        "ids restart per frame; this test needs the collision"
    );

    now += Duration::from_millis(60);
    state.advance_gif_animations(now, VIEW, None);
    assert_eq!(
        state.gif_playback_running(replacement_gif),
        Some(true),
        "the replacement board's GIF must not inherit the deleted board's paused entry"
    );
    assert_eq!(state.gif_frame_indices().get(&replacement_gif), Some(&0));
}

#[test]
fn an_over_budget_gif_never_animates_at_all() {
    let mut state = create_test_input_state();
    // One frame past the cap: the paste-time verdict classifies this static,
    // so playback must honor that immediately rather than animating hundreds
    // of frames until a runtime cache limit happens to intervene.
    let colors: Vec<u8> = (0..=crate::image_decode::MAX_ANIMATION_FRAMES)
        .map(|i| (i % 2) as u8)
        .collect();
    let id = add_gif_shape(
        &mut state,
        test_gif(&colors, 5, Some(gif::Repeat::Infinite)),
    );
    let mut now = Instant::now();
    state.advance_gif_animations(now, VIEW, None);

    for _ in 0..5 {
        now += Duration::from_millis(60);
        state.advance_gif_animations(now, VIEW, None);
    }

    assert_eq!(
        state.gif_playback_running(id),
        None,
        "an over-budget GIF gets no playback clock at all"
    );
    assert!(
        state.gif_frame_indices().is_empty(),
        "no frame index means the render path uses the static first frame"
    );
    assert!(
        !state.gif_frames_due(now + Duration::from_secs(1), VIEW),
        "an over-budget GIF contributes no deadlines"
    );
    assert!(
        !state.active_frame_has_animated_gif(),
        "an over-budget GIF may use the canvas layer cache"
    );
}

#[test]
fn a_gif_at_exactly_the_frame_cap_loops_instead_of_going_static() {
    let mut state = create_test_input_state();
    let colors: Vec<u8> = (0..crate::image_decode::MAX_ANIMATION_FRAMES)
        .map(|i| (i % 2) as u8)
        .collect();
    let id = add_gif_shape(
        &mut state,
        test_gif(&colors, 5, Some(gif::Repeat::Infinite)),
    );
    let mut now = Instant::now();
    state.advance_gif_animations(now, VIEW, None);

    // Step through the entire animation and across the wrap boundary.
    for _ in 0..colors.len() + 1 {
        now += Duration::from_millis(60);
        state.advance_gif_animations(now, VIEW, None);
    }

    assert_eq!(
        state.gif_playback_running(id),
        Some(true),
        "an exactly-at-cap GIF is within budget and must keep playing"
    );
    assert_eq!(
        state.gif_frame_indices().get(&id),
        Some(&1),
        "playback wrapped through frame 0 back to frame 1"
    );
}

#[test]
fn context_menu_offers_playback_toggle_for_animated_gifs_only() {
    let mut state = create_test_input_state();
    let gif_id = add_gif_shape(
        &mut state,
        test_gif(&[0, 1], 5, Some(gif::Repeat::Infinite)),
    );
    let png_id = state.boards.active_frame_mut().add_shape(Shape::Image {
        x: 50,
        y: 50,
        w: 4,
        h: 4,
        data: EmbeddedImage {
            mime_type: "image/png".to_string(),
            width: 1,
            height: 1,
            bytes: vec![1, 2, 3, 4],
        },
    });
    let now = Instant::now();
    state.advance_gif_animations(now, VIEW, None);

    let labels = |state: &mut InputState, id: ShapeId| -> Vec<String> {
        state.open_context_menu((0, 0), vec![id], ContextMenuKind::Shape, None);
        let labels = state
            .context_menu_entries()
            .iter()
            .map(|entry| entry.label.clone())
            .collect();
        state.close_context_menu();
        labels
    };
    assert!(labels(&mut state, gif_id).iter().any(|l| l == "Pause GIF"));
    assert!(
        !labels(&mut state, png_id).iter().any(|l| l.contains("GIF")),
        "static images get no playback entry"
    );

    state.set_selection(vec![gif_id]);
    let _ = state.dirty_tracker.take_region_report(2000, 2000);
    state.execute_menu_command(MenuCommand::ToggleGifPlayback);
    assert_eq!(state.gif_playback_running(gif_id), Some(false));
    let report = state.dirty_tracker.take_region_report(2000, 2000);
    assert!(
        report.regions.iter().any(|rect| rect.contains(15, 15)),
        "context-menu playback changes must damage the GIF bbox; got {report:?}"
    );
    assert!(labels(&mut state, gif_id).iter().any(|l| l == "Play GIF"));
}

#[test]
fn animated_gif_presence_check_tracks_content_changes() {
    let mut state = create_test_input_state();
    assert!(!state.active_frame_has_animated_gif());

    let id = add_gif_shape(&mut state, test_gif(&[0, 1], 5, None));
    state.invalidate_hit_cache();
    assert!(state.active_frame_has_animated_gif());

    state.boards.active_frame_mut().remove_shape_by_id(id);
    state.invalidate_hit_cache();
    assert!(!state.active_frame_has_animated_gif());
}

/// The button sits inside the shape, on pixels that would otherwise start a
/// move drag, so pressing it must toggle playback and consume the press.
#[test]
fn pressing_the_on_canvas_button_toggles_playback_without_dragging() {
    let mut state = create_test_input_state();
    let id = add_large_gif_shape(
        &mut state,
        test_gif(&[0, 1], 5, Some(gif::Repeat::Infinite)),
    );
    state.advance_gif_animations(Instant::now(), VIEW, None);
    state.set_selection(vec![id]);

    let bounds = state.selection_bounds().expect("selection bounds");
    let button = crate::draw::gif_playback_button_rect(&bounds).expect("button fits");
    let (cx, cy) = (button.x + button.width / 2, button.y + button.height / 2);
    assert_eq!(state.hit_gif_playback_button(cx, cy), Some(id));

    state.on_mouse_press(MouseButton::Left, cx, cy);
    assert_eq!(
        state.gif_playback_running(id),
        Some(false),
        "the press pauses playback"
    );
    assert!(
        matches!(state.state, DrawingState::Idle),
        "the press must not begin a move or resize; got {:?}",
        state.state
    );

    state.on_mouse_release(MouseButton::Left, cx, cy);
    state.on_mouse_press(MouseButton::Left, cx, cy);
    assert_eq!(
        state.gif_playback_running(id),
        Some(true),
        "a second press resumes playback"
    );
}

#[test]
fn non_primary_drag_binding_does_not_activate_the_on_canvas_button() {
    let mut state = create_test_input_state();
    let id = add_large_gif_shape(
        &mut state,
        test_gif(&[0, 1], 5, Some(gif::Repeat::Infinite)),
    );
    state.advance_gif_animations(Instant::now(), VIEW, None);
    state.set_selection(vec![id]);
    let mut bindings = DragToolBindings::default();
    bindings.right.drag = DragBinding::from_tool(Tool::Line);
    assert!(state.set_drag_tool_bindings(bindings));

    let bounds = state.selection_bounds().expect("selection bounds");
    let button = crate::draw::gif_playback_button_rect(&bounds).expect("button fits");
    let (cx, cy) = (button.x + button.width / 2, button.y + button.height / 2);
    state.on_mouse_press(MouseButton::Right, cx, cy);

    assert_eq!(
        state.gif_playback_running(id),
        Some(true),
        "only the primary button may toggle the playback control"
    );
    assert!(
        matches!(
            state.state,
            DrawingState::Drawing {
                tool: Tool::Line,
                ..
            }
        ),
        "the configured right-button drag must retain its tool behavior; got {:?}",
        state.state
    );
}

#[test]
fn the_on_canvas_button_appears_only_for_a_single_animated_gif() {
    let mut state = create_test_input_state();
    let gif = add_large_gif_shape(
        &mut state,
        test_gif(&[0, 1], 5, Some(gif::Repeat::Infinite)),
    );
    let small_gif = add_gif_shape(
        &mut state,
        test_gif(&[1, 0], 5, Some(gif::Repeat::Infinite)),
    );
    let png = state.boards.active_frame_mut().add_shape(Shape::Image {
        x: 400,
        y: 400,
        w: 200,
        h: 200,
        data: EmbeddedImage {
            mime_type: "image/png".to_string(),
            width: 1,
            height: 1,
            bytes: vec![1, 2, 3, 4],
        },
    });
    state.advance_gif_animations(Instant::now(), VIEW, None);

    let ids = |state: &InputState| -> Vec<ShapeId> {
        state
            .gif_playback_buttons()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect()
    };

    state.set_selection(vec![gif]);
    assert_eq!(ids(&state), vec![gif]);

    state.set_selection(vec![small_gif]);
    assert!(
        ids(&state).is_empty(),
        "a GIF smaller than the button gets none"
    );

    state.set_selection(vec![png]);
    assert!(ids(&state).is_empty(), "static images get none");

    state.set_selection(vec![gif, png]);
    assert!(
        ids(&state).is_empty(),
        "a playing GIF shows its button only while solely selected"
    );
}

/// A stopped GIF looks exactly like a static image, so its resume affordance
/// has to survive losing the selection — otherwise the only way back is a
/// context-menu entry the user has no reason to look for.
#[test]
fn a_stopped_gif_keeps_its_button_after_the_selection_is_cleared() {
    let mut state = create_test_input_state();
    let id = add_large_gif_shape(
        &mut state,
        test_gif(&[0, 1], 5, Some(gif::Repeat::Infinite)),
    );
    let now = Instant::now();
    state.advance_gif_animations(now, VIEW, None);
    state.set_selection(vec![id]);

    let buttons = |state: &InputState| -> Vec<(ShapeId, bool)> {
        state
            .gif_playback_buttons()
            .into_iter()
            .map(|(id, _, playing)| (id, playing))
            .collect()
    };
    assert_eq!(buttons(&state), vec![(id, true)], "playing, selected");

    state.clear_selection();
    assert!(
        buttons(&state).is_empty(),
        "a playing GIF keeps the canvas clean once deselected"
    );

    // Pause it, then walk away from the selection.
    state.set_selection(vec![id]);
    state.toggle_gif_playback(id, now);
    state.clear_selection();
    assert_eq!(
        buttons(&state),
        vec![(id, false)],
        "a paused GIF keeps its play button with nothing selected"
    );

    // And it remains clickable without re-selecting first.
    let bounds = state
        .boards
        .active_frame()
        .shape(id)
        .and_then(|shape| shape.bounding_box())
        .expect("bounds");
    let rect = crate::draw::gif_playback_button_rect(&bounds).expect("button");
    let (cx, cy) = (rect.x + rect.width / 2, rect.y + rect.height / 2);
    assert_eq!(state.hit_gif_playback_button(cx, cy), Some(id));
    state.on_mouse_press(MouseButton::Left, cx, cy);
    assert_eq!(state.gif_playback_running(id), Some(true));
}

/// A GIF that runs out of loops is stopped for the same reason a paused one
/// is, so it earns the same affordance and the same repaint.
#[test]
fn a_finished_gif_shows_its_button_and_damages_itself() {
    let mut state = create_test_input_state();
    let id = add_large_gif_shape(
        &mut state,
        test_gif(&[0, 1], 5, Some(gif::Repeat::Finite(1))),
    );
    let mut now = Instant::now();
    state.advance_gif_animations(now, VIEW, None);
    state.clear_selection();
    let _ = state.dirty_tracker.take_region_report(2000, 2000);

    while state.gif_playback_running(id) == Some(true) {
        now += Duration::from_millis(60);
        state.advance_gif_animations(now, VIEW, None);
    }

    let report = state.dirty_tracker.take_region_report(2000, 2000);
    assert!(
        report.regions.iter().any(|rect| rect.contains(100, 100)),
        "finishing must repaint the GIF so its play button appears; got {report:?}"
    );
    assert_eq!(
        state
            .gif_playback_buttons()
            .into_iter()
            .map(|(id, _, playing)| (id, playing))
            .collect::<Vec<_>>(),
        vec![(id, false)]
    );
}
