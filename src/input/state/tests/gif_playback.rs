use super::*;
use crate::draw::{EmbeddedImage, ShapeId};
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
            let mut frame = gif::Frame::default();
            frame.width = 2;
            frame.height = 2;
            frame.buffer = vec![color; 4].into();
            frame.delay = delay;
            frame.dispose = gif::DisposalMethod::Keep;
            encoder.write_frame(&frame).unwrap();
        }
    }
    bytes
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
    let id = add_gif_shape(&mut state, test_gif(&[0, 1], 5, Some(gif::Repeat::Infinite)));
    let t0 = Instant::now();

    assert!(state.gif_frame_indices().is_empty());
    assert!(!state.advance_gif_animations(t0, VIEW, None));
    assert_eq!(state.gif_frame_indices().get(&id), Some(&0));
    assert!(!state.gif_frames_due(t0, VIEW), "fresh entry is not yet due");

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
    add_gif_shape(&mut state, test_gif(&[0, 1], 5, Some(gif::Repeat::Infinite)));
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
    let id = add_gif_shape(&mut state, test_gif(&[0, 1], 5, Some(gif::Repeat::Infinite)));
    let t0 = Instant::now();
    state.advance_gif_animations(t0, VIEW, None);
    assert_eq!(state.gif_frame_indices().get(&id), Some(&0));

    state.boards.active_frame_mut().remove_shape_by_id(id);
    state.advance_gif_animations(t0 + Duration::from_millis(60), VIEW, None);
    assert!(state.gif_frame_indices().is_empty());
    assert!(state.gif_frame_timeout(t0 + Duration::from_millis(120), VIEW).is_none());
}

#[test]
fn finite_loop_count_finishes_and_holds_the_last_frame() {
    let mut state = create_test_input_state();
    // Two frames, one loop: F0 -> F1 -> wrap exhausts the loop budget.
    let id = add_gif_shape(&mut state, test_gif(&[0, 1], 5, Some(gif::Repeat::Finite(1))));
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
    let id = add_gif_shape(&mut state, test_gif(&[0, 1], 5, Some(gif::Repeat::Infinite)));
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
    let id = add_gif_shape(&mut state, test_gif(&[0, 1], 5, Some(gif::Repeat::Infinite)));
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
fn single_frame_gif_settles_as_finished_static() {
    let mut state = create_test_input_state();
    let id = add_gif_shape(&mut state, test_gif(&[0], 5, None));
    let mut now = Instant::now();
    state.advance_gif_animations(now, VIEW, None);
    now += Duration::from_millis(60);
    state.advance_gif_animations(now, VIEW, None);

    assert_eq!(state.gif_playback_running(id), Some(false));
    assert!(
        !state.gif_frames_due(now + Duration::from_secs(1), VIEW),
        "static GIFs stop contributing deadlines"
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
fn presence_check_tracks_content_changes() {
    let mut state = create_test_input_state();
    assert!(!state.active_frame_has_gif_image());

    let id = add_gif_shape(&mut state, test_gif(&[0, 1], 5, None));
    state.invalidate_hit_cache();
    assert!(state.active_frame_has_gif_image());

    state.boards.active_frame_mut().remove_shape_by_id(id);
    state.invalidate_hit_cache();
    assert!(!state.active_frame_has_gif_image());
}
