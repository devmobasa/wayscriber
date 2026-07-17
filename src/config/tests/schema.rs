use super::super::*;

#[test]
fn json_schema_includes_expected_sections() {
    let schema = Config::json_schema();
    let properties = schema
        .get("properties")
        .and_then(|value| value.as_object())
        .expect("schema should contain properties object");

    for key in [
        "drawing",
        "history",
        "arrow",
        "performance",
        "ui",
        "render_profiles",
        "boards",
        "board",
        "keybindings",
        "capture",
        "export",
        "session",
    ] {
        assert!(properties.contains_key(key), "missing schema field {key}");
    }
}

#[test]
fn performance_metadata_paths_exist_in_json_schema() {
    let schema = Config::json_schema();
    for metadata in PERFORMANCE_FIELD_METADATA {
        assert!(
            schema_contains_path(&schema, metadata.path),
            "schema missing metadata path {}",
            metadata.path
        );
    }
}

fn schema_contains_path(schema: &serde_json::Value, path: &str) -> bool {
    let mut node = schema;
    for segment in path.split('.') {
        node = resolve_schema_ref(schema, node);
        let Some(property) = node
            .get("properties")
            .and_then(|properties| properties.get(segment))
        else {
            return false;
        };
        node = property;
    }
    true
}

fn resolve_schema_ref<'a>(
    schema: &'a serde_json::Value,
    node: &'a serde_json::Value,
) -> &'a serde_json::Value {
    let Some(reference) = node.get("$ref").and_then(|value| value.as_str()) else {
        return node;
    };
    reference
        .strip_prefix('#')
        .and_then(|pointer| schema.pointer(pointer))
        .unwrap_or(node)
}
