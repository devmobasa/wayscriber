//! The paste decision after a failed private publish, through the same start
//! and probe-continuation steps a paste completion uses.
use super::*;
use crate::config::Action;
use crate::draw::Shape;
use crate::input::state::test_support::make_test_input_state;

fn fingerprint(hash: u64) -> ClipboardFingerprint {
    ClipboardFingerprint {
        offered_mime_types: vec!["image/png".to_string()],
        selected_mime_type: Some("image/png".to_string()),
        bounded_content_hash: Some(hash),
        bounded_content_len: Some(4096),
        bounded_content_truncated: true,
    }
}

/// Copies one rectangle and records its private publish as failed, with the
/// system clipboard fingerprint seen at the failure.
fn copy_with_failed_publish(
    input_state: &mut InputState,
    x: i32,
    at_failure: Option<ClipboardFingerprint>,
) -> u64 {
    let measurer = crate::draw::TextMeasurer::default();
    let ui_engine = crate::ui_text::UiTextEngine::default();
    let id = input_state
        .boards
        .active_frame_mut()
        .add_shape(Shape::Rect {
            x,
            y: 20,
            w: 100,
            h: 80,
            fill: false,
            fill_color: None,
            color: input_state.style.current_color,
            thick: input_state.style.current_thickness,
        });
    input_state.set_selection(vec![id]);
    input_state.handle_action_with_resources(
        crate::input::state::InputTextResources {
            measurer: &measurer,
            ui_engine: &ui_engine,
        },
        Action::CopySelection,
    );

    let publish = input_state
        .take_pending_selection_clipboard_publish()
        .expect("pending private clipboard publish");
    input_state.complete_selection_clipboard_publish(publish.generation, at_failure, false);
    publish.generation
}

/// Starts a paste, which probes the system clipboard because the publish failed.
fn start_probe(
    input_state: &mut InputState,
) -> (ClipboardPasteRequest, u64, Option<ClipboardFingerprint>) {
    let request = input_state.request_clipboard_paste();
    match plan_clipboard_paste_start(input_state, request).action {
        PasteAction::ProbeSystemFingerprint {
            request,
            generation,
            expected,
        } => (request, generation, expected),
        other => panic!("expected a fingerprint probe, got {other:?}"),
    }
}

#[test]
fn a_failed_publish_is_pasted_locally_only_while_the_system_clipboard_is_unchanged() {
    let mut input_state = make_test_input_state();
    copy_with_failed_publish(&mut input_state, 10, Some(fingerprint(1)));

    let (request, generation, expected) = start_probe(&mut input_state);
    let action = continue_after_fingerprint_probe(
        &mut input_state,
        request,
        generation,
        expected,
        Some(fingerprint(1)),
    );
    assert!(matches!(action, PasteAction::UseLocalShapes { .. }));

    let (request, generation, expected) = start_probe(&mut input_state);
    let action = continue_after_fingerprint_probe(
        &mut input_state,
        request,
        generation,
        expected,
        Some(fingerprint(2)),
    );
    assert!(matches!(action, PasteAction::ReadSystemClipboard { .. }));
    let local = input_state.selection_clipboard_snapshot();
    assert!(!local.fallback_allowed());
    assert_eq!(local.fallback_generation(), None);
}

#[test]
fn a_failed_publish_without_a_fingerprint_is_superseded_by_a_readable_clipboard() {
    let mut input_state = make_test_input_state();
    copy_with_failed_publish(&mut input_state, 10, None);

    let (request, generation, expected) = start_probe(&mut input_state);
    assert_eq!(expected, None);
    let action = continue_after_fingerprint_probe(
        &mut input_state,
        request,
        generation,
        expected,
        Some(fingerprint(1)),
    );

    assert!(matches!(action, PasteAction::ReadSystemClipboard { .. }));
    assert!(
        !input_state
            .selection_clipboard_snapshot()
            .fallback_allowed()
    );
}

#[test]
fn an_unreadable_clipboard_reads_the_system_clipboard_and_keeps_the_fallback() {
    let mut input_state = make_test_input_state();
    copy_with_failed_publish(&mut input_state, 10, None);

    let (request, generation, expected) = start_probe(&mut input_state);
    let action =
        continue_after_fingerprint_probe(&mut input_state, request, generation, expected, None);

    assert!(matches!(action, PasteAction::ReadSystemClipboard { .. }));
    assert!(
        input_state
            .selection_clipboard_snapshot()
            .fallback_allowed(),
        "a transport failure can still fall back after the normal read"
    );
}

#[test]
fn a_probe_for_an_older_copy_neither_pastes_nor_supersedes_a_newer_one() {
    let mut input_state = make_test_input_state();
    let first = copy_with_failed_publish(&mut input_state, 10, Some(fingerprint(1)));
    let (request, generation, expected) = start_probe(&mut input_state);
    assert_eq!(generation, first);

    let second = copy_with_failed_publish(&mut input_state, 200, Some(fingerprint(1)));

    // A paste requested for the first copy no longer probes it.
    assert!(matches!(
        plan_clipboard_paste_start(&input_state, request.clone()).action,
        PasteAction::ReadSystemClipboard { .. }
    ));
    // A probe already running for it finds nothing to paste.
    let action = continue_after_fingerprint_probe(
        &mut input_state,
        request,
        generation,
        expected,
        Some(fingerprint(1)),
    );
    assert!(matches!(action, PasteAction::ReadSystemClipboard { .. }));
    assert_eq!(
        input_state
            .selection_clipboard_snapshot()
            .fallback_generation(),
        Some(second)
    );
}
