use super::*;
use crate::runtime_ui_state::{
    InteractionSeedTarget, InteractionSeedValue, PersistedTopDisplayMode, ToolbarPositionSeed,
};

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

/// A build that does not know a key preserves it verbatim. If a later build
/// learns to manage that key, the preserved copy and the managed value collide
/// while encoding. Failing there made the whole file unencodable, and because
/// the conflict lives in memory rather than on disk the persistence incident
/// recurred on every retry - no toolbar state could ever be saved again. The
/// managed value has to win instead.
#[test]
fn a_preserved_key_this_build_now_manages_loses_to_the_managed_value() {
    let decoded = decode_runtime_ui_file(
        br#"
version = 1

[toolbar.top_pinned]
seed = false
value = true
"#,
    );
    let mut wire = decoded.supported_wire.expect("supported wire");

    // Simulate the upgrade: a preserved copy of a key this build manages,
    // as if an older build had carried it forward before the key was known.
    wire.passthrough.toolbar.insert(
        "top_pinned".to_string(),
        "preserved = \"stale\"\n".to_string(),
    );

    let encoded = encode_runtime_ui_file(&wire).expect("a collision must not fail the encode");
    let reparsed = decode_runtime_ui_file(&encoded);
    assert_eq!(reparsed.status, RuntimeUiFileStatus::Supported);
    assert_eq!(
        reparsed
            .supported_wire
            .expect("supported wire")
            .model
            .get(&InteractionSeedTarget::TopPinned)
            .map(|entry| &entry.value),
        Some(&InteractionSeedValue::Bool(true)),
        "the managed value survives, not the stale preserved copy"
    );
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

[toolbar.top_position]
seed = { x = 0.0, y = 0.0 }
value = { x = -12.5, y = 48.0 }

[toolbar.side_position]
seed = { x = 1.0, y = 2.0 }
value = { x = 3.25, y = -4.5 }

[toolbar.top_display_mode]
seed = "full"
value = "micro"

[boards.pinned.board-1]
seed = false
value = true
"#;
    let decoded = decode_runtime_ui_file(source);
    assert_eq!(decoded.status, RuntimeUiFileStatus::Supported);
    let wire = decoded.supported_wire.unwrap();
    assert_eq!(wire.model.iter().count(), 8);
    let encoded = encode_runtime_ui_file(&wire).unwrap();
    let reparsed = decode_runtime_ui_file(&encoded);
    assert_eq!(reparsed.supported_wire, Some(wire));
}

#[test]
fn position_and_display_mode_overrides_decode_to_their_typed_values() {
    let source = br#"version = 1

[toolbar.top_position]
seed = { x = 0.0, y = 0.0 }
value = { x = -12.5, y = 48.0 }

[toolbar.side_position]
seed = { x = 1.0, y = 2.0 }
value = { x = 3.25, y = -4.5 }

[toolbar.top_display_mode]
seed = "full"
value = "micro"
"#;
    let wire = decode_runtime_ui_file(source)
        .supported_wire
        .expect("supported wire");
    assert_eq!(
        wire.model
            .get(&InteractionSeedTarget::TopPosition)
            .map(|entry| &entry.value),
        Some(&InteractionSeedValue::Position(
            ToolbarPositionSeed::new(-12.5, 48.0).unwrap()
        ))
    );
    assert_eq!(
        wire.model
            .get(&InteractionSeedTarget::SidePosition)
            .map(|entry| &entry.seed),
        Some(&InteractionSeedValue::Position(
            ToolbarPositionSeed::new(1.0, 2.0).unwrap()
        ))
    );
    assert_eq!(
        wire.model
            .get(&InteractionSeedTarget::TopDisplayMode)
            .map(|entry| &entry.value),
        Some(&InteractionSeedValue::TopDisplayMode(
            PersistedTopDisplayMode::Micro
        ))
    );
}

#[test]
fn a_file_without_the_added_toolbar_keys_still_decodes() {
    // The new keys are optional additions to V1, so a file written before
    // they existed stays supported and writable.
    let decoded = decode_runtime_ui_file(
        br#"version = 1

[toolbar.top_pinned]
seed = false
value = true
"#,
    );
    assert_eq!(decoded.status, RuntimeUiFileStatus::Supported);
    let wire = decoded.supported_wire.expect("supported wire");
    assert_eq!(wire.model.iter().count(), 1);
    assert!(
        wire.model
            .get(&InteractionSeedTarget::TopPosition)
            .is_none()
    );
    assert!(
        wire.model
            .get(&InteractionSeedTarget::TopDisplayMode)
            .is_none()
    );
}

#[test]
fn unknown_keys_inside_the_added_entries_survive_a_rewrite() {
    let source = br#"version = 1

[toolbar.top_position]
seed = { x = 0.0, y = 0.0 }
value = { x = 4.0, y = 5.0 }
future_entry = { retained = true }

[toolbar.top_display_mode]
seed = "full"
value = "micro"
future_scalar = 7
"#;
    let wire = decode_runtime_ui_file(source)
        .supported_wire
        .expect("supported wire");
    let encoded = encode_runtime_ui_file(&wire).expect("encode");
    let text = String::from_utf8(encoded.clone()).unwrap();
    assert!(text.contains("future_entry"));
    assert!(text.contains("future_scalar"));
    assert_eq!(decode_runtime_ui_file(&encoded).supported_wire, Some(wire));
}

#[test]
fn malformed_position_and_display_mode_values_invalidate_the_file() {
    for source in [
        // Non-finite offsets cannot be compared bit-exactly against a seed.
        br#"version = 1
[toolbar.top_position]
seed = { x = 0.0, y = 0.0 }
value = { x = nan, y = 1.0 }
"#
        .as_slice(),
        br#"version = 1
[toolbar.top_position]
seed = { x = 0.0, y = 0.0 }
value = { x = 1.0 }
"#
        .as_slice(),
        br#"version = 1
[toolbar.side_position]
seed = { x = 0.0, y = 0.0 }
value = { x = 1.0, y = 2.0, z = 3.0 }
"#
        .as_slice(),
        br#"version = 1
[toolbar.top_position]
seed = { x = 0.0, y = 0.0 }
value = "somewhere"
"#
        .as_slice(),
        // `hidden` is a runtime-only rung and is never persisted.
        br#"version = 1
[toolbar.top_display_mode]
seed = "full"
value = "hidden"
"#
        .as_slice(),
    ] {
        let decoded = decode_runtime_ui_file(source);
        assert_eq!(
            decoded.status,
            RuntimeUiFileStatus::Invalid,
            "{}",
            String::from_utf8_lossy(source)
        );
        assert!(decoded.supported_wire.is_none());
    }
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
