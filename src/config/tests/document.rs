use super::super::*;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TempConfig {
    root: PathBuf,
    path: PathBuf,
}

impl TempConfig {
    fn new(name: &str) -> Self {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "wayscriber-config-document-{}-{sequence}-{name}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create temporary config directory");
        let path = root.join("config.toml");
        Self { root, path }
    }

    fn write(&self, contents: &str) {
        fs::write(&self.path, contents).expect("write temporary config");
    }
}

impl Drop for TempConfig {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn diagnostic_paths(document: &ConfigDocument) -> Vec<&str> {
    document
        .diagnostics()
        .iter()
        .map(ConfigDiagnostic::path)
        .collect()
}

#[test]
fn document_save_preserves_comments_order_unknowns_and_validates_known_values() {
    let temp = TempConfig::new("golden");
    temp.write(
        r#"# user header
future_root = "keep" # future root inline

[performance] # performance header
buffer_count = 99 # keep buffer explanation
enable_vsync = false
max_fps_no_vsync = 120
ui_animation_fps = 999 # clamp this known value
future_knob = 7 # preserve nested unknown

# user trailing comment
"#,
    );

    let document = ConfigDocument::load_from_path(&temp.path).expect("load document");
    let paths = diagnostic_paths(&document);
    assert!(paths.iter().any(|path| path.ends_with("future_root")));
    assert!(
        paths
            .iter()
            .any(|path| path.ends_with("performance.future_knob"))
    );
    assert_eq!(document.config().performance.buffer_count, 4);
    assert_eq!(document.config().performance.ui_animation_fps, 240);

    let mut updated = document.config().clone();
    updated.performance.max_fps_no_vsync = 144;
    let outcome = document
        .save_with_backup(updated)
        .expect("save merged document");
    let saved = fs::read_to_string(&temp.path).expect("read merged document");

    for preserved in [
        "# user header",
        "future_root = \"keep\" # future root inline",
        "[performance] # performance header",
        "# keep buffer explanation",
        "future_knob = 7 # preserve nested unknown",
        "# user trailing comment",
    ] {
        assert!(
            saved.contains(preserved),
            "missing preserved text: {preserved}"
        );
    }
    assert!(saved.find("future_root").unwrap() < saved.find("[performance]").unwrap());
    assert!(saved.contains("buffer_count = 4 # keep buffer explanation"));
    assert!(saved.contains("max_fps_no_vsync = 144"));
    assert!(saved.contains("ui_animation_fps = 240 # clamp this known value"));
    assert_eq!(
        diagnostic_paths(outcome.document()),
        diagnostic_paths(&document)
    );
}

#[test]
fn document_save_removes_omitted_known_option_without_removing_unknown_neighbor() {
    let temp = TempConfig::new("optional");
    temp.write(
        r#"[ui]
preferred_output = "DP-1"
future_output_policy = "keep"
"#,
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load document");
    let mut updated = document.config().clone();
    updated.ui.preferred_output = None;

    document
        .save_with_backup(updated)
        .expect("save optional removal");
    let saved = fs::read_to_string(&temp.path).expect("read saved config");
    assert!(!saved.contains("preferred_output"));
    assert!(saved.contains("future_output_policy = \"keep\""));
}

#[test]
fn document_load_and_save_tolerates_future_keys_in_strict_export_tables() {
    let temp = TempConfig::new("future-export-keys");
    let original = r#"config_revision = 1
[export]
future_format = "svg"

[export.pdf]
page_size = "a4"
future_bleed = 12.5

[export.pdf.labels]
enabled = true
future_font_weight = 600
"#;
    temp.write(original);

    let document = ConfigDocument::load_from_path(&temp.path)
        .expect("future export settings remain editor-compatible");
    let paths = diagnostic_paths(&document);
    for expected in [
        "export.future_format",
        "export.pdf.future_bleed",
        "export.pdf.labels.future_font_weight",
    ] {
        assert!(
            paths.iter().any(|path| path.ends_with(expected)),
            "missing diagnostic for {expected}: {paths:?}"
        );
    }

    document
        .save_with_backup(document.config().clone())
        .expect("save config with future export settings");

    assert_eq!(fs::read_to_string(&temp.path).unwrap(), original);
}

#[test]
fn no_op_save_removes_known_option_discarded_by_validation() {
    let temp = TempConfig::new("validated-away-known-option");
    temp.write(
        r#"config_revision = 1
[render_profiles]
active = "missing"
future_profile_policy = "keep"
"#,
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load render profiles");
    assert!(document.config().render_profiles.active.is_none());

    document
        .save_with_backup(document.config().clone())
        .expect("save validated render profiles");

    let saved = fs::read_to_string(&temp.path).expect("read validated render profiles");
    assert!(!saved.contains("active ="));
    assert!(saved.contains("future_profile_policy = \"keep\""));
}

#[test]
fn no_op_save_does_not_materialize_omitted_defaults() {
    let temp = TempConfig::new("omitted-defaults");
    let original =
        "config_revision = 1\n# intentionally sparse\n[performance]\nmax_fps_no_vsync = 120\n";
    temp.write(original);
    let document = ConfigDocument::load_from_path(&temp.path).expect("load sparse document");

    document
        .save_with_backup(document.config().clone())
        .expect("save sparse document without changes");

    assert_eq!(
        fs::read_to_string(&temp.path).expect("read sparse document"),
        original
    );
}

#[test]
fn changing_an_omitted_value_inserts_only_that_value() {
    let temp = TempConfig::new("sparse-change");
    temp.write("# intentionally sparse\n");
    let document = ConfigDocument::load_from_path(&temp.path).expect("load sparse document");
    let mut updated = document.config().clone();
    updated.performance.max_fps_no_vsync = 144;

    document
        .save_with_backup(updated)
        .expect("save one change to sparse document");

    let saved = fs::read_to_string(&temp.path).expect("read sparse document");
    assert!(saved.contains("# intentionally sparse"));
    assert!(saved.contains("[performance]"));
    assert!(saved.contains("max_fps_no_vsync = 144"));
    assert!(saved.find("# intentionally sparse").unwrap() < saved.find("[performance]").unwrap());
    assert!(!saved.contains("buffer_count"));
    assert!(!saved.contains("[drawing]"));
    assert!(!saved.contains("[session]"));
}

#[test]
fn first_save_for_missing_config_stays_sparse() {
    let temp = TempConfig::new("missing-sparse");
    let document = ConfigDocument::load_from_path(&temp.path).expect("load missing document");

    document
        .save_with_backup(document.config().clone())
        .expect("save missing document");

    let saved = fs::read_to_string(&temp.path).expect("read newly created document");
    assert_eq!(
        saved,
        format!("config_revision = {CURRENT_CONFIG_REVISION}\n")
    );
}

#[test]
fn editing_load_can_repair_typed_parse_failure_without_losing_unknown_keys() {
    let temp = TempConfig::new("repair-invalid-config");
    let original = r#"future_root = "preserve me"
[performance]
buffer_count = "not a number"
future_knob = 17
"#;
    temp.write(original);

    assert!(ConfigDocument::load_from_path(&temp.path).is_err());
    let (document, warning) =
        ConfigDocument::load_for_editing_from_path(&temp.path).expect("load repairable document");
    let warning = warning.expect("repair warning");
    assert!(warning.contains("Failed to parse config"));

    let outcome = document
        .save_with_backup(document.config().clone())
        .expect("repair invalid config");
    let saved = fs::read_to_string(&temp.path).expect("read repaired config");
    assert!(saved.contains("future_root = \"preserve me\""));
    assert!(saved.contains("future_knob = 17"));
    assert!(!saved.contains("buffer_count"));
    assert!(saved.contains("config_revision = 1"));
    let backup = outcome.backup_path().expect("repair backup");
    assert_eq!(fs::read_to_string(backup).unwrap(), original);
    ConfigDocument::load_from_path(&temp.path).expect("repaired config is valid");
}

#[test]
fn editing_load_can_repair_malformed_toml_with_a_backup() {
    let temp = TempConfig::new("repair-malformed-config");
    let original = "[performance\nmax_fps_no_vsync = 144\n";
    temp.write(original);

    let (document, warning) = ConfigDocument::load_for_editing_from_path(&temp.path)
        .expect("load malformed repair document");
    assert!(warning.is_some());
    let outcome = document
        .save_with_backup(document.config().clone())
        .expect("repair malformed config");

    assert_eq!(
        fs::read_to_string(&temp.path).unwrap(),
        format!("config_revision = {CURRENT_CONFIG_REVISION}\n")
    );
    assert_eq!(
        fs::read_to_string(outcome.backup_path().expect("repair backup")).unwrap(),
        original
    );
}

#[test]
fn repair_mode_removes_invalid_known_collections_but_keeps_root_unknowns() {
    let temp = TempConfig::new("repair-invalid-collection");
    let original = r#"config_revision = 1
future_root = "preserve me"

[drawing]
future_drawing_option = true

[[drawing.quick_colors]]
label = "Invalid"
color = 42
future_entry_option = "cannot be separated safely"
"#;
    temp.write(original);

    let (document, warning) = ConfigDocument::load_for_editing_from_path(&temp.path)
        .expect("load collection repair document");
    assert!(warning.is_some());
    document
        .save_with_backup(document.config().clone())
        .expect("repair invalid collection");

    let saved = fs::read_to_string(&temp.path).unwrap();
    assert!(saved.contains("future_root = \"preserve me\""));
    assert!(!saved.contains("future_drawing_option"));
    assert!(!saved.contains("quick_colors"));
    assert!(!saved.contains("future_entry_option"));
    ConfigDocument::load_from_path(&temp.path).expect("collection repair is valid");
}

#[test]
fn save_persists_migration_revision_and_does_not_repeat_keybinding_migration() {
    let temp = TempConfig::new("migration-revision");
    temp.write(
        r#"[keybindings]
toggle_command_palette = ["Ctrl+K"]
capture_full_screen = ["Ctrl+Shift+P"]
"#,
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load legacy shortcuts");
    let mut updated = document.config().clone();
    updated.keybindings.ui.toggle_command_palette = vec!["Ctrl+K".to_string()];
    updated.keybindings.capture.capture_full_screen = vec!["Ctrl+Shift+P".to_string()];
    document
        .save_with_backup(updated)
        .expect("save intentional legacy shortcut pair");

    let saved = fs::read_to_string(&temp.path).expect("read migrated config");
    assert!(saved.contains(&format!("config_revision = {CURRENT_CONFIG_REVISION}")));
    let reloaded = ConfigDocument::load_from_path(&temp.path).expect("reload migrated config");
    assert_eq!(
        reloaded.config().keybindings.ui.toggle_command_palette,
        ["Ctrl+K"]
    );
    assert_eq!(
        reloaded.config().keybindings.capture.capture_full_screen,
        ["Ctrl+Shift+P"]
    );
}

#[test]
fn inline_array_of_structs_preserves_representation_and_unknown_fields() {
    let temp = TempConfig::new("inline-struct-array");
    temp.write(
        r#"config_revision = 1
boards = { max_count = 2, default_board = "transparent", items = [{ id = "transparent", name = "Overlay", background = "transparent", future_owner = "keep" }] }
"#,
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load inline boards");
    document
        .save_with_backup(document.config().clone())
        .expect("save inline boards");

    let saved = fs::read_to_string(&temp.path).expect("read inline boards");
    assert!(saved.contains("boards = {"));
    assert!(!saved.contains("[[boards.items]]"));
    assert!(saved.contains("future_owner = \"keep\""));
    toml::from_str::<Config>(&saved).expect("inline representation remains valid");
}

#[test]
fn no_op_save_preserves_semantically_equal_scalar_formatting() {
    let temp = TempConfig::new("scalar-formatting");
    let original = "config_revision = 1\n[performance]\nmax_fps_no_vsync = 1_200\n";
    temp.write(original);
    let document = ConfigDocument::load_from_path(&temp.path).expect("load precise scalar");

    document
        .save_with_backup(document.config().clone())
        .expect("save precise scalar");

    assert_eq!(fs::read_to_string(&temp.path).unwrap(), original);
}

#[test]
fn no_op_save_preserves_integer_spelling_for_float_fields() {
    let temp = TempConfig::new("integer-float-spelling");
    let original = "config_revision = 1\n[drawing]\ndefault_thickness = 2\n";
    temp.write(original);
    let document = ConfigDocument::load_from_path(&temp.path).expect("load integer-form float");

    document
        .save_with_backup(document.config().clone())
        .expect("save integer-form float without changes");

    assert_eq!(fs::read_to_string(&temp.path).unwrap(), original);
}

#[test]
fn document_save_canonicalizes_key_aliases_and_preserves_their_comments() {
    let temp = TempConfig::new("aliases");
    temp.write(
        r#"[ui]
# floating badge alias comment
show_page_badge_with_status_bar = true
show_status_bar = false
show_frozen_badge = true

[ui.toolbar.mode_overrides.full]
# regular layout alias comment
show_presets = true

[[render_profiles.items]]
# profile alias comment
id = "one"
name = "One"
future_profile_key = "keep"
"#,
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load aliases");
    document
        .save_with_backup(document.config().clone())
        .expect("save canonical aliases");
    let saved = fs::read_to_string(&temp.path).expect("read canonical aliases");

    assert!(!saved.contains("show_page_badge_with_status_bar"));
    assert!(saved.contains("show_floating_badge_always = true"));
    assert!(
        saved.find("show_floating_badge_always").unwrap() < saved.find("show_status_bar").unwrap()
    );
    assert!(saved.find("show_status_bar").unwrap() < saved.find("show_frozen_badge").unwrap());
    assert!(!saved.contains("[ui.toolbar.mode_overrides.full]"));
    assert!(saved.contains("[ui.toolbar.mode_overrides.regular]"));
    assert!(!saved.contains("[[render_profiles.items]]"));
    assert!(saved.contains("[[render_profiles.profiles]]"));
    for comment in [
        "# floating badge alias comment",
        "# regular layout alias comment",
        "# profile alias comment",
    ] {
        assert!(saved.contains(comment));
    }
    assert!(saved.contains("future_profile_key = \"keep\""));
    toml::from_str::<Config>(&saved).expect("canonical output parses exactly once");
}

#[test]
fn document_save_keeps_unknown_fields_with_stable_id_when_tables_reorder() {
    let temp = TempConfig::new("stable-id");
    temp.write(
        r##"[[render_profiles.profiles]]
id = "a"
name = "A"
future_owner = "owner-a"

[[render_profiles.profiles]]
id = "b"
name = "B"
future_owner = "owner-b"
"##,
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load profiles");
    let mut updated = document.config().clone();
    updated.render_profiles.profiles.swap(0, 1);
    document
        .save_with_backup(updated)
        .expect("save reordered profiles");

    let saved = fs::read_to_string(&temp.path).expect("read reordered profiles");
    let value: toml::Value = toml::from_str(&saved).expect("parse reordered profiles");
    let profiles = value["render_profiles"]["profiles"]
        .as_array()
        .expect("profiles array");
    assert_eq!(profiles[0]["id"].as_str(), Some("b"));
    assert_eq!(profiles[0]["future_owner"].as_str(), Some("owner-b"));
    assert_eq!(profiles[1]["id"].as_str(), Some("a"));
    assert_eq!(profiles[1]["future_owner"].as_str(), Some("owner-a"));
}

#[test]
fn no_op_save_preserves_separated_array_table_positions() {
    let temp = TempConfig::new("separated-array-table-positions");
    let original = r#"config_revision = 1
[[render_profiles.profiles]]
id = "first"
name = "First"

[performance]
max_fps_no_vsync = 144

[[render_profiles.profiles]]
id = "second"
name = "Second"
"#;
    temp.write(original);
    let document = ConfigDocument::load_from_path(&temp.path).expect("load separated profiles");

    document
        .save_with_backup(document.config().clone())
        .expect("save separated profiles without changes");

    assert_eq!(fs::read_to_string(&temp.path).unwrap(), original);
}

#[test]
fn no_op_save_preserves_separated_nested_array_table_positions() {
    let temp = TempConfig::new("separated-nested-array-table-positions");
    let original = r##"config_revision = 1
[[render_profiles.profiles]]
id = "first"
name = "First"

[performance]
max_fps_no_vsync = 144

[[render_profiles.profiles.mappings]]
from = "#111111"
to = "#AAAAAA"
"##;
    temp.write(original);
    let document = ConfigDocument::load_from_path(&temp.path).expect("load separated mapping");

    document
        .save_with_backup(document.config().clone())
        .expect("save separated mapping without changes");

    assert_eq!(fs::read_to_string(&temp.path).unwrap(), original);
}

#[test]
fn adding_nested_array_table_keeps_it_with_the_edited_parent() {
    let temp = TempConfig::new("added-nested-array-table-position");
    temp.write(
        r#"config_revision = 1
[[render_profiles.profiles]]
id = "first"
name = "First"

[performance]
max_fps_no_vsync = 144

[[render_profiles.profiles]]
id = "second"
name = "Second"
"#,
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load separated profiles");
    let mut updated = document.config().clone();
    updated.render_profiles.profiles[0]
        .mappings
        .push(RenderColorMappingConfig {
            from: "#111111".to_string(),
            to: "#AAAAAA".to_string(),
        });

    document
        .save_with_backup(updated)
        .expect("save added nested mapping");

    let saved = fs::read_to_string(&temp.path).expect("read profiles with nested mapping");
    let value: toml::Value = toml::from_str(&saved).expect("parse profiles with nested mapping");
    let profiles = value["render_profiles"]["profiles"]
        .as_array()
        .expect("profiles array");
    assert_eq!(profiles[0]["mappings"].as_array().unwrap().len(), 1);
    assert!(profiles[1].get("mappings").is_none());
}

#[test]
fn profile_ids_that_differ_by_non_ascii_case_keep_distinct_metadata() {
    let temp = TempConfig::new("non-ascii-stable-id");
    temp.write(
        r#"[[render_profiles.profiles]]
id = "Ä"
name = "Upper"
future_owner = "owner-upper"

[[render_profiles.profiles]]
id = "ä"
name = "Lower"
future_owner = "owner-lower"
"#,
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load profiles");
    let mut updated = document.config().clone();
    updated.render_profiles.profiles.swap(0, 1);
    document
        .save_with_backup(updated)
        .expect("save reordered profiles");

    let saved = fs::read_to_string(&temp.path).expect("read reordered profiles");
    let value: toml::Value = toml::from_str(&saved).expect("parse reordered profiles");
    let profiles = value["render_profiles"]["profiles"]
        .as_array()
        .expect("profiles array");
    assert_eq!(profiles[0]["id"].as_str(), Some("ä"));
    assert_eq!(profiles[0]["future_owner"].as_str(), Some("owner-lower"));
    assert_eq!(profiles[1]["id"].as_str(), Some("Ä"));
    assert_eq!(profiles[1]["future_owner"].as_str(), Some("owner-upper"));
}

#[test]
fn deduplicated_profile_ids_keep_entry_metadata_when_reordered() {
    let temp = TempConfig::new("deduplicated-stable-id");
    temp.write(
        r#"[[render_profiles.profiles]]
id = "duplicate"
name = "First"
future_owner = "owner-first"

[[render_profiles.profiles]]
id = "duplicate"
name = "Second"
future_owner = "owner-second"
"#,
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load profiles");
    assert_eq!(
        document.config().render_profiles.profiles[0].id,
        "duplicate"
    );
    assert_eq!(
        document.config().render_profiles.profiles[1].id,
        "duplicate-2"
    );

    let mut updated = document.config().clone();
    updated.render_profiles.profiles.swap(0, 1);
    document
        .save_with_backup(updated)
        .expect("save reordered deduplicated profiles");

    let saved = fs::read_to_string(&temp.path).expect("read reordered profiles");
    let value: toml::Value = toml::from_str(&saved).expect("parse reordered profiles");
    let profiles = value["render_profiles"]["profiles"]
        .as_array()
        .expect("profiles array");
    assert_eq!(profiles[0]["id"].as_str(), Some("duplicate-2"));
    assert_eq!(profiles[0]["future_owner"].as_str(), Some("owner-second"));
    assert_eq!(profiles[1]["id"].as_str(), Some("duplicate"));
    assert_eq!(profiles[1]["future_owner"].as_str(), Some("owner-first"));
}

#[test]
fn nested_array_tables_stay_with_their_parent_after_reorder() {
    let temp = TempConfig::new("nested-array-table-position");
    temp.write(
        r##"config_revision = 1
[[render_profiles.profiles]]
id = "a"
name = "A"

[[render_profiles.profiles.mappings]]
from = "#111111"
to = "#AAAAAA"
future_owner = "mapping-a"

[[render_profiles.profiles]]
id = "b"
name = "B"

[[render_profiles.profiles.mappings]]
from = "#222222"
to = "#BBBBBB"
future_owner = "mapping-b"
"##,
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load nested profiles");
    let mut updated = document.config().clone();
    updated.render_profiles.profiles.swap(0, 1);
    document
        .save_with_backup(updated)
        .expect("save reordered nested profiles");

    let saved = fs::read_to_string(&temp.path).expect("read nested profiles");
    let value: toml::Value = toml::from_str(&saved).expect("parse nested profiles");
    let profiles = value["render_profiles"]["profiles"].as_array().unwrap();
    assert_eq!(profiles[0]["id"].as_str(), Some("b"));
    assert_eq!(
        profiles[0]["mappings"][0]["future_owner"].as_str(),
        Some("mapping-b")
    );
    assert_eq!(profiles[1]["id"].as_str(), Some("a"));
    assert_eq!(
        profiles[1]["mappings"][0]["future_owner"].as_str(),
        Some("mapping-a")
    );
}

#[test]
fn removing_stable_id_entry_keeps_metadata_with_retained_id() {
    let temp = TempConfig::new("removed-stable-id");
    temp.write(
        r#"[[render_profiles.profiles]]
id = "a"
name = "A"
future_owner = "owner-a"

[[render_profiles.profiles]]
id = "b"
name = "B"
future_owner = "owner-b"
"#,
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load profiles");
    let mut updated = document.config().clone();
    updated.render_profiles.profiles.remove(0);
    document
        .save_with_backup(updated)
        .expect("save profiles after removal");

    let saved = fs::read_to_string(&temp.path).expect("read profiles after removal");
    let value: toml::Value = toml::from_str(&saved).expect("parse profiles after removal");
    let profiles = value["render_profiles"]["profiles"]
        .as_array()
        .expect("profiles array");
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0]["id"].as_str(), Some("b"));
    assert_eq!(profiles[0]["future_owner"].as_str(), Some("owner-b"));
}

#[test]
fn adding_stable_id_entry_does_not_shift_retained_metadata() {
    let temp = TempConfig::new("added-stable-id");
    temp.write(
        r#"[[render_profiles.profiles]]
id = "a"
name = "A"
future_owner = "owner-a"

[[render_profiles.profiles]]
id = "b"
name = "B"
future_owner = "owner-b"
"#,
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load profiles");
    let mut updated = document.config().clone();
    let mut added = updated.render_profiles.profiles[0].clone();
    added.id = "new".to_string();
    added.name = "New".to_string();
    updated.render_profiles.profiles.insert(0, added);
    document
        .save_with_backup(updated)
        .expect("save profiles after insertion");

    let saved = fs::read_to_string(&temp.path).expect("read profiles after insertion");
    let value: toml::Value = toml::from_str(&saved).expect("parse profiles after insertion");
    let profiles = value["render_profiles"]["profiles"]
        .as_array()
        .expect("profiles array");
    assert_eq!(profiles.len(), 3);
    assert_eq!(profiles[0]["id"].as_str(), Some("new"));
    assert!(profiles[0].get("future_owner").is_none());
    assert_eq!(profiles[1]["id"].as_str(), Some("a"));
    assert_eq!(profiles[1]["future_owner"].as_str(), Some("owner-a"));
    assert_eq!(profiles[2]["id"].as_str(), Some("b"));
    assert_eq!(profiles[2]["future_owner"].as_str(), Some("owner-b"));
}

#[test]
fn unknown_diagnostics_cover_flattened_and_array_of_table_paths() {
    let temp = TempConfig::new("unknown-path-depth");
    temp.write(
        r#"[keybindings]
future_action = ["Ctrl+Alt+F24"]

[[render_profiles.profiles]]
id = "one"
name = "One"
future_profile_key = true
"#,
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load unknown paths");
    let paths = diagnostic_paths(&document);

    assert!(
        paths.iter().any(|path| path.contains("future_action")),
        "flattened unknown keybinding should be diagnosed: {paths:?}"
    );
    assert!(
        paths.iter().any(|path| path.contains("future_profile_key")),
        "array entry unknown should be diagnosed: {paths:?}"
    );
}

#[test]
fn array_entry_id_edit_preserves_metadata_on_the_same_entry() {
    let temp = TempConfig::new("edited-stable-id");
    temp.write(
        r#"[[render_profiles.profiles]]
id = "old-id"
name = "Profile"
future_profile_key = "keep"
"#,
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load profile");
    let mut updated = document.config().clone();
    updated.render_profiles.profiles[0].id = "new-id".to_string();
    document
        .save_with_backup(updated)
        .expect("save edited profile id");

    let saved = fs::read_to_string(&temp.path).expect("read edited profile");
    assert!(saved.contains("id = \"new-id\""));
    assert!(saved.contains("future_profile_key = \"keep\""));
}

#[test]
fn validated_id_normalization_preserves_entry_metadata() {
    let temp = TempConfig::new("normalized-stable-id");
    temp.write(
        r#"[[render_profiles.profiles]]
id = " Profile One "
name = "Profile One"
future_owner = "keep"
"#,
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load profile");
    assert_eq!(
        document.config().render_profiles.profiles[0].id,
        "profile one"
    );

    document
        .save_with_backup(document.config().clone())
        .expect("save normalized profile id");

    let saved = fs::read_to_string(&temp.path).expect("read normalized profile");
    assert!(saved.contains("id = \"profile one\""));
    assert!(saved.contains("future_owner = \"keep\""));
}

#[test]
fn validation_added_entry_does_not_take_metadata_from_a_normalized_id() {
    let temp = TempConfig::new("normalized-board-with-added-default");
    temp.write(
        r#"[boards]

[[boards.items]]
id = " WhiteBoard "
name = "White board"
background = { rgb = [1.0, 1.0, 1.0] }
future_owner = "keep-with-whiteboard"
"#,
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load boards");
    assert_eq!(document.config().boards.as_ref().unwrap().items.len(), 2);

    document
        .save_with_backup(document.config().clone())
        .expect("save validated boards");

    let saved = fs::read_to_string(&temp.path).expect("read validated boards");
    let value: toml::Value = toml::from_str(&saved).expect("parse validated boards");
    let boards = value["boards"]["items"].as_array().expect("boards array");
    assert_eq!(boards[0]["id"].as_str(), Some("transparent"));
    assert!(boards[0].get("future_owner").is_none());
    assert_eq!(boards[1]["id"].as_str(), Some("whiteboard"));
    assert_eq!(
        boards[1]["future_owner"].as_str(),
        Some("keep-with-whiteboard")
    );
}

#[test]
fn deduplicated_board_ids_keep_metadata_through_validation_reorder() {
    let temp = TempConfig::new("deduplicated-board-reorder");
    temp.write(
        r#"[boards]
max_count = 2
default_board = "DUPLICATE"

[[boards.items]]
id = " Duplicate "
name = "Color board"
background = { rgb = [1.0, 1.0, 1.0] }
future_owner = "owner-color"

[[boards.items]]
id = "other"
name = "Other color board"
background = { rgb = [0.5, 0.5, 0.5] }
future_owner = "owner-other"

[[boards.items]]
id = "duplicate"
name = "Overlay"
background = "transparent"
future_owner = "owner-overlay"
"#,
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load boards");
    let boards = &document.config().boards.as_ref().unwrap().items;
    assert_eq!(boards[0].id, "duplicate-2");
    assert_eq!(boards[1].id, "duplicate");

    document
        .save_with_backup(document.config().clone())
        .expect("save validated boards");

    let saved = fs::read_to_string(&temp.path).expect("read validated boards");
    let value: toml::Value = toml::from_str(&saved).expect("parse validated boards");
    let boards = value["boards"]["items"].as_array().expect("boards array");
    assert_eq!(boards[0]["id"].as_str(), Some("duplicate-2"));
    assert_eq!(boards[0]["future_owner"].as_str(), Some("owner-overlay"));
    assert_eq!(boards[1]["id"].as_str(), Some("duplicate"));
    assert_eq!(boards[1]["future_owner"].as_str(), Some("owner-color"));
}

#[test]
fn validation_truncated_array_entries_survive_unrelated_save() {
    let temp = TempConfig::new("validation-truncated-entry");
    temp.write(
        r#"config_revision = 1
[boards]
max_count = 1
default_board = "transparent"

[[boards.items]]
id = "transparent"
name = "Overlay"
background = "transparent"

[[boards.items]]
id = "future-board"
name = "Future board"
background = { rgb = [0.2, 0.3, 0.4] }
future_owner = "keep"
"#,
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load truncated boards");
    assert_eq!(document.config().boards.as_ref().unwrap().items.len(), 1);
    let mut updated = document.config().clone();
    updated.performance.max_fps_no_vsync = 144;
    document
        .save_with_backup(updated)
        .expect("save unrelated performance edit");

    let saved = fs::read_to_string(&temp.path).expect("read truncated boards");
    assert!(saved.contains("id = \"future-board\""));
    assert!(saved.contains("future_owner = \"keep\""));
}

#[test]
fn idless_array_insertion_keeps_metadata_with_unchanged_entries() {
    let temp = TempConfig::new("idless-array-insertion");
    temp.write(
        r#"config_revision = 1
[[drawing.quick_colors]]
label = "First"
color = "red"
future_owner = "first"

[[drawing.quick_colors]]
label = "Second"
color = "blue"
future_owner = "second"
"#,
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load quick colors");
    let mut updated = document.config().clone();
    updated.drawing.quick_colors.entries.insert(
        0,
        QuickColorConfig {
            label: "New".to_string(),
            color: ColorSpec::Name("green".to_string()),
        },
    );
    document
        .save_with_backup(updated)
        .expect("save inserted quick color");

    let value: toml::Value = toml::from_str(&fs::read_to_string(&temp.path).unwrap()).unwrap();
    let entries = value["drawing"]["quick_colors"].as_array().unwrap();
    assert_eq!(entries[0]["label"].as_str(), Some("New"));
    assert!(entries[0].get("future_owner").is_none());
    assert_eq!(entries[1]["future_owner"].as_str(), Some("first"));
    assert_eq!(entries[2]["future_owner"].as_str(), Some("second"));
}

#[test]
fn positional_array_entries_keep_unknown_fields_when_known_values_change() {
    let temp = TempConfig::new("positional-array");
    temp.write(
        r#"[[drawing.quick_colors]]
label = "First"
color = "red"
future_palette_key = "first-owner"

[[drawing.quick_colors]]
label = "Second"
color = "blue"
future_palette_key = "second-owner"
"#,
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load quick colors");
    let mut updated = document.config().clone();
    updated.drawing.quick_colors.entries[0].label = "Renamed".to_string();
    document
        .save_with_backup(updated)
        .expect("save positional entries");

    let saved = fs::read_to_string(&temp.path).expect("read positional entries");
    let value: toml::Value = toml::from_str(&saved).expect("parse positional entries");
    let entries = value["drawing"]["quick_colors"]
        .as_array()
        .expect("quick color array");
    assert_eq!(entries[0]["label"].as_str(), Some("Renamed"));
    assert_eq!(
        entries[0]["future_palette_key"].as_str(),
        Some("first-owner")
    );
    assert_eq!(
        entries[1]["future_palette_key"].as_str(),
        Some("second-owner")
    );
}

#[test]
fn scalar_array_edits_preserve_element_comments() {
    let temp = TempConfig::new("scalar-array-comments");
    temp.write(
        r#"[keybindings]
undo = [
    "Ctrl+Z", # primary shortcut
    "Alt+Backspace", # secondary shortcut
]
"#,
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load keybinding array");
    let mut updated = document.config().clone();
    updated.keybindings.core.undo[1] = "Ctrl+U".to_string();
    document
        .save_with_backup(updated)
        .expect("save keybinding array");

    let saved = fs::read_to_string(&temp.path).expect("read keybinding array");
    assert!(saved.contains("\"Ctrl+Z\", # primary shortcut"));
    assert!(saved.contains("\"Ctrl+U\", # secondary shortcut"));
}

#[test]
fn exact_revision_detects_same_timestamp_content_replacement() {
    let temp = TempConfig::new("same-time");
    temp.write("[performance]\nmax_fps_no_vsync = 120\n");
    let original_modified = fs::metadata(&temp.path)
        .and_then(|metadata| metadata.modified())
        .expect("read original timestamp");
    let document = ConfigDocument::load_from_path(&temp.path).expect("load document");
    temp.write("[performance]\nmax_fps_no_vsync = 144\n");
    fs::File::open(&temp.path)
        .and_then(|file| file.set_times(fs::FileTimes::new().set_modified(original_modified)))
        .expect("restore original timestamp");

    let error = document
        .save_with_backup(document.config().clone())
        .expect_err("same-time content replacement must conflict");
    assert!(error.to_string().contains("changed on disk"));
    assert!(fs::read_to_string(&temp.path).unwrap().contains("144"));
}

#[test]
fn exact_revision_detects_content_replacement_with_rolled_back_timestamp() {
    let temp = TempConfig::new("rolled-back-time");
    temp.write("[performance]\nmax_fps_no_vsync = 120\n");
    let document = ConfigDocument::load_from_path(&temp.path).expect("load document");
    temp.write("[performance]\nmax_fps_no_vsync = 165\n");
    fs::File::open(&temp.path)
        .and_then(|file| {
            file.set_times(fs::FileTimes::new().set_modified(std::time::SystemTime::UNIX_EPOCH))
        })
        .expect("roll back replacement timestamp");

    assert!(
        document
            .save_with_backup(document.config().clone())
            .expect_err("older timestamp must not hide changed content")
            .to_string()
            .contains("changed on disk")
    );
}

#[test]
fn exact_revision_allows_timestamp_only_change() {
    let temp = TempConfig::new("timestamp-only");
    temp.write("[performance]\nmax_fps_no_vsync = 120\n");
    let document = ConfigDocument::load_from_path(&temp.path).expect("load document");
    fs::File::open(&temp.path)
        .and_then(|file| {
            file.set_times(fs::FileTimes::new().set_modified(std::time::SystemTime::UNIX_EPOCH))
        })
        .expect("roll back timestamp without changing content");

    document
        .save_with_backup(document.config().clone())
        .expect("timestamp-only change is safe");
}

#[test]
fn exact_revision_detects_deletion_creation_and_unsupported_replacement() {
    let deleted = TempConfig::new("deleted");
    deleted.write("[performance]\nmax_fps_no_vsync = 120\n");
    let deleted_document =
        ConfigDocument::load_from_path(&deleted.path).expect("load deleted source");
    fs::remove_file(&deleted.path).expect("delete source");
    assert!(
        deleted_document
            .save_with_backup(deleted_document.config().clone())
            .expect_err("deletion must conflict")
            .to_string()
            .contains("changed on disk")
    );

    let created = TempConfig::new("created");
    let created_document =
        ConfigDocument::load_from_path(&created.path).expect("load missing source");
    created.write("external = true\n");
    assert!(
        created_document
            .save_with_backup(created_document.config().clone())
            .expect_err("creation must conflict")
            .to_string()
            .contains("changed on disk")
    );

    let replaced = TempConfig::new("directory-replacement");
    replaced.write("[performance]\nmax_fps_no_vsync = 120\n");
    let replaced_document =
        ConfigDocument::load_from_path(&replaced.path).expect("load replaced source");
    fs::remove_file(&replaced.path).expect("remove source");
    fs::create_dir(&replaced.path).expect("replace source with directory");
    assert!(
        replaced_document
            .save_with_backup(replaced_document.config().clone())
            .expect_err("unsupported replacement must fail")
            .to_string()
            .contains("not a regular file")
    );
}

#[cfg(unix)]
#[test]
fn exact_revision_detects_changed_symlink_target_with_identical_content() {
    use std::os::unix::fs::symlink;

    let temp = TempConfig::new("symlink-target");
    let first = temp.root.join("first.toml");
    let second = temp.root.join("second.toml");
    fs::write(&first, "[performance]\nmax_fps_no_vsync = 120\n").unwrap();
    fs::write(&second, "[performance]\nmax_fps_no_vsync = 120\n").unwrap();
    symlink(&first, &temp.path).expect("create source symlink");
    let document = ConfigDocument::load_from_path(&temp.path).expect("load symlinked source");
    fs::remove_file(&temp.path).expect("remove old symlink");
    symlink(&second, &temp.path).expect("replace symlink target");

    assert!(
        document
            .save_with_backup(document.config().clone())
            .expect_err("symlink target replacement must conflict")
            .to_string()
            .contains("changed on disk")
    );
}

#[cfg(unix)]
#[test]
fn dangling_symlink_loads_defaults_and_first_save_creates_its_target() {
    use std::os::unix::fs::symlink;

    let temp = TempConfig::new("dangling-symlink");
    let target = temp.root.join("managed.toml");
    symlink(&target, &temp.path).expect("create dangling symlink");

    let document = ConfigDocument::load_from_path(&temp.path).expect("load dangling symlink");
    assert!(matches!(document.source(), ConfigSource::Default));
    document
        .save_with_backup(document.config().clone())
        .expect("save through dangling symlink");

    assert!(target.is_file());
    assert!(
        fs::symlink_metadata(&temp.path)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::read_to_string(target).unwrap(),
        format!("config_revision = {CURRENT_CONFIG_REVISION}\n")
    );
}

#[cfg(unix)]
#[test]
fn dangling_symlink_save_creates_missing_target_parent_directories() {
    use std::os::unix::fs::symlink;

    let temp = TempConfig::new("dangling-symlink-missing-target-parent");
    let target = temp.root.join("managed/nested/config.toml");
    symlink(&target, &temp.path).expect("create dangling symlink");

    let document = ConfigDocument::load_from_path(&temp.path).expect("load dangling symlink");
    document
        .save_with_backup(document.config().clone())
        .expect("save through dangling symlink with missing target parents");

    assert_eq!(
        fs::read_to_string(target).unwrap(),
        format!("config_revision = {CURRENT_CONFIG_REVISION}\n")
    );
    assert!(
        fs::symlink_metadata(&temp.path)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[cfg(unix)]
#[test]
fn document_save_follows_multi_level_symlink_chain() {
    use std::os::unix::fs::symlink;

    let temp = TempConfig::new("multi-level-symlink");
    let target = temp.root.join("managed.toml");
    let intermediate = temp.root.join("intermediate.toml");
    fs::write(
        &target,
        "config_revision = 1\n[performance]\nmax_fps_no_vsync = 120\n",
    )
    .unwrap();
    symlink(&target, &intermediate).unwrap();
    symlink(&intermediate, &temp.path).unwrap();

    let document = ConfigDocument::load_from_path(&temp.path).expect("load symlink chain");
    let mut updated = document.config().clone();
    updated.performance.max_fps_no_vsync = 144;
    document
        .save_with_backup(updated)
        .expect("save through symlink chain");

    assert!(fs::read_to_string(target).unwrap().contains("144"));
    assert!(
        fs::symlink_metadata(&temp.path)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(
        fs::symlink_metadata(intermediate)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[cfg(unix)]
#[test]
fn document_save_preserves_symlink_permissions_and_backs_up_source_contents() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let temp = TempConfig::new("symlink-save");
    let target = temp.root.join("managed.toml");
    let original = "# managed config\n[performance]\nmax_fps_no_vsync = 120\n";
    fs::write(&target, original).expect("write managed target");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600))
        .expect("set managed permissions");
    symlink(&target, &temp.path).expect("create config symlink");
    let document = ConfigDocument::load_from_path(&temp.path).expect("load symlinked document");
    let mut updated = document.config().clone();
    updated.performance.max_fps_no_vsync = 144;

    let outcome = document
        .save_with_backup(updated)
        .expect("save symlinked document");
    let backup = outcome.backup_path().expect("existing source gets backup");
    assert_eq!(fs::read_to_string(backup).unwrap(), original);
    assert!(
        fs::symlink_metadata(&temp.path)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(
        fs::read_to_string(&target)
            .unwrap()
            .contains("max_fps_no_vsync = 144")
    );
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[cfg(not(tablet))]
#[test]
fn disabled_tablet_section_round_trips_without_unknown_warning() {
    let temp = TempConfig::new("disabled-tablet");
    temp.write(
        "[tablet]\nenabled = false\nfuture_tablet_setting = 12\n\n[performance]\nmax_fps_no_vsync = 120\n",
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load tablet section");
    assert!(document.diagnostics().is_empty());

    document
        .save_with_backup(document.config().clone())
        .expect("save disabled tablet section");
    let saved = fs::read_to_string(&temp.path).expect("read disabled tablet section");
    assert!(saved.contains("[tablet]"));
    assert!(saved.contains("enabled = false"));
    assert!(saved.contains("future_tablet_setting = 12"));
}

#[cfg(tablet)]
#[test]
fn enabled_tablet_section_reports_and_preserves_nested_unknown_setting() {
    let temp = TempConfig::new("enabled-tablet");
    temp.write("[tablet]\nenabled = false\nfuture_tablet_setting = 12\n");
    let document = ConfigDocument::load_from_path(&temp.path).expect("load tablet section");
    assert!(
        diagnostic_paths(&document)
            .iter()
            .any(|path| path.ends_with("tablet.future_tablet_setting"))
    );

    document
        .save_with_backup(document.config().clone())
        .expect("save enabled tablet section");
    let saved = fs::read_to_string(&temp.path).expect("read enabled tablet section");
    assert!(saved.contains("future_tablet_setting = 12"));
}

#[test]
fn performance_metadata_is_unique_and_matches_example_and_docs() {
    let mut ids = std::collections::HashSet::new();
    let mut paths = std::collections::HashSet::new();
    let example = include_str!("../../../config.example.toml");
    let example_value: toml::Value = toml::from_str(example).expect("parse config example");
    let docs = include_str!("../../../docs/CONFIG.md");

    for metadata in PERFORMANCE_FIELD_METADATA {
        assert!(ids.insert(metadata.id), "duplicate id: {:?}", metadata.id);
        assert!(
            paths.insert(metadata.path),
            "duplicate path: {}",
            metadata.path
        );
        assert!(value_at_path(&example_value, metadata.path).is_some());
        assert!(
            docs.contains(metadata.path.rsplit('.').next().unwrap()),
            "docs missing {}",
            metadata.path
        );
    }
    assert_eq!(ids.len(), PerformanceFieldId::ALL.len());
}

#[test]
fn performance_validation_uses_metadata_constraints() {
    let mut config = Config::default();
    config.performance.buffer_count = u32::MAX;
    config.performance.ui_animation_fps = u32::MAX;
    config.validate_and_clamp();

    assert_eq!(
        config.performance.buffer_count,
        PERFORMANCE_BUFFER_COUNT_MAX
    );
    assert_eq!(
        config.performance.ui_animation_fps,
        PERFORMANCE_UI_ANIMATION_FPS_MAX
    );
    assert!(
        performance_field_metadata(PerformanceFieldId::BufferCount)
            .constraint
            .accepts_u32(config.performance.buffer_count)
    );
    assert!(
        performance_field_metadata(PerformanceFieldId::UiAnimationFps)
            .constraint
            .accepts_u32(config.performance.ui_animation_fps)
    );
}

fn value_at_path<'a>(root: &'a toml::Value, path: &str) -> Option<&'a toml::Value> {
    path.split('.')
        .try_fold(root, |value, segment| value.get(segment))
}
