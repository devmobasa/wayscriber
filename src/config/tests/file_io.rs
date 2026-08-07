use super::super::*;
use super::{save_through_document, save_through_document_with_backup};
use crate::config::test_helpers::with_temp_config_home;
use std::fs;

#[cfg(unix)]
use std::os::unix::fs::{PermissionsExt, symlink};

#[test]
fn save_with_backup_creates_timestamped_file() {
    with_temp_config_home(|config_root| {
        let config_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&config_dir).unwrap();
        let config_file = config_dir.join("config.toml");
        let original = "# Keep this backup source comment.\n[ui.toolbar]\ntop_pinned = true\n";
        fs::write(&config_file, original).unwrap();

        let mut config = Config::load()
            .expect("load config before backup save")
            .config;
        config.ui.toolbar.top_pinned = false;
        let backup_path =
            save_through_document_with_backup(config).expect("backup should be created");

        assert!(backup_path.exists());
        assert!(
            backup_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .contains("config.toml."),
            "backup file should include timestamp suffix"
        );
        assert_eq!(fs::read_to_string(&backup_path).unwrap(), original);

        let new_contents = fs::read_to_string(&config_file).unwrap();
        assert!(new_contents.contains("# Keep this backup source comment."));
        assert!(new_contents.contains("top_pinned = false"));
        assert!(!new_contents.contains("[drawing]"));
    });
}

#[test]
fn a_toolbar_preference_save_preserves_comments_and_unrelated_toml_formatting() {
    with_temp_config_home(|config_root| {
        let config_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&config_dir).unwrap();
        let config_file = config_dir.join("config.toml");
        fs::write(
            &config_file,
            r#"# Keep this user comment.

[ui.toolbar]
top_pinned = true

[boards]
default_board = "transparent"

[[boards.items]]
id = "transparent"
name = "Overlay"
background = "transparent"

[[boards.items]]
id = "whiteboard"
name = "Whiteboard"
background = { rgb = [0.992, 0.992, 0.992] }
default_pen_color = { rgb = [0.0, 0.0, 0.0] }
"#,
        )
        .unwrap();

        let mut config = Config::load().expect("load sparse config").config;
        config.ui.toolbar.top_pinned = false;
        save_through_document(config);

        let saved = fs::read_to_string(&config_file).unwrap();
        assert!(saved.contains("# Keep this user comment."));
        assert!(saved.contains("top_pinned = false"));
        assert!(saved.contains("background = { rgb = [0.992, 0.992, 0.992] }"));
        assert!(saved.contains("default_pen_color = { rgb = [0.0, 0.0, 0.0] }"));
        assert!(!saved.contains("[drawing]"));
    });
}

/// A value serde cannot map costs its own section for the session — not the
/// whole file. Before this, one typo threw away every customization with only
/// a log line, the total-loss variant of #293.
#[test]
fn a_bad_value_in_one_section_keeps_every_other_section() {
    with_temp_config_home(|config_root| {
        let config_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&config_dir).unwrap();
        let config_file = config_dir.join("config.toml");
        let original = "[ui]\ntheme = \"drak\"\nshow_status_bar = false\n\n\
                        [drawing]\ndefault_thickness = 7.0\n\n\
                        [capture]\nexit_after_capture = true\n";
        fs::write(&config_file, original).unwrap();

        let loaded = Config::load().expect("a value error must not fail the load");

        assert_eq!(loaded.config.drawing.default_thickness, 7.0);
        assert!(loaded.config.capture.exit_after_capture);
        assert!(
            loaded.config.ui.show_status_bar,
            "the section holding the bad value falls back to defaults as a whole"
        );
        assert_eq!(loaded.section_errors.len(), 1);
        assert_eq!(loaded.section_errors[0].section, "ui");
        assert_eq!(
            fs::read_to_string(&config_file).unwrap(),
            original,
            "loading repairs the session, never the file"
        );
    });
}

/// A top-level scalar gets the same treatment as a section.
#[test]
fn a_bad_top_level_value_reports_its_own_key() {
    with_temp_config_home(|config_root| {
        let config_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&config_dir).unwrap();
        let config_file = config_dir.join("config.toml");
        fs::write(
            &config_file,
            "config_revision = \"three\"\n\n[drawing]\ndefault_thickness = 7.0\n",
        )
        .unwrap();

        let loaded = Config::load().expect("a value error must not fail the load");

        assert_eq!(loaded.config.drawing.default_thickness, 7.0);
        assert_eq!(loaded.section_errors.len(), 1);
        assert_eq!(loaded.section_errors[0].section, "config_revision");
        assert_eq!(
            loaded.config.config_revision, 0,
            "an unreadable revision behaves like a legacy file without one"
        );
    });
}

/// There is no parsed document to salvage from, so a syntax error still fails
/// the load (and the caller reports the total fallback).
#[test]
fn a_syntax_error_still_fails_the_load() {
    with_temp_config_home(|config_root| {
        let config_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("config.toml"), "not = [valid").unwrap();

        assert!(Config::load().is_err());
    });
}

/// Clamping is a load-time repair of the running session, not an edit. A save
/// that follows it must leave the authored number alone so the user still sees
/// (and can fix) what they wrote (#293).
#[test]
fn a_save_keeps_an_out_of_range_value_as_authored() {
    with_temp_config_home(|config_root| {
        let config_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&config_dir).unwrap();
        let config_file = config_dir.join("config.toml");
        fs::write(
            &config_file,
            "[performance]\nbuffer_count = 99\n\n[ui.toolbar]\ntop_pinned = true\n",
        )
        .unwrap();

        let mut config = Config::load().expect("load clamped config").config;
        assert_eq!(config.performance.buffer_count, 4);
        config.ui.toolbar.top_pinned = false;
        save_through_document(config);

        let saved = fs::read_to_string(&config_file).unwrap();
        assert!(saved.contains("buffer_count = 99"));
        assert!(saved.contains("top_pinned = false"));
    });
}

#[test]
fn a_targeted_update_preserves_a_newer_sibling_edit() {
    with_temp_config_home(|config_root| {
        let config_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&config_dir).unwrap();
        let config_file = config_dir.join("config.toml");
        fs::write(
            &config_file,
            "[ui]\nshow_floating_badge = true\n\n[performance]\nmax_fps_no_vsync = 120\n",
        )
        .unwrap();

        // Simulate the running overlay's older in-memory snapshot, followed by
        // an edit made in a second editor while the configurator holds a
        // document loaded from the first contents.
        let _stale_overlay_config = Config::load().expect("load startup config").config;
        fs::write(
            &config_file,
            "# Preserve this newer configurator edit.\n[ui]\nshow_floating_badge = true\n\n[performance]\nmax_fps_no_vsync = 60\n",
        )
        .unwrap();

        Config::update_file(|config| config.ui.show_floating_badge = false)
            .expect("save only the edited badge preference");

        let saved = fs::read_to_string(&config_file).unwrap();
        assert!(saved.contains("# Preserve this newer configurator edit."));
        let reloaded = Config::load().expect("reload targeted update").config;
        assert!(!reloaded.ui.show_floating_badge);
        assert_eq!(reloaded.performance.max_fps_no_vsync, 60);
    });
}

/// The write authoring one quick color performs: one slot's color changes and
/// nothing else in the file moves.
#[test]
fn a_quick_color_recolor_save_rewrites_one_slot_and_keeps_the_rest() {
    with_temp_config_home(|config_root| {
        let config_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&config_dir).unwrap();
        let config_file = config_dir.join("config.toml");
        fs::write(
            &config_file,
            r##"# Keep this palette comment.
[[drawing.quick_colors]]
label = "Red"
color = "#FF0000"

[[drawing.quick_colors]]
label = "Green"
color = "#00FF00"
"##,
        )
        .unwrap();

        Config::update_file(|config| {
            let crimson = crate::draw::Color {
                r: 220.0 / 255.0,
                g: 20.0 / 255.0,
                b: 60.0 / 255.0,
                a: 1.0,
            };
            assert_eq!(
                config.drawing.quick_colors.set_color_at(1, crimson),
                crate::config::QuickColorWrite::Written
            );
        })
        .expect("save the recolored quick color");

        let saved = fs::read_to_string(&config_file).unwrap();
        assert!(saved.contains("# Keep this palette comment."));

        let reloaded = Config::load().expect("reload recolored palette").config;
        let palette = QuickColorPalette::from_config(&reloaded.drawing.quick_colors);
        assert_eq!(
            palette.color_for_index(1),
            Some(crate::draw::Color {
                r: 220.0 / 255.0,
                g: 20.0 / 255.0,
                b: 60.0 / 255.0,
                a: 1.0,
            })
        );
        // The recolored slot keeps its label, the untouched slot keeps both,
        // and the backfilled shortcut slots are now explicit.
        assert_eq!(
            palette.entry(1).map(|entry| entry.label.as_str()),
            Some("Green")
        );
        assert_eq!(
            palette.color_for_index(0),
            Some(crate::draw::Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            })
        );
        assert_eq!(palette.len(), 8);
    });
}

#[test]
fn a_board_reorder_save_does_not_materialize_unchanged_item_preferences() {
    with_temp_config_home(|config_root| {
        let config_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&config_dir).unwrap();
        let config_file = config_dir.join("config.toml");
        fs::write(
            &config_file,
            r#"[boards]
default_board = "transparent"

[[boards.items]]
id = "transparent"
name = "Overlay"
background = "transparent"

[[boards.items]]
id = "whiteboard"
name = "Whiteboard"
background = { rgb = [0.992, 0.992, 0.992] }
"#,
        )
        .unwrap();

        let mut config = Config::load().expect("load board config").config;
        config.boards.as_mut().expect("boards").items.swap(0, 1);
        save_through_document(config);

        let saved = fs::read_to_string(&config_file).unwrap();
        let document = saved.parse::<toml_edit::DocumentMut>().unwrap();
        let boards = document["boards"]["items"].as_array_of_tables().unwrap();
        assert_eq!(
            boards.get(0).and_then(|board| board["id"].as_str()),
            Some("whiteboard")
        );
        assert_eq!(
            boards.get(1).and_then(|board| board["id"].as_str()),
            Some("transparent")
        );
        assert!(boards.iter().all(|board| !board.contains_key("pinned")));
        assert!(
            boards
                .iter()
                .all(|board| !board.contains_key("auto_adjust_pen"))
        );
        assert!(boards.iter().all(|board| !board.contains_key("persist")));
    });
}

#[test]
fn a_save_updates_inline_board_background_without_losing_unknown_fields() {
    with_temp_config_home(|config_root| {
        let config_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&config_dir).unwrap();
        let config_file = config_dir.join("config.toml");
        fs::write(
            &config_file,
            r#"# Preserve this comment while changing the color.

[boards]
default_board = "transparent"

[[boards.items]]
id = "transparent"
name = "Overlay"
background = "transparent"

[[boards.items]]
id = "whiteboard"
name = "Whiteboard"
background = { rgb = [0.992, 0.992, 0.992], future_color_space = "display-p3" }
"#,
        )
        .unwrap();

        let mut config = Config::load().expect("load board config").config;
        let whiteboard = config
            .boards
            .as_mut()
            .unwrap()
            .items
            .iter_mut()
            .find(|board| board.id == "whiteboard")
            .unwrap();
        whiteboard.background =
            BoardBackgroundConfig::Color(BoardColorConfig::Rgb([0.2, 0.3, 0.4]));
        save_through_document(config);

        let saved = fs::read_to_string(&config_file).unwrap();
        assert!(saved.contains("# Preserve this comment while changing the color."));
        let saved_document = saved.parse::<toml_edit::DocumentMut>().unwrap();
        let whiteboard = saved_document["boards"]["items"]
            .as_array_of_tables()
            .unwrap()
            .iter()
            .find(|board| board["id"].as_str() == Some("whiteboard"))
            .unwrap();
        let background = whiteboard["background"].as_inline_table().unwrap();
        assert_eq!(
            background
                .get("future_color_space")
                .and_then(toml_edit::Value::as_str),
            Some("display-p3")
        );
        let reloaded = Config::load().expect("reload changed board config").config;
        let whiteboard = reloaded
            .boards
            .as_ref()
            .unwrap()
            .items
            .iter()
            .find(|board| board.id == "whiteboard")
            .unwrap();
        match &whiteboard.background {
            BoardBackgroundConfig::Color(color) => {
                assert_eq!(color.rgb(), [0.2, 0.3, 0.4]);
            }
            BoardBackgroundConfig::Transparent(value) => {
                panic!("expected changed color board, got {value}");
            }
        }
    });
}

#[test]
fn a_save_updates_inline_default_pen_color_without_losing_unknown_fields() {
    with_temp_config_home(|config_root| {
        let config_dir = config_root.join(PRIMARY_CONFIG_DIR);
        fs::create_dir_all(&config_dir).unwrap();
        let config_file = config_dir.join("config.toml");
        fs::write(
            &config_file,
            r#"[boards]
default_board = "whiteboard"

[[boards.items]]
id = "whiteboard"
name = "Whiteboard"
background = { rgb = [0.992, 0.992, 0.992] }
default_pen_color = { rgb = [0.0, 0.0, 0.0], future_color_space = "display-p3" }
"#,
        )
        .unwrap();

        let mut config = Config::load().expect("load board config").config;
        let whiteboard = config
            .boards
            .as_mut()
            .unwrap()
            .items
            .iter_mut()
            .find(|board| board.id == "whiteboard")
            .unwrap();
        whiteboard.default_pen_color = Some(BoardColorConfig::Rgb([0.8, 0.7, 0.6]));
        save_through_document(config);

        let saved = fs::read_to_string(&config_file).unwrap();
        let saved_document = saved.parse::<toml_edit::DocumentMut>().unwrap();
        let whiteboard = saved_document["boards"]["items"]
            .as_array_of_tables()
            .unwrap()
            .iter()
            .find(|board| board["id"].as_str() == Some("whiteboard"))
            .unwrap();
        let default_pen_color = whiteboard["default_pen_color"].as_inline_table().unwrap();
        assert_eq!(
            default_pen_color
                .get("future_color_space")
                .and_then(toml_edit::Value::as_str),
            Some("display-p3")
        );

        let reloaded = Config::load().expect("reload changed board config").config;
        let whiteboard = reloaded
            .boards
            .as_ref()
            .unwrap()
            .items
            .iter()
            .find(|board| board.id == "whiteboard")
            .unwrap();
        assert_eq!(
            whiteboard
                .default_pen_color
                .as_ref()
                .expect("default pen color should remain configured")
                .rgb(),
            [0.8, 0.7, 0.6]
        );
    });
}

#[cfg(unix)]
#[test]
fn save_with_backup_preserves_symlinked_config_target_and_backup_contents() {
    with_temp_config_home(|config_root| {
        let config_dir = config_root.join(PRIMARY_CONFIG_DIR);
        let managed_dir = config_root.join("managed-config");
        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&managed_dir).unwrap();

        let target = managed_dir.join("config.toml");
        let config_link = config_dir.join("config.toml");
        let original = "# Keep this symlinked comment.\n[ui.toolbar]\ntop_pinned = true\n";
        fs::write(&target, original).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        symlink(&target, &config_link).unwrap();

        let mut config = Config::load().expect("load symlinked config").config;
        config.ui.toolbar.top_pinned = false;
        let backup_path = save_through_document_with_backup(config)
            .expect("backup should be created for symlinked config");

        assert!(
            fs::symlink_metadata(&config_link)
                .unwrap()
                .file_type()
                .is_symlink(),
            "config path should remain a symlink"
        );
        assert_eq!(fs::read_link(&config_link).unwrap(), target);
        assert_eq!(
            fs::read_to_string(&backup_path).unwrap(),
            original,
            "backup should capture the pre-save target contents"
        );
        assert!(
            backup_path
                .parent()
                .is_some_and(|parent| parent == config_dir),
            "backup should stay next to the user-facing config path"
        );

        let target_contents = fs::read_to_string(&target).unwrap();
        assert!(target_contents.contains("# Keep this symlinked comment."));
        assert!(target_contents.contains("top_pinned = false"));
        assert!(!target_contents.contains("[drawing]"));
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o600,
            "symlink target permissions should be preserved"
        );
    });
}

/// Every value in `config.example.toml` claims to be a configured default.
/// This pins that claim: the example parsed as a `Config` must serialize to
/// the same tree as `Config::default()`, so a default changed in code without
/// updating the example (or vice versa) fails here instead of drifting.
#[test]
fn config_example_values_equal_the_compiled_defaults() {
    // Values with a documented reason to differ. Page navigation used to be
    // listed here too; the comparison now scrubs the desktop environment
    // instead, so the example's non-GNOME variant is checked everywhere.
    // - boards.items, drawing.quick_colors, presets.slot_1, and the toolbar
    //   order lists spell out effective built-in defaults for fields whose
    //   compiled default is "unset" (or, for boards, author colors in the
    //   friendlier `{ rgb = ... }` syntax that serializes differently).
    //   boards.items also hides a known internal inconsistency: the serde
    //   field default for auto_adjust_pen is true while the hand-built
    //   transparent default is false.
    const ALLOWED_DRIFT: &[&str] = &[
        "boards.items",
        "drawing.quick_colors",
        "presets.slot_1",
        "ui.toolbar.items.order",
    ];

    // Both sides are built under the same scrubbed environment: parsing the
    // example runs serde's default functions for every key it omits, and some
    // of those consult the desktop environment, so computing one side under
    // GNOME and the other without it compares two different machines.
    let (example, defaults) = crate::test_env::with_scrubbed_desktop_env(|| {
        let example = include_str!("../../../config.example.toml");
        let example: Config = toml::from_str(example).expect("config.example.toml should parse");
        (
            toml::Value::try_from(example).expect("serialize example config"),
            toml::Value::try_from(Config::default()).expect("serialize default config"),
        )
    });

    let mut drifts = Vec::new();
    collect_value_drifts("", &example, &defaults, &mut drifts);
    // Match the allowlist against whole path segments: a bare `starts_with`
    // over `presets.slot_1` would also exempt a future `presets.slot_10`.
    drifts.retain(|drift| {
        !ALLOWED_DRIFT
            .iter()
            .any(|allowed| drift.path == *allowed || drift.path.starts_with(&format!("{allowed}.")))
    });
    assert!(
        drifts.is_empty(),
        "config.example.toml drifted from Config::default():\n{}",
        drifts
            .iter()
            .map(|drift| format!("{}: {}", drift.path, drift.detail))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// One disagreement between the example and the compiled defaults. The path
/// is kept separate from the prose so the allowlist can match path segments
/// rather than message prefixes.
struct ValueDrift {
    path: String,
    detail: String,
}

fn collect_value_drifts(
    path: &str,
    example: &toml::Value,
    defaults: &toml::Value,
    drifts: &mut Vec<ValueDrift>,
) {
    match (example, defaults) {
        (toml::Value::Table(example), toml::Value::Table(defaults)) => {
            let missing = defaults.keys().filter(|key| !example.contains_key(*key));
            for key in example.keys().chain(missing) {
                let child = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                match (example.get(key), defaults.get(key)) {
                    (Some(example), Some(defaults)) => {
                        collect_value_drifts(&child, example, defaults, drifts)
                    }
                    (Some(example), None) => drifts.push(ValueDrift {
                        path: child,
                        detail: format!("example has {example}, defaults omit it"),
                    }),
                    (None, Some(defaults)) => drifts.push(ValueDrift {
                        path: child,
                        detail: format!("defaults have {defaults}, example omits it"),
                    }),
                    (None, None) => unreachable!(),
                }
            }
        }
        (example, defaults) if example == defaults => {}
        (example, defaults) => drifts.push(ValueDrift {
            path: path.to_string(),
            detail: format!("example {example} vs default {defaults}"),
        }),
    }
}

#[test]
fn config_example_parses_and_documents_current_user_facing_fields() {
    let example = include_str!("../../../config.example.toml");
    toml::from_str::<Config>(example).expect("config.example.toml should parse");

    assert!(
        example.contains("show_floating_badge_always ="),
        "example should use the current floating badge field name"
    );
    assert!(
        !example.contains("show_page_badge_with_status_bar ="),
        "example should not use the old floating badge alias"
    );

    for field in [
        "undo_all",
        "redo_all",
        "undo_all_delayed",
        "redo_all_delayed",
        "board_1",
        "board_2",
        "board_3",
        "board_4",
        "board_5",
        "board_6",
        "board_7",
        "board_8",
        "board_9",
        "board_prev",
        "board_next",
        "board_new",
        "board_duplicate",
        "board_delete",
        "board_picker",
        "toggle_quick_help",
        "toggle_command_palette",
        "toggle_floating_badge",
        "toggle_zoom_chip",
        "toggle_focus_mode",
        "zoom_chip_display",
        "show_floating_badge",
        "show_zoom_chip",
    ] {
        assert!(
            example.contains(&format!("{field} =")),
            "example should document keybinding field `{field}`"
        );
    }
}
