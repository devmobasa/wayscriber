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

/// A save writes the caller's one change. Values that only differ because
/// loading clamped them keep their authored text: persisting them would let an
/// unrelated save rewrite settings the user never touched (#293).
#[test]
fn document_save_preserves_comments_order_unknowns_and_clamped_source_text() {
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
    assert!(saved.contains("buffer_count = 99 # keep buffer explanation"));
    assert!(saved.contains("max_fps_no_vsync = 144"));
    assert!(saved.contains("ui_animation_fps = 999 # clamp this known value"));
    assert_eq!(outcome.document().config().performance.buffer_count, 4);
    assert_eq!(
        outcome.document().config().performance.ui_animation_fps,
        240
    );
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
fn document_load_and_save_tolerates_future_keys_in_export_tables() {
    let temp = TempConfig::new("future-export-keys");
    let original = format!(
        r#"config_revision = {CURRENT_CONFIG_REVISION}
[export]
future_format = "svg"

[export.pdf]
page_size = "a4"
future_bleed = 12.5

[export.pdf.labels]
enabled = true
future_font_weight = 600
"#
    );
    temp.write(&original);

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

/// Validation drops a known option in memory only. Removing its line here
/// would be a deletion the caller never asked for, so the text stays and the
/// user keeps the chance to fix the value (#293).
#[test]
fn no_op_save_keeps_known_option_discarded_by_validation() {
    let temp = TempConfig::new("validated-away-known-option");
    let original = r#"config_revision = 1
[render_profiles]
active = "missing"
future_profile_policy = "keep"
"#;
    temp.write(original);
    let document = ConfigDocument::load_from_path(&temp.path).expect("load render profiles");
    assert!(document.config().render_profiles.active.is_none());

    document
        .save_with_backup(document.config().clone())
        .expect("save validated render profiles");

    assert_eq!(
        fs::read_to_string(&temp.path).expect("read validated render profiles"),
        original
    );
}

#[test]
fn save_removes_a_known_option_the_caller_cleared() {
    let temp = TempConfig::new("caller-cleared-known-option");
    temp.write(
        r#"config_revision = 1
[render_profiles]
active = "one"
future_profile_policy = "keep"

[[render_profiles.profiles]]
id = "one"
name = "One"
"#,
    );
    let document = ConfigDocument::load_from_path(&temp.path).expect("load render profiles");
    assert_eq!(
        document.config().render_profiles.active.as_deref(),
        Some("one")
    );
    let mut updated = document.config().clone();
    updated.render_profiles.active = None;

    document
        .save_with_backup(updated)
        .expect("save cleared render profile");

    let saved = fs::read_to_string(&temp.path).expect("read cleared render profile");
    assert!(!saved.contains("active ="));
    assert!(saved.contains("future_profile_policy = \"keep\""));
}

#[test]
fn no_op_save_does_not_materialize_omitted_defaults() {
    let temp = TempConfig::new("omitted-defaults");
    let original = format!(
        "config_revision = {CURRENT_CONFIG_REVISION}\n# intentionally sparse\n[performance]\nmax_fps_no_vsync = 120\n"
    );
    temp.write(&original);
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

/// A file that exists but says nothing is still a file the user wrote, and
/// revision 0 is what it says. Only a document written from scratch — a missing
/// config, or a repair draft — carries a revision the user never chose, because
/// there the stamp describes this build rather than a migration nobody ran.
#[test]
fn saving_over_an_empty_existing_config_does_not_stamp_a_revision() {
    let temp = TempConfig::new("empty-existing");
    temp.write("");
    let document = ConfigDocument::load_from_path(&temp.path).expect("load empty document");
    assert_eq!(document.config().config_revision, 0);

    let mut updated = document.config().clone();
    updated.performance.max_fps_no_vsync = 144;
    document
        .save_with_backup(updated)
        .expect("save empty document");

    let saved = fs::read_to_string(&temp.path).expect("read saved document");
    assert!(saved.contains("max_fps_no_vsync = 144"));
    assert!(
        !saved.contains("config_revision"),
        "the revision belongs to an explicit migration, got:\n{saved}"
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
    assert!(saved.contains(&format!("config_revision = {CURRENT_CONFIG_REVISION}")));
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

/// A legacy revision is a fact about the file, not a task for loading. The
/// document keeps both the authored shortcuts and the authored revision, so the
/// configurator has something to propose and the file has nothing done to it.
#[test]
fn loading_a_legacy_revision_keeps_its_shortcuts_revision_and_bytes() {
    let temp = TempConfig::new("legacy-revision");
    let original = r#"[keybindings]
toggle_command_palette = ["Ctrl+K"]
capture_full_screen = ["Ctrl+Shift+P"]
"#;
    temp.write(original);

    let document = ConfigDocument::load_from_path(&temp.path).expect("load legacy shortcuts");

    assert_eq!(document.config().config_revision, 0);
    assert_eq!(
        document.config().keybindings.ui.toggle_command_palette,
        ["Ctrl+K"]
    );
    assert_eq!(
        document.config().keybindings.capture.capture_full_screen,
        ["Ctrl+Shift+P"]
    );
    assert_eq!(document.authored_config().config_revision, 0);
    assert!(
        document
            .keybinding_authorship()
            .is_explicit("capture_full_screen"),
        "the authored keys are what a migration preview diffs against"
    );
    assert_eq!(fs::read_to_string(&temp.path).unwrap(), original);
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
    let original = format!(
        "config_revision = {CURRENT_CONFIG_REVISION}\n[performance]\nmax_fps_no_vsync = 1_200\n"
    );
    temp.write(&original);
    let document = ConfigDocument::load_from_path(&temp.path).expect("load precise scalar");

    document
        .save_with_backup(document.config().clone())
        .expect("save precise scalar");

    assert_eq!(fs::read_to_string(&temp.path).unwrap(), original);
}

#[test]
fn no_op_save_preserves_integer_spelling_for_float_fields() {
    let temp = TempConfig::new("integer-float-spelling");
    let original =
        format!("config_revision = {CURRENT_CONFIG_REVISION}\n[drawing]\ndefault_thickness = 2\n");
    temp.write(&original);
    let document = ConfigDocument::load_from_path(&temp.path).expect("load integer-form float");

    document
        .save_with_backup(document.config().clone())
        .expect("save integer-form float without changes");

    assert_eq!(fs::read_to_string(&temp.path).unwrap(), original);
}

/// A file that still spells three settings the old way.
const ALIASED_SOURCE: &str = r#"[ui]
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
"#;

/// An alias the save writes over has to be renamed first: serde takes either
/// spelling but not both, so the old key left beside the canonical one the
/// merge inserts would make the file unloadable.
#[test]
fn document_save_canonicalizes_the_key_aliases_it_writes_and_preserves_their_comments() {
    let temp = TempConfig::new("aliases");
    temp.write(ALIASED_SOURCE);
    let document = ConfigDocument::load_from_path(&temp.path).expect("load aliases");

    let mut updated = document.config().clone();
    updated.ui.show_floating_badge_always = false;
    updated.ui.toolbar.mode_overrides.regular.show_presets = Some(false);
    updated
        .render_profiles
        .profiles
        .first_mut()
        .expect("the fixture authors one profile")
        .name = "Renamed".to_string();
    document
        .save_with_backup(updated)
        .expect("save canonical aliases");
    let saved = fs::read_to_string(&temp.path).expect("read canonical aliases");

    assert!(!saved.contains("show_page_badge_with_status_bar"));
    assert!(saved.contains("show_floating_badge_always = false"));
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

/// The other half of the same rule: a save that does not write the aliased
/// setting leaves its spelling alone.
///
/// Renaming it anyway would put settings the caller never touched into the
/// diff — a recolored swatch respelling an unrelated `[ui]` key — which is
/// exactly what the narrow editors promise cannot happen. The old spelling
/// still loads; the save that does change the value renames it then.
#[test]
fn document_save_leaves_the_key_aliases_it_does_not_write_alone() {
    let temp = TempConfig::new("aliases-untouched");
    temp.write(ALIASED_SOURCE);
    let document = ConfigDocument::load_from_path(&temp.path).expect("load aliases");

    // A delta somewhere else entirely, the shape every narrow editor produces.
    let mut updated = document.config().clone();
    updated.drawing.default_thickness = 7.0;
    document
        .save_with_backup(updated)
        .expect("save an unrelated key");
    let saved = fs::read_to_string(&temp.path).expect("read the saved config");

    assert!(saved.contains("default_thickness = 7.0"));
    assert!(saved.contains("show_page_badge_with_status_bar = true"));
    assert!(!saved.contains("show_floating_badge_always"));
    assert!(saved.contains("[ui.toolbar.mode_overrides.full]"));
    assert!(saved.contains("[[render_profiles.items]]"));
    toml::from_str::<Config>(&saved).expect("the untouched aliases still parse");
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
    let original = format!(
        r#"config_revision = {CURRENT_CONFIG_REVISION}
[[render_profiles.profiles]]
id = "first"
name = "First"

[performance]
max_fps_no_vsync = 144

[[render_profiles.profiles]]
id = "second"
name = "Second"
"#
    );
    temp.write(&original);
    let document = ConfigDocument::load_from_path(&temp.path).expect("load separated profiles");

    document
        .save_with_backup(document.config().clone())
        .expect("save separated profiles without changes");

    assert_eq!(fs::read_to_string(&temp.path).unwrap(), original);
}

#[test]
fn no_op_save_preserves_separated_nested_array_table_positions() {
    let temp = TempConfig::new("separated-nested-array-table-positions");
    let original = format!(
        r##"config_revision = {CURRENT_CONFIG_REVISION}
[[render_profiles.profiles]]
id = "first"
name = "First"

[performance]
max_fps_no_vsync = 144

[[render_profiles.profiles.mappings]]
from = "#111111"
to = "#AAAAAA"
"##
    );
    temp.write(&original);
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
    // The reorder is the caller's change; the disambiguating `-2` suffix is
    // not: it is derived on load, so both entries keep their authored id.
    assert_eq!(profiles[0]["name"].as_str(), Some("Second"));
    assert_eq!(profiles[0]["id"].as_str(), Some("duplicate"));
    assert_eq!(profiles[0]["future_owner"].as_str(), Some("owner-second"));
    assert_eq!(profiles[1]["name"].as_str(), Some("First"));
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

/// Id normalization happens on load, not on save: an unrelated save keeps the
/// authored spelling and only the entry the caller edited changes (#293).
#[test]
fn validated_id_normalization_keeps_authored_text_until_the_entry_changes() {
    let temp = TempConfig::new("normalized-stable-id");
    let original = r#"[[render_profiles.profiles]]
id = " Profile One "
name = "Profile One"
future_owner = "keep"
"#;
    temp.write(original);
    let document = ConfigDocument::load_from_path(&temp.path).expect("load profile");
    assert_eq!(
        document.config().render_profiles.profiles[0].id,
        "profile one"
    );

    document
        .save_with_backup(document.config().clone())
        .expect("save normalized profile id");

    assert_eq!(
        fs::read_to_string(&temp.path).expect("read normalized profile"),
        original
    );

    let document = ConfigDocument::load_from_path(&temp.path).expect("reload profile");
    let mut updated = document.config().clone();
    updated.render_profiles.profiles[0].name = "Renamed".to_string();
    document
        .save_with_backup(updated)
        .expect("save renamed profile");

    let saved = fs::read_to_string(&temp.path).expect("read renamed profile");
    assert!(saved.contains("id = \" Profile One \""));
    assert!(saved.contains("name = \"Renamed\""));
    assert!(saved.contains("future_owner = \"keep\""));
}

/// Validation adds the transparent board in memory. Writing that entry on an
/// unrelated save would edit a list the caller never touched, so the file keeps
/// its single authored board until an edit actually changes the list (#293).
#[test]
fn validation_added_entry_reaches_the_file_only_with_a_caller_change() {
    let temp = TempConfig::new("normalized-board-with-added-default");
    let original = r#"[boards]

[[boards.items]]
id = " WhiteBoard "
name = "White board"
background = { rgb = [1.0, 1.0, 1.0] }
future_owner = "keep-with-whiteboard"
"#;
    temp.write(original);
    let document = ConfigDocument::load_from_path(&temp.path).expect("load boards");
    assert_eq!(document.config().boards.as_ref().unwrap().items.len(), 2);

    document
        .save_with_backup(document.config().clone())
        .expect("save validated boards");

    assert_eq!(
        fs::read_to_string(&temp.path).expect("read validated boards"),
        original
    );

    let document = ConfigDocument::load_from_path(&temp.path).expect("reload boards");
    let mut updated = document.config().clone();
    updated.boards.as_mut().unwrap().items[1].name = "Renamed board".to_string();
    document
        .save_with_backup(updated)
        .expect("save renamed board");

    let saved = fs::read_to_string(&temp.path).expect("read renamed boards");
    let value: toml::Value = toml::from_str(&saved).expect("parse renamed boards");
    let boards = value["boards"]["items"].as_array().expect("boards array");
    assert_eq!(boards[0]["id"].as_str(), Some("transparent"));
    assert!(boards[0].get("future_owner").is_none());
    assert_eq!(boards[1]["id"].as_str(), Some(" WhiteBoard "));
    assert_eq!(boards[1]["name"].as_str(), Some("Renamed board"));
    assert_eq!(
        boards[1]["future_owner"].as_str(),
        Some("keep-with-whiteboard")
    );
}

#[test]
fn deduplicated_board_ids_keep_metadata_through_validation_reorder() {
    let temp = TempConfig::new("deduplicated-board-reorder");
    let original = r#"[boards]
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
"#;
    temp.write(original);
    let document = ConfigDocument::load_from_path(&temp.path).expect("load boards");
    let boards = &document.config().boards.as_ref().unwrap().items;
    assert_eq!(boards[0].id, "duplicate-2");
    assert_eq!(boards[1].id, "duplicate");

    document
        .save_with_backup(document.config().clone())
        .expect("save validated boards");

    // Deduplication, truncation and reordering are load-time repairs: an
    // unrelated save must not press them onto the user's file (#293).
    assert_eq!(
        fs::read_to_string(&temp.path).expect("read validated boards"),
        original
    );

    let document = ConfigDocument::load_from_path(&temp.path).expect("reload boards");
    let mut updated = document.config().clone();
    updated.boards.as_mut().unwrap().items[0].name = "Renamed overlay".to_string();
    document
        .save_with_backup(updated)
        .expect("save renamed board");

    let saved = fs::read_to_string(&temp.path).expect("read renamed boards");
    let value: toml::Value = toml::from_str(&saved).expect("parse renamed boards");
    let boards = value["boards"]["items"].as_array().expect("boards array");
    let renamed = boards
        .iter()
        .find(|board| board["name"].as_str() == Some("Renamed overlay"))
        .expect("the renamed board is still in the file");
    assert_eq!(renamed["future_owner"].as_str(), Some("owner-overlay"));
    let color = boards
        .iter()
        .find(|board| board["future_owner"].as_str() == Some("owner-color"))
        .expect("the color board keeps its entry");
    assert_eq!(color["name"].as_str(), Some("Color board"));
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

/// The #293 report: one collision makes validation reset the whole section in
/// memory, and every unrelated save used to write that reset over the user's
/// shortcuts — truncating longer arrays and replacing single bindings.
#[test]
fn reset_keybindings_never_reach_the_file_through_an_unrelated_save() {
    let temp = TempConfig::new("reset-keybindings");
    let keybindings = format!(
        r#"[keybindings]
# Keep these authored shortcuts.
exit = ["Escape", "Ctrl+Q", "Q"]
capture_full_screen = ["Ctrl+Shift+P"]
# `F2` still collides with the `cycle_toolbar_display` default at revision
# {CURRENT_CONFIG_REVISION}, which is what the reporter's file carried.
toggle_toolbar = ["F2", "F9"]
"#
    );
    let original = format!(
        "config_revision = {CURRENT_CONFIG_REVISION}\n\n[ui.toolbar]\nuse_icons = true\n\n{keybindings}"
    );
    temp.write(&original);

    let document = ConfigDocument::load_from_path(&temp.path).expect("load colliding shortcuts");
    let loaded = document.config();
    // The collision costs exactly the colliding key on the side that never
    // authored it. Everything else the file assigns survives the load.
    assert_eq!(
        loaded.keybindings.ui.toggle_toolbar,
        ["F2", "F9"],
        "the authored side of the collision keeps every key it listed"
    );
    assert!(
        loaded.keybindings.ui.cycle_toolbar_display.is_empty(),
        "the serde-filled default loses only the contested key"
    );
    assert_eq!(loaded.keybindings.core.exit, ["Escape", "Ctrl+Q", "Q"]);
    assert_eq!(
        loaded.keybindings.capture.capture_full_screen,
        ["Ctrl+Shift+P"]
    );
    assert_eq!(
        loaded.keybindings.ui.toggle_command_palette,
        ["Ctrl+K"],
        "the palette default keeps the key it does not have to give up"
    );
    let map = loaded
        .keybindings
        .build_action_map()
        .expect("the resolved keymap has no duplicates left");
    assert_eq!(
        map.get(&KeyBinding::parse("F2").expect("F2 parses")),
        Some(&Action::ToggleToolbar)
    );
    assert_eq!(
        map.get(&KeyBinding::parse("Ctrl+Shift+P").expect("Ctrl+Shift+P parses")),
        Some(&Action::CaptureFullScreen)
    );

    // Both keys came off actions the file never mentions, so this reaches the
    // configurator as news about the shipped defaults rather than as a conflict
    // the user has to settle.
    let skipped = document
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.kind() == ConfigDiagnosticKind::DefaultShortcutSkipped)
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    assert_eq!(skipped.len(), 2, "unexpected diagnostics: {skipped:?}");
    assert!(
        skipped.iter().any(|entry| entry.contains("`F2`"))
            && skipped.iter().any(|entry| entry.contains("`Ctrl+Shift+P`")),
        "both skipped keys must be named: {skipped:?}"
    );
    assert!(
        document
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.kind() != ConfigDiagnosticKind::KeybindingConflict),
        "no two authored actions contest a key here"
    );
    assert_eq!(
        diagnostic_paths(&document),
        [
            "keybindings.cycle_toolbar_display",
            "keybindings.toggle_command_palette"
        ]
    );

    let mut updated = document.config().clone();
    updated.ui.toolbar.use_icons = false;
    document
        .save_with_backup(updated)
        .expect("save the unrelated toolbar preference");

    let saved = fs::read_to_string(&temp.path).expect("read saved config");
    assert!(saved.contains("use_icons = false"));
    assert!(
        saved.contains(&keybindings),
        "the keybindings section must be byte-identical, got:\n{saved}"
    );
}

/// A mistyped shortcut used to fail the whole keymap, and the runtime then
/// swapped in the complete shipped defaults for the session — #293's symptom
/// from a single character. Loading now drops the one string, keeps the rest,
/// and leaves the typo in the file for the user to fix.
#[test]
fn dropped_invalid_shortcuts_never_reach_the_file_through_an_unrelated_save() {
    let temp = TempConfig::new("invalid-keybindings");
    let keybindings = r#"[keybindings]
# A typo: the modifiers are there but the key itself is missing.
clear_canvas = ["Ctrl+Shift", "Ctrl+L"]
undo = ["Ctrl+Alt+U"]
"#;
    let original = format!(
        "config_revision = {CURRENT_CONFIG_REVISION}\n\n[ui.toolbar]\nuse_icons = true\n\n{keybindings}"
    );
    temp.write(&original);

    let document = ConfigDocument::load_from_path(&temp.path).expect("load a mistyped shortcut");
    let loaded = document.config();
    assert_eq!(
        loaded.keybindings.core.clear_canvas,
        ["Ctrl+L"],
        "only the string that cannot be parsed is dropped"
    );
    assert_eq!(loaded.keybindings.core.undo, ["Ctrl+Alt+U"]);
    loaded
        .keybindings
        .build_action_map()
        .expect("the session keymap builds without the typo");

    let invalid = document
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.kind() == ConfigDiagnosticKind::InvalidKeybinding)
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    assert_eq!(invalid.len(), 1, "unexpected diagnostics: {invalid:?}");
    assert!(
        invalid[0].contains("`Ctrl+Shift`") && invalid[0].contains("Clear Canvas"),
        "the diagnostic must name the string and the action: {invalid:?}"
    );
    assert_eq!(diagnostic_paths(&document), ["keybindings.clear_canvas"]);

    let mut updated = document.config().clone();
    updated.ui.toolbar.use_icons = false;
    document
        .save_with_backup(updated)
        .expect("save the unrelated toolbar preference");

    let saved = fs::read_to_string(&temp.path).expect("read saved config");
    assert!(saved.contains("use_icons = false"));
    assert!(
        saved.contains(keybindings),
        "the keybindings section must be byte-identical, got:\n{saved}"
    );
}

/// The legacy `["F2", "F9"]` pair is authored and `cycle_toolbar_display` is
/// not, so the file keeps `F2` and the newer default stands down — with no
/// migration write, no backup, and not one byte of the file touched.
#[test]
fn a_legacy_toolbar_pair_outranks_the_newer_default_without_a_write() {
    let temp = TempConfig::new("legacy-toolbar-pair");
    let original = r#"# Keep this comment.
[keybindings]
toggle_toolbar = ["F2", "F9"]
undo = ["Ctrl+Alt+U"]

[performance]
buffer_count = 99
future_knob = 7
"#;
    temp.write(original);

    let document = ConfigDocument::load_from_path(&temp.path).expect("load a legacy toolbar pair");

    assert_eq!(
        document.config().keybindings.ui.toggle_toolbar,
        ["F2", "F9"]
    );
    assert!(
        document
            .config()
            .keybindings
            .ui
            .cycle_toolbar_display
            .is_empty()
    );
    assert_eq!(document.config().keybindings.core.undo, ["Ctrl+Alt+U"]);
    assert_eq!(
        document
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.kind() == ConfigDiagnosticKind::DefaultShortcutSkipped)
            .count(),
        1
    );
    assert_eq!(fs::read_to_string(&temp.path).unwrap(), original);
    assert!(
        fs::read_dir(&temp.root)
            .expect("read temp config directory")
            .filter_map(Result::ok)
            .all(|entry| entry.path().extension().is_none_or(|ext| ext != "bak")),
        "loading never backs the file up"
    );
}

/// #315 added `toggle_input_hud = ["Ctrl+Shift+K"]`, and a file written before
/// the action existed cannot have opted out of it. Source presence settles that
/// on every load without the file ever having to record the answer.
#[test]
fn the_input_hud_default_stands_down_for_a_shortcut_the_file_claims() {
    let temp = TempConfig::new("input-hud-skipped");
    let original = r#"config_revision = 2

[keybindings]
# Screenshot to clipboard, rebound long before the input HUD existed.
capture_clipboard_full = ["Ctrl+Shift+K"]
"#;
    temp.write(original);

    let document = ConfigDocument::load_from_path(&temp.path).expect("load a contested default");

    assert!(document.config().keybindings.ui.toggle_input_hud.is_empty());
    assert_eq!(
        document.config().keybindings.capture.capture_clipboard_full,
        ["Ctrl+Shift+K"]
    );
    assert_eq!(
        document.config().config_revision,
        2,
        "loading does not advance the authored revision"
    );
    assert_eq!(
        diagnostic_paths(&document),
        ["keybindings.toggle_input_hud"]
    );
    assert_eq!(
        document.diagnostics()[0].kind(),
        ConfigDiagnosticKind::DefaultShortcutSkipped,
        "an omitted action losing a default is not the user's conflict"
    );
    let map = document
        .config()
        .keybindings
        .build_action_map()
        .expect("the resolved keymap has no duplicates");
    assert_eq!(
        map.get(&KeyBinding::parse("Ctrl+Shift+K").expect("Ctrl+Shift+K parses")),
        Some(&Action::CaptureClipboardFull)
    );
    assert_eq!(fs::read_to_string(&temp.path).unwrap(), original);
}

/// Nothing contests `Ctrl+Shift+K` here, so the omitted action keeps the whole
/// default and there is nothing to report.
#[test]
fn an_uncontested_default_reaches_an_omitted_action_untouched() {
    let temp = TempConfig::new("input-hud-uncontested");
    let original = "config_revision = 2\n\n[keybindings]\nundo = [\"Ctrl+Alt+U\"]\n";
    temp.write(original);

    let document = ConfigDocument::load_from_path(&temp.path).expect("load an uncontested default");

    assert_eq!(
        document.config().keybindings.ui.toggle_input_hud,
        ["Ctrl+Shift+K"]
    );
    assert!(document.diagnostics().is_empty());
    assert_eq!(fs::read_to_string(&temp.path).unwrap(), original);
}

/// An action the file does spell out is authored, whatever it says. The
/// `Ctrl+Shift+K` collision here is between two authored lists, so it is the
/// user's to settle and traversal order picks the session winner.
#[test]
fn a_customized_input_hud_binding_is_arbitrated_as_an_authored_conflict() {
    let temp = TempConfig::new("input-hud-customized");
    let original = "config_revision = 2\n\n[keybindings]\ncapture_clipboard_full = [\"Ctrl+Shift+K\"]\ntoggle_input_hud = [\"Ctrl+Shift+K\"]\n";
    temp.write(original);

    let document = ConfigDocument::load_from_path(&temp.path).expect("load two authored claims");

    // `ui` is traversed before `capture`, so the input HUD keeps the key.
    assert_eq!(
        document.config().keybindings.ui.toggle_input_hud,
        ["Ctrl+Shift+K"]
    );
    assert!(
        document
            .config()
            .keybindings
            .capture
            .capture_clipboard_full
            .is_empty()
    );
    assert_eq!(
        document.diagnostics()[0].kind(),
        ConfigDiagnosticKind::KeybindingConflict
    );
    assert_eq!(fs::read_to_string(&temp.path).unwrap(), original);
}

/// The revision stamp no longer decides anything about shortcuts: the same
/// file at the current revision resolves exactly as it does at revision 2,
/// because presence — not provenance — is what says which side is authored.
#[test]
fn a_current_revision_file_keeps_its_text_when_it_contests_the_input_hud_default() {
    let temp = TempConfig::new("input-hud-current-revision");
    let original = format!(
        "config_revision = {CURRENT_CONFIG_REVISION}\n\n[keybindings]\ncapture_clipboard_full = [\"Ctrl+Shift+K\"]\n"
    );
    temp.write(&original);

    let document =
        ConfigDocument::load_from_path(&temp.path).expect("load current-revision config");
    assert_eq!(
        document.config().keybindings.capture.capture_clipboard_full,
        ["Ctrl+Shift+K"],
        "the authored side wins the session"
    );
    assert!(
        document.config().keybindings.ui.toggle_input_hud.is_empty(),
        "the omitted action's default loses the contested key for the session"
    );
    assert_eq!(
        diagnostic_paths(&document),
        ["keybindings.toggle_input_hud"]
    );
    assert_eq!(fs::read_to_string(&temp.path).unwrap(), original);
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

#[cfg(not(feature = "tablet-input"))]
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

#[cfg(feature = "tablet-input")]
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

/// The export tables were the config tree's only `deny_unknown_fields`
/// holdouts, so one typo there used to fail the plain deserialize the overlay
/// and the migration writer run — the whole file, not just `[export]`.
#[test]
fn plain_deserialize_tolerates_unknown_export_keys() {
    let config = toml::from_str::<Config>(
        r#"
[export]
future_format = "svg"

[export.pdf]
page_size = "a4"
future_bleed = 12.5

[export.pdf.labels]
enabled = true
future_font_weight = 600
"#,
    )
    .expect("an unknown export key must not fail the whole config");

    assert!(matches!(config.export.pdf.page_size, PdfPageSize::A4));
    assert!(config.export.pdf.labels.enabled);
}

/// A save that really changes an export setting still writes only that
/// setting: the merge never enumerates keys the config does not know, so the
/// user's unrecognized neighbors keep their text and their comments.
#[test]
fn export_save_keeps_unknown_neighbors_in_the_file() {
    let temp = TempConfig::new("changed-export-with-unknowns");
    temp.write(
        r#"[export]
future_format = "svg" # keep this note

[export.pdf]
page_size = "a4"
future_bleed = 12.5

[export.pdf.labels]
enabled = true
future_font_weight = 600
"#,
    );

    let document = ConfigDocument::load_from_path(&temp.path).expect("load export document");
    let mut updated = document.config().clone();
    updated.export.pdf.custom_width = 123.0;
    updated.export.pdf.labels.font_size = 42.0;
    document
        .save_with_backup(updated)
        .expect("save changed export settings");

    let saved = fs::read_to_string(&temp.path).expect("read saved export document");
    assert!(saved.contains("future_format = \"svg\" # keep this note"));
    assert!(saved.contains("future_bleed = 12.5"));
    assert!(saved.contains("future_font_weight = 600"));
    assert!(saved.contains("custom_width = 123.0"));
    assert!(saved.contains("font_size = 42.0"));

    let reloaded = ConfigDocument::load_from_path(&temp.path).expect("reload export document");
    assert!(matches!(
        reloaded.config().export.pdf.page_size,
        PdfPageSize::A4
    ));
    assert_eq!(reloaded.config().export.pdf.custom_width, 123.0);
    assert!(reloaded.config().export.pdf.labels.enabled);
    assert_eq!(reloaded.config().export.pdf.labels.font_size, 42.0);
    let paths = diagnostic_paths(&reloaded);
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
}
