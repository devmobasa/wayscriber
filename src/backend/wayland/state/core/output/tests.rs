use super::{
    OutputTransitionStart, live_source_reconciliation_ready, output_transition_retry_at,
    output_transition_start, replace_output_session_snapshot,
};
use crate::{
    backend::wayland::session::SessionState,
    draw::{Color, Frame, Shape},
    input::state::test_support::make_test_input_state,
    session::{BoardPagesSnapshot, BoardSnapshot, SessionOptions, SessionSnapshot},
};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn add_test_line(input: &mut crate::input::InputState) {
    input.boards.active_frame_mut().add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 20,
        y2: 20,
        color: Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 2.0,
    });
}

#[test]
fn empty_output_load_replaces_source_board_contents() {
    let options = SessionOptions::new(PathBuf::from("/tmp"), "empty-output");
    let mut input = make_test_input_state();
    add_test_line(&mut input);
    assert_eq!(input.boards.active_frame().shapes.len(), 1);

    replace_output_session_snapshot(&mut input, None, &options).expect("empty output replacement");

    assert!(input.boards.active_frame().shapes.is_empty());
}

#[test]
fn partial_output_load_clears_boards_omitted_from_snapshot() {
    let options = SessionOptions::new(PathBuf::from("/tmp"), "partial-output");
    let mut input = make_test_input_state();
    input.switch_board_force("transparent");
    add_test_line(&mut input);
    assert_eq!(input.boards.active_frame().shapes.len(), 1);
    let snapshot = SessionSnapshot {
        active_board_id: "whiteboard".to_string(),
        boards: vec![BoardSnapshot {
            id: "whiteboard".to_string(),
            pages: BoardPagesSnapshot {
                pages: vec![Frame::new()],
                active: 0,
            },
        }],
        tool_state: None,
    };

    replace_output_session_snapshot(&mut input, Some(snapshot), &options)
        .expect("partial output replacement");

    input.switch_board_force("transparent");
    assert!(input.boards.active_frame().shapes.is_empty());
}

#[test]
fn failed_output_replacement_preserves_source_board_contents() {
    let options = SessionOptions::new(PathBuf::from("/tmp"), "oversized-output");
    let mut input = make_test_input_state();
    add_test_line(&mut input);
    let boards = (0..=input.boards.max_count())
        .map(|index| BoardSnapshot {
            id: format!("replacement-{index}"),
            pages: BoardPagesSnapshot {
                pages: vec![Frame::new()],
                active: 0,
            },
        })
        .collect();
    let snapshot = SessionSnapshot {
        active_board_id: "replacement-0".to_string(),
        boards,
        tool_state: None,
    };

    let err = replace_output_session_snapshot(&mut input, Some(snapshot), &options)
        .expect_err("oversized replacement must fail before mutating live boards");

    assert!(err.to_string().contains("current runtime allows"));
    assert_eq!(input.boards.active_frame().shapes.len(), 1);
}

#[test]
fn configure_retry_keeps_matching_epoch_bound_transition() {
    assert_eq!(
        output_transition_start(false, false, true, true, false, false),
        OutputTransitionStart::KeepPending
    );
}

#[test]
fn initial_and_loaded_transitions_defer_for_active_interaction() {
    assert_eq!(
        output_transition_start(false, true, false, false, false, true),
        OutputTransitionStart::DeferForInteraction
    );
    assert_eq!(
        output_transition_start(true, true, false, false, false, true),
        OutputTransitionStart::DeferForInteraction
    );
}

#[test]
fn transition_start_distinguishes_initial_load_and_loaded_switch() {
    assert_eq!(
        output_transition_start(false, true, false, false, false, false),
        OutputTransitionStart::LoadInitial
    );
    assert_eq!(
        output_transition_start(true, true, false, false, false, false),
        OutputTransitionStart::ResolveTransition
    );
}

#[test]
fn unloaded_dirty_source_with_nonmatching_pending_transition_resolves_before_load() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "source-output");
    options.per_output = true;
    let mut first_target = options.clone();
    first_target.set_output_identity(Some("output-a"));
    let mut session = SessionState::new(Some(options.clone()));
    session.stage_output_transition(first_target, Some("output-a".to_string()), Instant::now());
    session.record_input_dirty(Instant::now(), true);

    let incoming_identity = Some("output-b".to_string());
    let mut incoming = options;
    let changed = incoming.set_output_identity(incoming_identity.as_deref());
    let same_epoch_pending = session
        .pending_output_transition()
        .is_some_and(|pending| pending.source_epoch == session.target_epoch());
    let matching_pending = same_epoch_pending
        && session
            .pending_output_transition()
            .is_some_and(|pending| pending.physical_output_identity == incoming_identity);

    assert!(!session.is_loaded());
    assert!(session.is_dirty());
    assert!(!matching_pending);
    assert_eq!(
        output_transition_start(
            session.is_loaded(),
            changed,
            matching_pending,
            same_epoch_pending,
            false,
            false,
        ),
        OutputTransitionStart::ResolveTransition
    );
    assert!(session.is_dirty());
}

#[test]
fn unloaded_dirty_return_to_current_target_blocks_followup_configure_load() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "source-output");
    options.per_output = true;
    let mut destination = options.clone();
    destination.set_output_identity(Some("output-a"));
    let mut session = SessionState::new(Some(options.clone()));
    session.stage_output_transition(destination, Some("output-a".to_string()), Instant::now());
    session.record_input_dirty(Instant::now(), true);
    let source_epoch = session.target_epoch();
    let edit_generation = session.edit_generation();

    let incoming_identity = None;
    let mut incoming = options.clone();
    let changed = incoming.set_output_identity(incoming_identity);
    let same_epoch_pending = session
        .pending_output_transition()
        .is_some_and(|pending| pending.source_epoch == source_epoch);
    let matching_pending = same_epoch_pending
        && session
            .pending_output_transition()
            .is_some_and(|pending| pending.physical_output_identity.is_none());
    let start = output_transition_start(
        session.is_loaded(),
        changed,
        matching_pending,
        same_epoch_pending,
        false,
        false,
    );

    assert!(!session.is_loaded());
    assert!(!changed);
    assert!(!matching_pending);
    assert_eq!(start, OutputTransitionStart::IgnoreCurrentTarget);
    assert!(
        session
            .cancel_output_transition_for_live_source(false)
            .is_some()
    );
    assert!(session.pending_output_transition().is_none());
    assert!(session.is_loaded());
    assert!(session.is_dirty());
    assert!(session.prepare_autosave_submission().is_ok());
    assert_eq!(session.target_epoch(), source_epoch);
    assert_eq!(session.edit_generation(), edit_generation);

    let mut configure_options = options;
    let configure_changed = configure_options.set_output_identity(None);
    let followup = output_transition_start(
        session.is_loaded(),
        configure_changed,
        false,
        false,
        false,
        false,
    );

    assert!(!configure_changed);
    assert_eq!(followup, OutputTransitionStart::IgnoreCurrentTarget);
    assert_ne!(followup, OutputTransitionStart::LoadInitial);
    assert!(session.is_dirty());
    assert!(session.prepare_autosave_submission().is_ok());
    assert_eq!(session.target_epoch(), source_epoch);
    assert_eq!(session.edit_generation(), edit_generation);
}

#[test]
fn active_stroke_return_resolves_to_dirty_live_source_or_clean_initial_load() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "source-output");
    options.per_output = true;
    let mut destination = options.clone();
    destination.set_output_identity(Some("output-a"));

    let mut committed = SessionState::new(Some(options.clone()));
    committed.stage_output_transition(
        destination.clone(),
        Some("output-a".to_string()),
        Instant::now(),
    );
    let committed_epoch = committed.target_epoch();
    assert_eq!(
        output_transition_start(false, false, false, true, false, true),
        OutputTransitionStart::IgnoreCurrentTarget
    );
    assert!(
        committed
            .cancel_output_transition_for_live_source(false)
            .is_some()
    );
    assert!(!committed.is_loaded());
    assert!(committed.resolve_live_source_resolution(false, true));
    assert_eq!(
        output_transition_start(false, false, false, false, true, true),
        OutputTransitionStart::IgnoreCurrentTarget
    );

    committed.record_input_dirty(Instant::now(), true);
    assert!(committed.is_loaded());
    assert!(committed.is_dirty());
    assert!(!committed.resolve_live_source_resolution(false, false));
    assert_eq!(committed.target_epoch(), committed_epoch);
    assert_eq!(
        output_transition_start(true, false, false, false, false, false),
        OutputTransitionStart::IgnoreCurrentTarget
    );

    let mut configure_first = SessionState::new(Some(options.clone()));
    configure_first.stage_output_transition(
        destination.clone(),
        Some("output-a".to_string()),
        Instant::now(),
    );
    assert!(
        configure_first
            .cancel_output_transition_for_live_source(false)
            .is_some()
    );
    assert!(!configure_first.resolve_live_source_resolution(true, false));
    assert!(configure_first.is_loaded());

    let mut canceled = SessionState::new(Some(options));
    canceled.stage_output_transition(destination, Some("output-a".to_string()), Instant::now());
    let canceled_epoch = canceled.target_epoch();
    assert!(
        canceled
            .cancel_output_transition_for_live_source(false)
            .is_some()
    );
    assert!(!canceled.resolve_live_source_resolution(false, false));
    assert!(!canceled.is_loaded());
    assert!(!canceled.is_dirty());
    assert_eq!(canceled.target_epoch(), canceled_epoch);
    assert_eq!(
        output_transition_start(false, false, false, false, false, false),
        OutputTransitionStart::LoadInitial
    );
}

#[test]
fn live_source_reconciliation_runs_only_when_idle_without_a_destination() {
    assert!(live_source_reconciliation_ready(true, false, false, true));
    assert!(!live_source_reconciliation_ready(false, false, false, true));
    assert!(!live_source_reconciliation_ready(true, true, false, true));
    assert!(!live_source_reconciliation_ready(true, false, true, true));
    assert!(!live_source_reconciliation_ready(true, false, false, false));
}

#[test]
fn clean_unloaded_cancellation_arms_immediate_source_resolution() {
    let options = SessionOptions::new(PathBuf::from("/tmp"), "source-output");
    let mut session = SessionState::new(Some(options.clone()));
    session.stage_output_transition(options, Some("output-a".to_string()), Instant::now());

    assert!(
        session
            .cancel_output_transition_for_live_source(false)
            .is_some()
    );
    assert!(session.has_pending_live_source_resolution());
    assert!(live_source_reconciliation_ready(true, false, false, true));
    assert!(!session.resolve_live_source_resolution(false, false));
    assert!(!session.is_loaded());
    assert!(!session.has_pending_live_source_resolution());
    assert_eq!(
        output_transition_start(false, false, false, false, false, false),
        OutputTransitionStart::LoadInitial
    );
}

#[test]
fn loaded_source_cancellation_does_not_arm_provisional_resolution() {
    let mut session = SessionState::new(None);
    session.mark_loaded(false);
    session.stage_output_transition(
        SessionOptions::new(PathBuf::from("/tmp"), "loaded-destination"),
        Some("output-a".to_string()),
        Instant::now(),
    );

    assert!(
        session
            .cancel_output_transition_for_live_source(false)
            .is_some()
    );
    assert!(session.is_loaded());
    assert!(!session.has_pending_live_source_resolution());
}

#[test]
fn failure_retry_deadline_is_based_on_failure_observation_time() {
    let backoff = Duration::from_millis(50);
    let before_failure_handling = Instant::now();
    let retry_at = output_transition_retry_at(backoff);

    assert!(retry_at >= before_failure_handling + backoff);
}
