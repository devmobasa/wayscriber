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
        "boards",
        "board",
        "keybindings",
        "capture",
        "session",
    ] {
        assert!(properties.contains_key(key), "missing schema field {key}");
    }
}
