use super::*;
use crate::backend::wayland::handlers::test_support::HandlerFixture;
use crate::input::DrawingState;
use smithay_client_toolkit::seat::pointer::BTN_LEFT;
use wayland_client::{Proxy, protocol::wl_pointer::WlPointer};

#[test]
fn unresolved_dirty_source_has_one_backoff_instead_of_an_idle_redraw_loop() {
    use crate::backend::wayland::session::SessionState;
    let temp = crate::test_temp::tempdir().unwrap();
    let mut options =
        crate::session::SessionOptions::new(temp.path().to_path_buf(), "failed-launch");
    options.persist_transparent = true;
    options.set_named_file_target(temp.path().join("failed.wayscriber-session"));
    std::fs::write(options.session_file_path(), b"unreadable").unwrap();
    std::fs::write(
        crate::session::append_path_suffix(
            &options.session_file_path(),
            ".corrupt-18446744073709551615",
        ),
        b"occupied",
    )
    .unwrap();
    let mut fixture = HandlerFixture::new(crate::config::Config::default());
    fixture.state.session = SessionState::new(Some(options));
    fixture
        .state
        .begin_session_output_transition(None, "failed first load");
    assert!(!fixture.state.session.is_loaded());
    fixture
        .state
        .input_state
        .on_mouse_press(crate::input::MouseButton::Left, 600, 400);
    fixture.state.input_state.on_mouse_motion(620, 420);
    fixture
        .state
        .input_state
        .on_mouse_release(crate::input::MouseButton::Left, 620, 420);
    fixture
        .state
        .session
        .cancel_output_transition_for_live_source();
    fixture.state.input_state.needs_redraw = false;
    for _ in 0..20 {
        assert!(
            !fixture
                .state
                .reconcile_live_source_interaction_if_idle("idle tick")
        );
        assert!(!fixture.state.input_state.needs_redraw);
    }
    assert!(
        fixture
            .state
            .session
            .pending_output_transition()
            .unwrap()
            .retry_at
            > std::time::Instant::now()
    );
}

fn draw_line(fixture: &mut HandlerFixture) -> Vec<crate::draw::Shape> {
    fixture
        .state
        .input_state
        .on_mouse_press(crate::input::MouseButton::Left, 600, 400);
    fixture.state.input_state.on_mouse_motion(620, 420);
    fixture
        .state
        .input_state
        .on_mouse_release(crate::input::MouseButton::Left, 620, 420);
    let shapes = fixture
        .state
        .input_state
        .boards
        .active_frame()
        .shapes
        .iter()
        .map(|shape| shape.shape.clone())
        .collect::<Vec<_>>();
    assert!(!shapes.is_empty());
    shapes
}

#[test]
fn leaving_the_per_output_placeholder_keeps_early_ink_and_finishes_before_input() {
    use crate::backend::wayland::session::SessionState;
    let temp = crate::test_temp::tempdir().unwrap();
    let mut placeholder = crate::session::SessionOptions::new(temp.path().to_path_buf(), "startup");
    placeholder.persist_transparent = true;
    placeholder.per_output = true;
    let mut fixture = HandlerFixture::new(crate::config::Config::default());
    fixture.state.session = SessionState::new(Some(placeholder.clone()));
    fixture
        .state
        .begin_session_output_transition(None, "initial no-output");
    assert!(fixture.state.session.is_loaded());
    let early = draw_line(&mut fixture);

    fixture
        .state
        .begin_session_output_transition(Some("monitor-a".to_string()), "first monitor");

    // The switch ran to completion inside the callback: nothing is pending
    // that further input could abort, and the output's own session is active.
    assert!(fixture.state.session_transaction.is_none());
    let active = fixture.state.session_options().unwrap().clone();
    assert_eq!(active.output_identity(), Some("monitor_a"));
    assert!(!fixture.state.session.is_dirty());
    // The early ink went to the placeholder's own file, as on main, rather
    // than being held unsaveable on screen.
    let saved = crate::session::load_snapshot(&placeholder)
        .unwrap()
        .expect("early ink saved");
    let points = |shape: &crate::draw::Shape| match shape {
        crate::draw::Shape::Freehand { points, .. } => points.clone(),
        other => panic!("expected the early stroke, got {other:?}"),
    };
    let saved_points = saved.boards[0].pages.pages[0]
        .shapes
        .iter()
        .map(|shape| points(&shape.shape))
        .collect::<Vec<_>>();
    assert_eq!(saved_points, early.iter().map(points).collect::<Vec<_>>());
    // Nothing is left that autosave would retry without pause.
    assert_eq!(
        fixture
            .state
            .session
            .autosave_timeout(std::time::Instant::now(), &active),
        None
    );
}

#[test]
fn early_ink_on_an_unloaded_per_output_session_is_kept_and_its_retry_backs_off() {
    use crate::backend::wayland::session::SessionState;
    let temp = crate::test_temp::tempdir().unwrap();
    let mut options = crate::session::SessionOptions::new(temp.path().to_path_buf(), "startup");
    options.persist_transparent = true;
    options.per_output = true;
    let mut fixture = HandlerFixture::new(crate::config::Config::default());
    fixture.state.session = SessionState::new(Some(options.clone()));
    let shapes = draw_line(&mut fixture);
    fixture
        .state
        .begin_session_output_transition(Some("monitor-a".to_string()), "first monitor");
    crate::backend::wayland::session::driver::finish_pending_session_command(&mut fixture.state)
        .unwrap();
    assert_eq!(
        fixture
            .state
            .input_state
            .boards
            .active_frame()
            .shapes
            .iter()
            .map(|shape| shape.shape.clone())
            .collect::<Vec<_>>(),
        shapes
    );
    assert!(
        !options.session_file_path().exists(),
        "ink drawn before any load must not enter a file it never loaded"
    );
    assert!(fixture.state.input_state.active_toast().is_some());
    fixture
        .state
        .session
        .cancel_output_transition_for_live_source();
    fixture.state.input_state.needs_redraw = false;
    for _ in 0..20 {
        assert!(
            !fixture
                .state
                .reconcile_live_source_interaction_if_idle("test tick")
        );
        assert!(!fixture.state.input_state.needs_redraw);
    }
    let retry = fixture.state.session.pending_output_transition().unwrap();
    assert!(retry.retry_at > std::time::Instant::now());
}

#[test]
fn alt_text_drag_ends_through_the_real_pointer_release_handler() {
    let mut config = crate::config::Config::default();
    config.ui.show_onboarding_hints = false;
    let mut fixture = HandlerFixture::new(config);
    fixture.state.input_state.state = DrawingState::text_input(100, 100, "hello".to_string());
    fixture.state.input_state.modifiers.alt = true;
    let pointer = WlPointer::inert(fixture.conn.backend().downgrade());
    let surface = fixture.state.surface.wl_surface().unwrap().clone();
    let qh = fixture.queue.handle();

    for (start, end, expected) in [
        ((110.0, 100.0), (130.0, 115.0), (120, 115)),
        ((130.0, 115.0), (150.0, 130.0), (140, 130)),
    ] {
        let events = [
            PointerEvent {
                surface: surface.clone(),
                position: start,
                kind: PointerEventKind::Press {
                    time: 1,
                    button: BTN_LEFT,
                    serial: 1,
                },
            },
            PointerEvent {
                surface: surface.clone(),
                position: end,
                kind: PointerEventKind::Motion { time: 2 },
            },
            PointerEvent {
                surface: surface.clone(),
                position: end,
                kind: PointerEventKind::Release {
                    time: 3,
                    button: BTN_LEFT,
                    serial: 2,
                },
            },
        ];
        fixture
            .state
            .pointer_frame(&fixture.conn, &qh, &pointer, &events);
        assert!(
            !fixture.state.input_state.pointer_drag_active(),
            "the actual release must end the block move"
        );
        assert!(!fixture.state.input_state.has_active_pointer_interaction());
        fixture.state.pointer_frame(
            &fixture.conn,
            &qh,
            &pointer,
            &[PointerEvent {
                surface: surface.clone(),
                position: (180.0, 160.0),
                kind: PointerEventKind::Motion { time: 4 },
            }],
        );
        match &fixture.state.input_state.state {
            DrawingState::TextInput { x, y, buffer, .. } => {
                assert_eq!((*x, *y), expected);
                assert_eq!(buffer, "hello");
            }
            other => panic!("text draft changed: {other:?}"),
        }
    }
}
