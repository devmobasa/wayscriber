use super::*;
use crate::draw::Shape;
use crate::util::Rect;

#[test]
fn fingerprint_mismatch_supersedes_then_reads_system_clipboard() {
    let request = request_with_fallback_generation(Some(7));
    let previous = fingerprint(1);
    let current = fingerprint(2);

    let plan = plan_after_fingerprint_probe(request, 7, Some(previous), Some(current), None);

    assert_eq!(
        plan.effects,
        vec![TransferEffect::SupersedeLocalGeneration { generation: 7 }]
    );
    assert!(matches!(
        plan.action,
        PasteAction::ReadSystemClipboard { .. }
    ));
}

#[test]
fn timeout_uses_fresh_fallback_action_instead_of_captured_shapes() {
    let request = request_with_fallback_generation(Some(7));
    let completion = ClipboardPasteCompletion {
        request,
        result: ClipboardPasteResult::ReadTimedOut,
    };

    let plan = plan_paste_completion(completion, Some(42), None);

    assert!(plan.effects.is_empty());
    assert!(matches!(
        plan.action,
        PasteAction::TryFreshLocalFallbackOrWarn { .. }
    ));
}

#[test]
fn clipboard_terminal_outcomes_keep_their_existing_event_loop_policy() {
    let warning_cases = [
        (
            ClipboardPasteResult::ClipboardEmpty,
            TransferWarning::ClipboardEmpty,
        ),
        (
            ClipboardPasteResult::NoSupportedMime {
                offered: vec!["text/plain".to_string()],
            },
            TransferWarning::UnsupportedContent,
        ),
        (
            ClipboardPasteResult::DecodeFailed("bad image".to_string()),
            TransferWarning::DecodeFailed,
        ),
        (
            ClipboardPasteResult::TooLarge { limit: 1024 },
            TransferWarning::TooLarge { limit: 1024 },
        ),
    ];
    for (result, expected_warning) in warning_cases {
        let completion = ClipboardPasteCompletion {
            request: request_with_fallback_generation(Some(7)),
            result,
        };
        let plan = plan_paste_completion(completion, Some(42), None);
        assert!(matches!(
            plan.action,
            PasteAction::ShowWarning {
                warning,
                block_feedback: true,
                ..
            } if warning == expected_warning
        ));
    }

    let failure = plan_paste_completion(
        ClipboardPasteCompletion {
            request: request_with_fallback_generation(Some(7)),
            result: ClipboardPasteResult::ClipboardError("disconnected".to_string()),
        },
        Some(42),
        None,
    );
    assert!(matches!(
        failure.action,
        PasteAction::TryFreshLocalFallbackOrWarn {
            missing_warning: TransferWarning::ClipboardError,
            ..
        }
    ));

    let image = plan_paste_completion(
        ClipboardPasteCompletion {
            request: request_with_fallback_generation(Some(7)),
            result: ClipboardPasteResult::Image(crate::draw::EmbeddedImage {
                mime_type: "image/png".to_string(),
                width: 1,
                height: 1,
                bytes: vec![0, 0, 0, 255],
            }),
        },
        Some(42),
        None,
    );
    assert!(matches!(
        image.action,
        PasteAction::ApplyExternalImage { .. }
    ));
}

#[test]
fn stale_domain_request_is_rejected_after_transport_matching() {
    let completion = ClipboardPasteCompletion {
        request: request_with_fallback_generation(None),
        result: ClipboardPasteResult::ClipboardEmpty,
    };
    let plan = plan_paste_completion(completion, Some(99), None);
    assert!(matches!(
        plan.action,
        PasteAction::StaleCompletion { request_id: 42 }
    ));
}

#[test]
fn same_instance_nonmatching_generation_does_not_apply_private_selection() {
    let request = request_with_fallback_generation(Some(7));
    let completion = ClipboardPasteCompletion {
        request,
        result: ClipboardPasteResult::PrivateSelection(WayscriberClipboardSelection {
            schema_version: 1,
            app_version: "test".to_string(),
            app_instance_id: "same".to_string(),
            copy_generation: 8,
            shapes: vec![rect()],
        }),
    };
    let resolution = PrivateSelectionResolution {
        payload_matches_local: false,
        same_instance: true,
        shapes: None,
    };

    let plan = plan_paste_completion(completion, Some(42), Some(resolution));

    assert_eq!(
        plan.effects,
        vec![TransferEffect::SupersedeLocalGeneration { generation: 7 }]
    );
    assert!(matches!(
        plan.action,
        PasteAction::ShowWarning {
            request: _,
            warning: TransferWarning::NoShapesPasted,
            block_feedback: true,
        }
    ));
}

#[test]
fn different_instance_private_payload_supersedes_then_applies_shapes() {
    let request = request_with_fallback_generation(Some(7));
    let completion = ClipboardPasteCompletion {
        request,
        result: ClipboardPasteResult::PrivateSelection(WayscriberClipboardSelection {
            schema_version: 1,
            app_version: "test".to_string(),
            app_instance_id: "other".to_string(),
            copy_generation: 8,
            shapes: vec![rect()],
        }),
    };
    let resolution = PrivateSelectionResolution {
        payload_matches_local: false,
        same_instance: false,
        shapes: Some(vec![rect()]),
    };

    let plan = plan_paste_completion(completion, Some(42), Some(resolution));

    assert_eq!(
        plan.effects,
        vec![TransferEffect::SupersedeLocalGeneration { generation: 7 }]
    );
    assert!(matches!(
        plan.action,
        PasteAction::ApplyPrivateSelection { .. }
    ));
}

fn request_with_fallback_generation(
    local_selection_fallback_generation: Option<u64>,
) -> ClipboardPasteRequest {
    ClipboardPasteRequest {
        id: 42,
        target_board_id: "board".to_string(),
        target_page_index: 0,
        target_page_generation: 1,
        anchor: crate::input::state::PasteAnchor::VisibleCenter { x: 50, y: 50 },
        visible_canvas_rect: Rect::new(0, 0, 100, 100).unwrap(),
        screen_size: (100, 100),
        selection_clipboard_generation_at_request: local_selection_fallback_generation.unwrap_or(0),
        local_selection_fallback_generation,
    }
}

fn fingerprint(hash: u64) -> ClipboardFingerprint {
    ClipboardFingerprint {
        offered_mime_types: vec!["image/png".to_string()],
        selected_mime_type: Some("image/png".to_string()),
        bounded_content_hash: Some(hash),
        bounded_content_len: Some(4),
        bounded_content_truncated: false,
    }
}

fn rect() -> Shape {
    Shape::Rect {
        x: 0,
        y: 0,
        w: 10,
        h: 10,
        fill: false,
        color: crate::draw::BLACK,
        thick: 1.0,
    }
}
