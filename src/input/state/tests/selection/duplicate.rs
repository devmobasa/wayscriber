use super::*;

#[test]
fn duplicate_selection_via_action_creates_offset_shape() {
    let mut state = create_test_input_state();
    let original_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 100,
        h: 80,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.set_selection(vec![original_id]);
    state.handle_action(Action::DuplicateSelection);

    let frame = state.boards.active_frame();
    assert_eq!(frame.shapes.len(), 2);

    let new_id = frame
        .shapes
        .iter()
        .map(|shape| shape.id)
        .find(|id| *id != original_id)
        .expect("duplicate shape id");
    let original = frame.shape(original_id).unwrap();
    let duplicate = frame.shape(new_id).unwrap();

    match (&original.shape, &duplicate.shape) {
        (Shape::Rect { x: ox, y: oy, .. }, Shape::Rect { x: dx, y: dy, .. }) => {
            assert_eq!(*dx, ox + 12);
            assert_eq!(*dy, oy + 12);
        }
        _ => panic!("Expected rectangles"),
    }
}

#[test]
fn copy_paste_selection_creates_offset_shape() {
    let mut state = create_test_input_state();
    let original_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 100,
        h: 80,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.set_selection(vec![original_id]);
    state.handle_action(Action::CopySelection);
    state.handle_action(Action::PasteSelection);
    let request = state
        .take_pending_clipboard_paste_request()
        .expect("pending paste request");
    let shapes = state
        .local_selection_shapes_for_fallback(
            request
                .local_selection_fallback_generation
                .expect("local fallback generation"),
        )
        .expect("local selection fallback");
    assert_eq!(
        state.paste_clipboard_shapes_from_request(&request, shapes),
        1
    );
    state.finish_clipboard_paste_request(request.id);

    let frame = state.boards.active_frame();
    assert_eq!(frame.shapes.len(), 2);

    let new_id = frame
        .shapes
        .iter()
        .map(|shape| shape.id)
        .find(|id| *id != original_id)
        .expect("pasted shape id");
    let original = frame.shape(original_id).unwrap();
    let pasted = frame.shape(new_id).unwrap();

    match (&original.shape, &pasted.shape) {
        (Shape::Rect { x: ox, y: oy, .. }, Shape::Rect { x: px, y: py, .. }) => {
            assert_eq!(*px, ox + 12);
            assert_eq!(*py, oy + 12);
        }
        _ => panic!("Expected rectangles"),
    }
}

#[test]
fn immediate_paste_after_copy_uses_pending_local_publish_shapes() {
    let mut state = create_test_input_state();
    let original_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 100,
        h: 80,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![original_id]);
    state.handle_action(Action::CopySelection);
    state.handle_action(Action::PasteSelection);
    let request = state
        .take_pending_clipboard_paste_request()
        .expect("pending paste request");
    let shapes = state
        .local_selection_shapes_for_pending_publish(request.local_selection_fallback_generation)
        .expect("pending local publish selection");

    assert_eq!(
        state.paste_clipboard_shapes_from_request(&request, shapes),
        1
    );
    state.finish_clipboard_paste_request(request.id);

    assert_eq!(state.boards.active_frame().shapes.len(), 2);
}

#[test]
fn stale_publish_completion_is_ignored_for_newer_copy() {
    let mut state = create_test_input_state();
    let first_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 100,
        h: 80,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![first_id]);
    state.handle_action(Action::CopySelection);
    let first_publish = state
        .take_pending_selection_clipboard_publish()
        .expect("pending first private clipboard publish");

    let second_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 200,
        y: 220,
        w: 90,
        h: 70,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![second_id]);
    state.handle_action(Action::CopySelection);
    let second_publish = state
        .take_pending_selection_clipboard_publish()
        .expect("pending second private clipboard publish");

    assert!(!state.complete_selection_clipboard_publish(first_publish.generation, None, false));
    assert_eq!(
        state.local_selection_fallback_generation(),
        Some(second_publish.generation)
    );
    assert!(
        state
            .local_selection_shapes_for_pending_publish(Some(second_publish.generation))
            .is_some()
    );
}

#[test]
fn failed_local_clipboard_precedence_clears_when_fingerprint_changes() {
    let mut state = create_test_input_state();
    let original_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 100,
        h: 80,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![original_id]);
    state.handle_action(Action::CopySelection);
    let publish = state
        .take_pending_selection_clipboard_publish()
        .expect("pending private clipboard publish");
    let initial_fingerprint = ClipboardFingerprint {
        offered_mime_types: vec!["image/png".to_string()],
        selected_mime_type: Some("image/png".to_string()),
        bounded_content_hash: Some(1),
        bounded_content_len: Some(4096),
        bounded_content_truncated: true,
    };
    state.complete_selection_clipboard_publish(
        publish.generation,
        Some(initial_fingerprint.clone()),
        false,
    );

    assert!(
        state
            .failed_local_selection_after_fingerprint_probe(
                Some(publish.generation),
                Some(initial_fingerprint),
            )
            .is_some()
    );

    let changed_fingerprint = ClipboardFingerprint {
        offered_mime_types: vec!["image/png".to_string()],
        selected_mime_type: Some("image/png".to_string()),
        bounded_content_hash: Some(2),
        bounded_content_len: Some(4096),
        bounded_content_truncated: true,
    };
    assert!(
        state
            .failed_local_selection_after_fingerprint_probe(
                Some(publish.generation),
                Some(changed_fingerprint),
            )
            .is_none()
    );
    assert!(!state.local_selection_fallback_allowed());
    assert_eq!(state.local_selection_fallback_generation(), None);
}

#[test]
fn failed_local_clipboard_without_failure_fingerprint_supersedes_when_current_is_readable() {
    let mut state = create_test_input_state();
    let original_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 100,
        h: 80,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![original_id]);
    state.handle_action(Action::CopySelection);
    let publish = state
        .take_pending_selection_clipboard_publish()
        .expect("pending private clipboard publish");
    state.complete_selection_clipboard_publish(publish.generation, None, false);

    let current_fingerprint = ClipboardFingerprint {
        offered_mime_types: vec!["image/png".to_string()],
        selected_mime_type: Some("image/png".to_string()),
        bounded_content_hash: Some(1),
        bounded_content_len: Some(4096),
        bounded_content_truncated: true,
    };
    assert!(
        state
            .failed_local_selection_after_fingerprint_probe(
                Some(publish.generation),
                Some(current_fingerprint),
            )
            .is_none()
    );
    assert!(!state.local_selection_fallback_allowed());
}

#[test]
fn failed_local_clipboard_without_current_fingerprint_does_not_fast_path() {
    let mut state = create_test_input_state();
    let original_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 100,
        h: 80,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![original_id]);
    state.handle_action(Action::CopySelection);
    let publish = state
        .take_pending_selection_clipboard_publish()
        .expect("pending private clipboard publish");
    state.complete_selection_clipboard_publish(publish.generation, None, false);

    assert!(
        state
            .failed_local_selection_after_fingerprint_probe(Some(publish.generation), None)
            .is_none()
    );
    assert!(
        state.local_selection_fallback_allowed(),
        "transport failure fallback remains available after normal resolution"
    );
}

#[test]
fn published_selection_allows_local_fallback_until_superseded() {
    let mut state = create_test_input_state();
    let original_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 100,
        h: 80,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![original_id]);
    state.handle_action(Action::CopySelection);
    let publish = state
        .take_pending_selection_clipboard_publish()
        .expect("pending private clipboard publish");

    state.complete_selection_clipboard_publish(publish.generation, None, true);
    assert!(state.local_selection_fallback_allowed());
    let generation = state
        .local_selection_fallback_generation()
        .expect("fallback generation");
    assert!(
        state
            .local_selection_shapes_for_fallback(generation)
            .is_some()
    );

    state.mark_selection_clipboard_superseded();
    assert!(!state.local_selection_fallback_allowed());
    assert!(
        state
            .local_selection_shapes_for_fallback(generation)
            .is_none()
    );
}

#[test]
fn fallback_generation_rejects_newer_local_copy() {
    let mut state = create_test_input_state();
    let original_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 100,
        h: 80,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![original_id]);
    state.handle_action(Action::CopySelection);
    state.handle_action(Action::PasteSelection);
    let request = state
        .take_pending_clipboard_paste_request()
        .expect("pending paste request");
    let request_generation = request
        .local_selection_fallback_generation
        .expect("fallback generation");

    state.handle_action(Action::CopySelection);

    assert_ne!(
        Some(request_generation),
        state.local_selection_fallback_generation()
    );
    assert!(
        state
            .local_selection_shapes_for_fallback(request_generation)
            .is_none()
    );
}

#[test]
fn private_payload_for_request_rejects_newer_same_instance_generation() {
    let mut state = create_test_input_state();
    let first_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 100,
        h: 80,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![first_id]);
    state.handle_action(Action::CopySelection);
    let _first_publish = state
        .take_pending_selection_clipboard_publish()
        .expect("pending first private clipboard publish");
    state.handle_action(Action::PasteSelection);
    let request = state
        .take_pending_clipboard_paste_request()
        .expect("pending paste request");

    let second_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 200,
        y: 220,
        w: 90,
        h: 70,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![second_id]);
    state.handle_action(Action::CopySelection);
    let second_publish = state
        .take_pending_selection_clipboard_publish()
        .expect("pending second private clipboard publish");
    let second_payload: WayscriberClipboardSelection =
        serde_json::from_str(&second_publish.payload_json).expect("second payload json");

    assert!(
        state
            .private_payload_shapes_for_request(&request, second_payload)
            .is_none()
    );
}

#[test]
fn private_payload_for_request_uses_payload_when_current_generation_changed() {
    let mut state = create_test_input_state();
    let first_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 100,
        h: 80,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![first_id]);
    state.handle_action(Action::CopySelection);
    let first_publish = state
        .take_pending_selection_clipboard_publish()
        .expect("pending first private clipboard publish");
    let first_payload: WayscriberClipboardSelection =
        serde_json::from_str(&first_publish.payload_json).expect("first payload json");
    state.handle_action(Action::PasteSelection);
    let request = state
        .take_pending_clipboard_paste_request()
        .expect("pending paste request");

    let second_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 200,
        y: 220,
        w: 90,
        h: 70,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![second_id]);
    state.handle_action(Action::CopySelection);

    let shapes = state
        .private_payload_shapes_for_request(&request, first_payload)
        .expect("request-owned private payload shapes");
    assert_eq!(shapes.len(), 1);
    match &shapes[0] {
        Shape::Rect { x, y, .. } => {
            assert_eq!((*x, *y), (10, 20));
        }
        _ => panic!("Expected rectangle"),
    }
}

#[test]
fn same_instance_private_payload_with_no_fallback_generation_uses_payload() {
    let mut state = create_test_input_state();
    let original_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 100,
        h: 80,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![original_id]);
    state.handle_action(Action::CopySelection);
    let publish = state
        .take_pending_selection_clipboard_publish()
        .expect("pending private clipboard publish");
    let payload: WayscriberClipboardSelection =
        serde_json::from_str(&publish.payload_json).expect("payload json");

    state.mark_selection_clipboard_superseded();
    state.handle_action(Action::PasteSelection);
    let request = state
        .take_pending_clipboard_paste_request()
        .expect("pending paste request");

    assert_eq!(request.local_selection_fallback_generation, None);
    assert_eq!(
        request.selection_clipboard_generation_at_request,
        payload.copy_generation
    );
    assert!(
        state
            .private_payload_shapes_for_request(&request, payload)
            .is_some()
    );
}

#[test]
fn request_generation_supersede_ignores_newer_local_copy() {
    let mut state = create_test_input_state();
    let first_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 100,
        h: 80,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![first_id]);
    state.handle_action(Action::CopySelection);
    state.handle_action(Action::PasteSelection);
    let request = state
        .take_pending_clipboard_paste_request()
        .expect("pending paste request");
    let request_generation = request
        .local_selection_fallback_generation
        .expect("request generation");

    let second_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 200,
        y: 220,
        w: 90,
        h: 70,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![second_id]);
    state.handle_action(Action::CopySelection);
    let current_generation = state
        .local_selection_fallback_generation()
        .expect("current generation");

    assert_ne!(request_generation, current_generation);
    state.mark_selection_clipboard_superseded_for_generation(Some(request_generation));

    assert_eq!(
        state.local_selection_fallback_generation(),
        Some(current_generation)
    );
    assert!(state.local_selection_fallback_allowed());
}

#[test]
fn failed_local_fast_path_rejects_newer_generation() {
    let mut state = create_test_input_state();
    let first_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 100,
        h: 80,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![first_id]);
    state.handle_action(Action::CopySelection);
    let first_publish = state
        .take_pending_selection_clipboard_publish()
        .expect("pending first private clipboard publish");
    let fingerprint = ClipboardFingerprint {
        offered_mime_types: vec!["image/png".to_string()],
        selected_mime_type: Some("image/png".to_string()),
        bounded_content_hash: Some(1),
        bounded_content_len: Some(4096),
        bounded_content_truncated: true,
    };
    state.complete_selection_clipboard_publish(
        first_publish.generation,
        Some(fingerprint.clone()),
        false,
    );
    state.handle_action(Action::PasteSelection);
    let request = state
        .take_pending_clipboard_paste_request()
        .expect("pending paste request");
    let request_generation = request.local_selection_fallback_generation;

    let second_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 200,
        y: 220,
        w: 90,
        h: 70,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![second_id]);
    state.handle_action(Action::CopySelection);
    let second_publish = state
        .take_pending_selection_clipboard_publish()
        .expect("pending second private clipboard publish");
    state.complete_selection_clipboard_publish(
        second_publish.generation,
        Some(fingerprint.clone()),
        false,
    );

    assert!(!state.has_failed_local_selection_for_generation(request_generation));
    assert!(
        state
            .failed_local_selection_after_fingerprint_probe(request_generation, Some(fingerprint))
            .is_none()
    );
    assert_eq!(
        state.local_selection_fallback_generation(),
        Some(second_publish.generation)
    );
}

#[test]
fn duplicate_selection_skips_locked_shapes() {
    let mut state = create_test_input_state();
    let unlocked_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 0,
        y: 0,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let locked_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 20,
        y: 20,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    if let Some(index) = state.boards.active_frame().find_index(locked_id) {
        state.boards.active_frame_mut().shapes[index].locked = true;
    }

    state.set_selection(vec![unlocked_id, locked_id]);
    state.handle_action(Action::DuplicateSelection);

    let frame = state.boards.active_frame();
    assert_eq!(frame.shapes.len(), 3, "only one duplicate should be added");
    assert!(
        frame.shape(locked_id).unwrap().locked,
        "locked shape should remain locked"
    );
}

#[test]
fn copy_selection_of_only_locked_shapes_leaves_clipboard_empty() {
    let mut state = create_test_input_state();
    let locked_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 5,
        y: 5,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let locked_index = state
        .boards
        .active_frame()
        .find_index(locked_id)
        .expect("locked index");
    state.boards.active_frame_mut().shapes[locked_index].locked = true;
    state.set_selection(vec![locked_id]);

    assert_eq!(state.copy_selection(), 0);
    assert!(state.selection_clipboard_is_empty());
}

#[test]
fn repeated_paste_selection_uses_increasing_offsets() {
    let mut state = create_test_input_state();
    let original_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 30,
        h: 40,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![original_id]);
    assert_eq!(state.copy_selection(), 1);

    assert_eq!(state.paste_selection(), 1);
    assert_eq!(state.paste_selection(), 1);

    let frame = state.boards.active_frame();
    assert_eq!(frame.shapes.len(), 3);
    let coords = frame
        .shapes
        .iter()
        .map(|shape| match &shape.shape {
            Shape::Rect { x, y, .. } => (*x, *y),
            _ => panic!("expected rectangles"),
        })
        .collect::<Vec<_>>();
    assert_eq!(coords, vec![(10, 20), (22, 32), (34, 44)]);
}

#[test]
fn paste_selection_warns_when_shape_limit_prevents_any_paste() {
    let mut state = create_test_input_state();
    let original_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 30,
        h: 40,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![original_id]);
    assert_eq!(state.copy_selection(), 1);
    state.max_shapes_per_frame = 1;

    assert_eq!(state.paste_selection(), 0);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Shape limit reached; nothing pasted.")
    );
}

#[test]
fn paste_selection_warns_when_shape_limit_allows_only_partial_paste() {
    let mut state = create_test_input_state();
    let first = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 0,
        y: 0,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let second = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 20,
        y: 20,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![first, second]);
    assert_eq!(state.copy_selection(), 2);
    state.max_shapes_per_frame = 3;

    assert_eq!(state.paste_selection(), 1);
    assert_eq!(state.boards.active_frame().shapes.len(), 3);
    assert_eq!(state.selected_shape_ids().len(), 1);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Shape limit reached; pasted 1 of 2.")
    );
}
