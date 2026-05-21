use super::*;
use crate::draw::Color;
use crate::input::BOARD_ID_TRANSPARENT;
use crate::input::state::{PasteAnchor, test_support::make_test_input_state};
use crate::session::{CompressionMode, SnapshotPayloadEstimate, SnapshotSaveEstimate};
use crate::util::Rect;
use std::path::PathBuf;

#[test]
fn paste_preflight_synthesizes_empty_persisted_target_board() {
    let input = make_test_input_state();
    let mut options = session::SessionOptions::new(PathBuf::from("/tmp"), "display");
    options.persist_transparent = true;
    options.restore_tool_state = false;

    assert!(
        session::snapshot_from_input(&input, &options).is_none(),
        "empty boards are omitted from ordinary snapshots"
    );

    let snapshot = snapshot_after_external_image_paste_from_input(
        &input,
        &request_for_active_board(&input),
        &test_image(128),
        &options,
    )
    .expect("preflight snapshot");

    assert_eq!(snapshot.boards.len(), 1);
    assert_eq!(snapshot.boards[0].id, BOARD_ID_TRANSPARENT);
    assert_eq!(snapshot.boards[0].pages.pages.len(), 1);
    assert_eq!(snapshot.boards[0].pages.pages[0].shapes.len(), 1);
    assert_eq!(snapshot.boards[0].pages.pages[0].undo_stack_len(), 1);
}

#[test]
fn paste_preflight_does_not_synthesize_non_persisted_target_board() {
    let input = make_test_input_state();
    let mut options = session::SessionOptions::new(PathBuf::from("/tmp"), "display");
    options.persist_transparent = false;
    options.restore_tool_state = true;

    let snapshot = snapshot_after_external_image_paste_from_input(
        &input,
        &request_for_active_board(&input),
        &test_image(128),
        &options,
    );

    assert!(snapshot.is_none());
}

#[test]
fn paste_preflight_skips_stale_target_page_generation() {
    let mut input = make_test_input_state();
    input.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 1,
        y: 2,
        w: 3,
        h: 4,
        fill: false,
        color: Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 1.0,
    });
    let mut options = session::SessionOptions::new(PathBuf::from("/tmp"), "display");
    options.persist_transparent = true;
    options.restore_tool_state = false;

    assert!(
        session::snapshot_from_input(&input, &options).is_some(),
        "non-empty board would otherwise be eligible for preflight"
    );
    let mut request = request_for_active_board(&input);
    request.target_page_generation = request.target_page_generation.wrapping_add(1);

    let snapshot = snapshot_after_external_image_paste_from_input(
        &input,
        &request,
        &test_image(128),
        &options,
    );

    assert!(
        snapshot.is_none(),
        "stale target generation should bypass size preflight so paste reports target changed"
    );
}

#[test]
fn paste_decision_blocks_when_visible_data_exceeds_limit() {
    let mut options = session::SessionOptions::new(PathBuf::from("/tmp"), "display");
    options.max_file_size_bytes = 10 * 1024 * 1024;
    let estimate = estimate(13 * 1024 * 1024, 13 * 1024 * 1024, &options);

    match paste_persistence_decision(&estimate, &options) {
        PastePersistenceDecision::Block { warning } => {
            assert!(warning.toast.contains("Image blocked"));
            assert!(warning.toast.contains("20 MiB"));
        }
        other => panic!("expected block decision, got {other:?}"),
    }
}

#[test]
fn paste_decision_blocks_expanded_visible_data_with_restore_safety_message() {
    let mut options = session::SessionOptions::new(PathBuf::from("/tmp"), "display");
    options.max_file_size_bytes = 10 * 1024 * 1024;
    let estimate = SnapshotSaveEstimate {
        full: expanded_payload_estimate(&options),
        visible_without_history: expanded_payload_estimate(&options),
    };

    match paste_persistence_decision(&estimate, &options) {
        PastePersistenceDecision::Block { warning } => {
            assert!(warning.toast.contains("restore safety"));
            let body = warning
                .notification
                .as_ref()
                .map(|(_, body)| body.as_str())
                .unwrap_or_default();
            assert!(body.contains("restore safety limit"));
            assert!(body.contains("max_file_size_mb will not help"));
        }
        other => panic!("expected expanded-size block decision, got {other:?}"),
    }
}

#[test]
fn paste_decision_allows_with_warning_when_only_history_exceeds_limit() {
    let mut options = session::SessionOptions::new(PathBuf::from("/tmp"), "display");
    options.max_file_size_bytes = 10 * 1024 * 1024;
    let estimate = estimate(12 * 1024 * 1024, 8 * 1024 * 1024, &options);

    match paste_persistence_decision(&estimate, &options) {
        PastePersistenceDecision::Allow {
            warning: Some(warning),
        } => {
            assert!(warning.toast.contains("Undo history may be dropped"));
            assert!(warning.toast.contains("18 MiB"));
        }
        other => panic!("expected warning decision, got {other:?}"),
    }
}

#[test]
fn paste_decision_warns_for_expanded_history_without_file_size_advice() {
    let mut options = session::SessionOptions::new(PathBuf::from("/tmp"), "display");
    options.max_file_size_bytes = 10 * 1024 * 1024;
    let estimate = SnapshotSaveEstimate {
        full: expanded_payload_estimate(&options),
        visible_without_history: payload_estimate(8 * 1024 * 1024, &options),
    };

    match paste_persistence_decision(&estimate, &options) {
        PastePersistenceDecision::Allow {
            warning: Some(warning),
        } => {
            assert!(warning.toast.contains("restore safety"));
            let body = warning
                .notification
                .as_ref()
                .map(|(_, body)| body.as_str())
                .unwrap_or_default();
            assert!(body.contains("restore safety limit"));
            assert!(!body.contains("Max file size"));
        }
        other => panic!("expected expanded history warning decision, got {other:?}"),
    }
}

#[test]
fn paste_decision_warns_when_full_payload_is_near_limit() {
    let mut options = session::SessionOptions::new(PathBuf::from("/tmp"), "display");
    options.max_file_size_bytes = 10 * 1024 * 1024;
    let estimate = estimate(9 * 1024 * 1024, 9 * 1024 * 1024, &options);

    match paste_persistence_decision(&estimate, &options) {
        PastePersistenceDecision::Allow {
            warning: Some(warning),
        } => {
            assert!(warning.toast.contains("Session near limit"));
            assert!(warning.toast.contains("15 MiB"));
        }
        other => panic!("expected near-limit warning, got {other:?}"),
    }
}

fn request_for_active_board(input: &InputState) -> ClipboardPasteRequest {
    ClipboardPasteRequest {
        id: 1,
        target_board_id: input.board_id().to_string(),
        target_page_index: input.boards.active_page_index(),
        target_page_generation: input.boards.active_page_generation(),
        anchor: PasteAnchor::VisibleCenter { x: 10, y: 10 },
        visible_canvas_rect: Rect::new(0, 0, 100, 100).expect("rect"),
        screen_size: (100, 100),
        selection_clipboard_generation_at_request: 0,
        local_selection_fallback_generation: None,
    }
}

fn test_image(bytes: usize) -> EmbeddedImage {
    EmbeddedImage {
        mime_type: "image/png".to_string(),
        width: 16,
        height: 16,
        bytes: vec![7; bytes],
    }
}

fn estimate(
    full_written: u64,
    visible_written: u64,
    options: &session::SessionOptions,
) -> SnapshotSaveEstimate {
    SnapshotSaveEstimate {
        full: payload_estimate(full_written, options),
        visible_without_history: payload_estimate(visible_written, options),
    }
}

fn payload_estimate(
    written_size: u64,
    options: &session::SessionOptions,
) -> SnapshotPayloadEstimate {
    SnapshotPayloadEstimate {
        raw_size: written_size as usize,
        written_size: written_size as usize,
        max_file_size_bytes: options.max_file_size_bytes,
        compressed: matches!(options.compression, CompressionMode::On),
        limit_exceeded: (written_size > options.max_file_size_bytes).then_some(
            session::SaveLimitExceeded::WrittenSize {
                written_size,
                max_file_size: options.max_file_size_bytes,
            },
        ),
    }
}

fn expanded_payload_estimate(options: &session::SessionOptions) -> SnapshotPayloadEstimate {
    SnapshotPayloadEstimate {
        raw_size: 129 * 1024 * 1024,
        written_size: 2 * 1024 * 1024,
        max_file_size_bytes: options.max_file_size_bytes,
        compressed: true,
        limit_exceeded: Some(session::SaveLimitExceeded::ExpandedSize {
            raw_size: 129 * 1024 * 1024,
            max_expanded_size: 128 * 1024 * 1024,
        }),
    }
}
