use super::*;
use crate::runtime_ui_state::{InteractionSeedTarget, InteractionSeedValue};

#[test]
fn unsupported_version_is_envelope_only() {
    let decoded = decode_runtime_ui_file(
        br#"version = 42
toolbar = "intentionally malformed for V1"
future = { nested = true }
"#,
    );
    assert_eq!(
        decoded.status,
        RuntimeUiFileStatus::UnsupportedReadOnly { version: Some(42) }
    );
    assert_eq!(decoded.envelope, RuntimeStateObservedEnvelope::Version(42));
    assert!(decoded.supported_wire.is_none());
}

#[test]
fn supported_unknown_fields_round_trip_semantically() {
    let source = br#"
version = 1
future_root = { answer = 42 }

[toolbar]
future_toolbar = ["a", "b"]

[toolbar.top_pinned]
seed = false
value = true
future_entry = { retained = true }

[boards]
future_boards = "kept"
"#;
    let decoded = decode_runtime_ui_file(source);
    assert_eq!(decoded.status, RuntimeUiFileStatus::Supported);
    let wire = decoded.supported_wire.expect("supported wire");
    assert_eq!(
        wire.model
            .get(&InteractionSeedTarget::TopPinned)
            .map(|entry| &entry.value),
        Some(&InteractionSeedValue::Bool(true))
    );

    let encoded = encode_runtime_ui_file(&wire).expect("encode");
    let reparsed = decode_runtime_ui_file(&encoded);
    assert_eq!(reparsed.status, RuntimeUiFileStatus::Supported);
    assert_eq!(reparsed.supported_wire, Some(wire));
}

#[test]
fn unknown_ids_and_unknown_order_items_are_pruned() {
    let source = br#"
version = 1

[toolbar.collapsed_sections.future-section]
seed = false
value = true

[toolbar.item_visibility.future-item]
seed = "default"
value = "hidden"

[toolbar.item_order.top_tools]
seed = ["top.tool.pen", "future-tool"]
value = ["future-tool", "top.tool.marker"]
"#;
    let decoded = decode_runtime_ui_file(source);
    assert_eq!(decoded.status, RuntimeUiFileStatus::Supported);
    let wire = decoded.supported_wire.expect("wire");
    assert_eq!(wire.model.iter().count(), 1);
    let encoded = String::from_utf8(encode_runtime_ui_file(&wire).unwrap()).unwrap();
    assert!(!encoded.contains("future-section"));
    assert!(!encoded.contains("future-item"));
    assert!(!encoded.contains("future-tool"));
    assert!(encoded.contains("top.tool.pen"));
    assert!(encoded.contains("top.tool.marker"));
}

#[test]
fn malformed_recognized_entry_invalidates_file() {
    let decoded = decode_runtime_ui_file(
        br#"version = 1
[toolbar.top_pinned]
seed = false
value = "yes"
"#,
    );
    assert_eq!(decoded.status, RuntimeUiFileStatus::Invalid);
    assert!(decoded.supported_wire.is_none());
}

#[test]
fn malformed_file_without_version_is_invalid() {
    let decoded = decode_runtime_ui_file(b"not = [valid");
    assert_eq!(decoded.status, RuntimeUiFileStatus::Invalid);
    assert_eq!(
        decoded.envelope,
        RuntimeStateObservedEnvelope::PresentWithoutReadableVersion
    );
}

#[test]
fn every_v1_override_shape_round_trips() {
    let source = br#"version = 1

[toolbar.side_pane]
seed = "draw"
value = "canvas"

[toolbar.collapsed_sections.colors]
seed = false
value = true

[toolbar.item_visibility."top.tool.pen"]
seed = "default"
value = "hidden"

[toolbar.item_order.top_tools]
seed = ["top.tool.pen", "top.tool.marker"]
value = ["top.tool.marker", "top.tool.pen"]

[boards.pinned.board-1]
seed = false
value = true
"#;
    let decoded = decode_runtime_ui_file(source);
    assert_eq!(decoded.status, RuntimeUiFileStatus::Supported);
    let wire = decoded.supported_wire.unwrap();
    assert_eq!(wire.model.iter().count(), 5);
    let encoded = encode_runtime_ui_file(&wire).unwrap();
    let reparsed = decode_runtime_ui_file(&encoded);
    assert_eq!(reparsed.supported_wire, Some(wire));
}

#[test]
fn invalid_version_values_are_not_treated_as_supported_or_downgradable() {
    for source in [
        b"version = -1\n".as_slice(),
        b"version = '2'\n".as_slice(),
        b"toolbar = {}\n".as_slice(),
    ] {
        let decoded = decode_runtime_ui_file(source);
        assert_eq!(decoded.status, RuntimeUiFileStatus::Invalid);
        assert!(decoded.supported_wire.is_none());
    }
}

#[test]
fn duplicate_normalized_recognized_ids_are_invalid() {
    let decoded = decode_runtime_ui_file(
        br#"version = 1

[toolbar.collapsed_sections.colors]
seed = false
value = true

[toolbar.collapsed_sections."COLORS"]
seed = true
value = false
"#,
    );
    assert_eq!(decoded.status, RuntimeUiFileStatus::Invalid);
}
