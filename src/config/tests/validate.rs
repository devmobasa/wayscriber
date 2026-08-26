use super::super::*;
use crate::input::state::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};

/// A configuration parsed the way a file is, presence included.
///
/// A `Config` built in code is `AllExplicit` — everything it holds counts as
/// authored — so a fixture that needs the authored-versus-omitted distinction
/// has to come from source text, exactly like a load does.
fn config_from_toml(input: &str) -> Config {
    let mut config: Config = toml::from_str(input).expect("fixture config should parse");
    config.keybinding_authorship = KeybindingAuthorship::from_toml_source(input);
    config
}

#[test]
fn validate_and_clamp_clamps_out_of_range_values() {
    let mut config = Config::default();
    config.drawing.default_thickness = 80.0;
    config.drawing.default_font_size = 3.0;
    config.drawing.polygon_sides = 2;
    config.drawing.font_weight = "not-a-real-weight".to_string();
    config.drawing.font_style = "diagonal".to_string();
    config.arrow.length = 100.0;
    config.arrow.angle_degrees = 5.0;
    config.performance.buffer_count = 8;
    config.board.default_mode = "magenta-board".to_string();
    config.board.whiteboard_color = [1.5, -0.5, 0.5];
    config.board.blackboard_color = [-0.2, 2.0, 0.5];
    config.board.whiteboard_pen_color = [2.0, 2.0, 2.0];
    config.board.blackboard_pen_color = [-1.0, -1.0, -1.0];

    config.validate_and_clamp();

    assert_eq!(config.drawing.default_thickness, MAX_STROKE_THICKNESS);
    assert_eq!(config.drawing.default_font_size, 8.0);
    assert_eq!(config.drawing.polygon_sides, 3);
    assert_eq!(config.drawing.font_weight, "bold");
    assert_eq!(config.drawing.font_style, "normal");
    assert_eq!(config.arrow.length, 50.0);
    assert_eq!(config.arrow.angle_degrees, 15.0);
    assert_eq!(config.performance.buffer_count, 4);
    assert_eq!(config.board.default_mode, "transparent");
    assert!(
        config
            .board
            .whiteboard_color
            .iter()
            .all(|c| (0.0..=1.0).contains(c))
    );
    assert!(
        config
            .board
            .blackboard_color
            .iter()
            .all(|c| (0.0..=1.0).contains(c))
    );
    assert!(
        config
            .board
            .whiteboard_pen_color
            .iter()
            .all(|c| (0.0..=1.0).contains(c))
    );
    assert!(
        config
            .board
            .blackboard_pen_color
            .iter()
            .all(|c| (0.0..=1.0).contains(c))
    );
}

#[test]
fn drawing_polygon_sides_validation_keeps_supported_bounds() {
    for supported in [3, 12] {
        let mut config = Config::default();
        config.drawing.polygon_sides = supported;
        config.validate_and_clamp();
        assert_eq!(config.drawing.polygon_sides, supported);
    }

    let mut config = Config::default();
    config.drawing.polygon_sides = u8::MAX;
    config.validate_and_clamp();
    assert_eq!(config.drawing.polygon_sides, 12);
}

#[test]
fn default_overlay_item_is_the_transparent_board() {
    let overlay = BoardsConfig::default_overlay_item();
    assert_eq!(overlay.id, "transparent");
    assert_eq!(overlay.name, "Overlay");
    assert!(overlay.background.is_transparent());
    let first = &BoardsConfig::default_items()[0];
    assert_eq!(first.id, overlay.id);
    assert_eq!(first.name, overlay.name);
    assert!(first.background.is_transparent());
}

#[test]
fn validate_boards_uses_boundary_id_normalization() {
    let mut config = Config {
        boards: Some(BoardsConfig {
            max_count: 4,
            auto_create: true,
            show_board_badge: true,
            pan_enabled: true,
            show_pan_badge: true,
            persist_customizations: true,
            default_board: "transparent".to_string(),
            items: vec![
                BoardItemConfig {
                    id: " Transparent ".to_string(),
                    name: "Overlay".to_string(),
                    background: BoardBackgroundConfig::Transparent("transparent".to_string()),
                    default_pen_color: None,
                    auto_adjust_pen: false,
                    persist: true,
                    pinned: false,
                },
                BoardItemConfig {
                    id: "  BOARD-A ".to_string(),
                    name: "A".to_string(),
                    background: BoardBackgroundConfig::Color(BoardColorConfig::Rgb([
                        1.2, 0.5, -0.1,
                    ])),
                    default_pen_color: Some(BoardColorConfig::Rgb([0.2, 1.4, 0.6])),
                    auto_adjust_pen: true,
                    persist: true,
                    pinned: false,
                },
                BoardItemConfig {
                    id: "board-a".to_string(),
                    name: "Duplicate".to_string(),
                    background: BoardBackgroundConfig::Color(BoardColorConfig::Rgb([
                        0.2, 0.3, 0.4,
                    ])),
                    default_pen_color: None,
                    auto_adjust_pen: true,
                    persist: true,
                    pinned: false,
                },
                BoardItemConfig {
                    id: "   ".to_string(),
                    name: "Defaulted".to_string(),
                    background: BoardBackgroundConfig::Color(BoardColorConfig::Rgb([
                        0.2, 0.3, 0.4,
                    ])),
                    default_pen_color: None,
                    auto_adjust_pen: true,
                    persist: true,
                    pinned: false,
                },
            ],
        }),
        ..Config::default()
    };

    config.validate_and_clamp();

    let boards = config.boards.as_ref().expect("boards");
    let ids: Vec<_> = boards.items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, vec!["transparent", "board-a", "board-a-2", "board-4"]);
    match &boards.items[1].background {
        BoardBackgroundConfig::Color(color) => assert_eq!(color.rgb(), [1.0, 0.5, 0.0]),
        BoardBackgroundConfig::Transparent(_) => panic!("expected color background"),
    }
    assert_eq!(
        boards.items[1]
            .default_pen_color
            .as_ref()
            .expect("pen")
            .rgb(),
        [0.2, 1.0, 0.6]
    );
}

#[test]
fn validate_render_profiles_normalizes_ids_and_mappings() {
    let mut config = Config {
        render_profiles: RenderProfilesConfig {
            active: Some(" PRINT ".to_string()),
            apply_to_canvas: true,
            apply_to_ui: true,
            export: RenderProfileExportMode::Profile,
            export_profile: Some(" off ".to_string()),
            profiles: vec![
                RenderProfileConfig {
                    id: " Print ".to_string(),
                    name: "  Print Friendly  ".to_string(),
                    mappings: vec![
                        RenderColorMappingConfig {
                            from: "#000000".to_string(),
                            to: "FFFFFF".to_string(),
                        },
                        RenderColorMappingConfig {
                            from: "#000000".to_string(),
                            to: "#111111".to_string(),
                        },
                        RenderColorMappingConfig {
                            from: "#GGGGGG".to_string(),
                            to: "#222222".to_string(),
                        },
                    ],
                },
                RenderProfileConfig {
                    id: "off".to_string(),
                    name: " ".to_string(),
                    mappings: Vec::new(),
                },
            ],
        },
        ..Config::default()
    };

    config.validate_and_clamp();

    assert_eq!(config.render_profiles.active.as_deref(), Some("print"));
    assert_eq!(
        config.render_profiles.export,
        RenderProfileExportMode::Profile
    );
    assert_eq!(
        config.render_profiles.export_profile.as_deref(),
        Some("off")
    );
    assert_eq!(config.render_profiles.profiles[0].id, "print");
    assert_eq!(config.render_profiles.profiles[0].name, "Print Friendly");
    assert_eq!(config.render_profiles.profiles[1].id, "off");
    assert_eq!(config.render_profiles.profiles[1].name, "Profile 2");
    assert_eq!(
        config.render_profiles.profiles[0].mappings,
        vec![RenderColorMappingConfig {
            from: "#000000".to_string(),
            to: "#111111".to_string(),
        }]
    );
}

#[test]
fn validate_render_profiles_disables_missing_active_profile() {
    let mut config = Config {
        render_profiles: RenderProfilesConfig {
            active: Some("missing".to_string()),
            apply_to_canvas: true,
            apply_to_ui: true,
            export: RenderProfileExportMode::Off,
            export_profile: None,
            profiles: vec![RenderProfileConfig {
                id: "print".to_string(),
                name: "Print".to_string(),
                mappings: Vec::new(),
            }],
        },
        ..Config::default()
    };

    config.validate_and_clamp();

    assert_eq!(config.render_profiles.active, None);
}

#[test]
fn validate_render_profiles_disables_missing_export_profile() {
    let mut config = Config {
        render_profiles: RenderProfilesConfig {
            active: None,
            apply_to_canvas: true,
            apply_to_ui: true,
            export: RenderProfileExportMode::Profile,
            export_profile: Some("missing".to_string()),
            profiles: vec![RenderProfileConfig {
                id: "print".to_string(),
                name: "Print".to_string(),
                mappings: Vec::new(),
            }],
        },
        ..Config::default()
    };

    config.validate_and_clamp();

    assert_eq!(config.render_profiles.export, RenderProfileExportMode::Off);
    assert_eq!(config.render_profiles.export_profile, None);
}

#[test]
fn validate_render_profiles_ignores_stale_export_profile_for_active_export() {
    let mut config = Config {
        render_profiles: RenderProfilesConfig {
            active: Some("print".to_string()),
            apply_to_canvas: true,
            apply_to_ui: true,
            export: RenderProfileExportMode::Active,
            export_profile: Some("missing".to_string()),
            profiles: vec![RenderProfileConfig {
                id: "print".to_string(),
                name: "Print".to_string(),
                mappings: Vec::new(),
            }],
        },
        ..Config::default()
    };

    config.validate_and_clamp();

    assert_eq!(
        config.render_profiles.export,
        RenderProfileExportMode::Active
    );
    assert_eq!(
        config.render_profiles.export_profile.as_deref(),
        Some("missing")
    );
}

#[test]
fn pdf_filename_template_falls_back_to_capture_template() {
    let mut config = Config::default();
    config.capture.filename_template = "capture_%Y".to_string();
    config.export.pdf.filename_template = None;
    config.export.pdf.all_boards_filename_template = None;

    assert_eq!(
        config
            .export
            .pdf
            .resolved_filename_template(&config.capture),
        "capture_%Y"
    );

    config.export.pdf.filename_template = Some(" board_%Y ".to_string());
    assert_eq!(
        config
            .export
            .pdf
            .resolved_filename_template(&config.capture),
        "board_%Y"
    );

    assert_eq!(
        config
            .export
            .pdf
            .resolved_all_boards_filename_template(&config.capture),
        "board_%Y"
    );

    config.export.pdf.all_boards_filename_template = Some(" all_%Y ".to_string());
    assert_eq!(
        config
            .export
            .pdf
            .resolved_all_boards_filename_template(&config.capture),
        "all_%Y"
    );
}

/// `[export]` used to be the config tree's only table that rejected unknown
/// keys, so one typo failed the entire file. It now follows the same contract
/// as every other section: the key is ignored in memory, reported as an
/// unrecognized setting by the document loader, and left in the file.
#[test]
fn export_pdf_unknown_fields_do_not_fail_the_load() {
    let config = toml::from_str::<Config>("[export.pdf]\nunknown = true\ncustom_width = 640.0\n")
        .expect("an unknown export.pdf field must not fail the config");

    assert_eq!(config.export.pdf.custom_width, 640.0);
}

#[test]
fn pdf_label_template_validation_accepts_placeholders_and_literal_braces() {
    validate_pdf_label_template("{{ {board_name} }} {page_name} {document_page}/{document_pages}")
        .expect("template should validate");

    let err = validate_pdf_label_template("{board_name} {missing}")
        .expect_err("unknown placeholder should fail");
    assert!(err.contains("Unknown"));

    let err =
        validate_pdf_label_template("{board_name").expect_err("unclosed placeholder should fail");
    assert!(err.contains("Unclosed"));
}

#[test]
fn validate_export_pdf_sanitizes_numbers_colors_and_bad_templates() {
    let mut config = Config::default();
    config.export.pdf.custom_width = f64::NAN;
    config.export.pdf.custom_height = 50_000.0;
    config.export.pdf.content_source_padding = -1.0;
    config.export.pdf.labels.template = "{missing}".to_string();
    config.export.pdf.labels.font_family = "  ".to_string();
    config.export.pdf.labels.font_size = f64::INFINITY;
    config.export.pdf.labels.margin = -3.0;
    config.export.pdf.labels.padding_x = 500.0;
    config.export.pdf.labels.text_color = [f64::NAN, -1.0, 2.0, 0.5];
    config.export.pdf.labels.background_color = [0.2, f64::INFINITY, -0.4, 1.5];

    config.validate_and_clamp();

    assert_eq!(config.export.pdf.custom_width, 800.0);
    assert_eq!(config.export.pdf.custom_height, 14_400.0);
    assert_eq!(config.export.pdf.content_source_padding, 0.0);
    assert_eq!(
        config.export.pdf.labels.template,
        PDF_LABEL_DEFAULT_TEMPLATE
    );
    assert_eq!(config.export.pdf.labels.font_family, "Sans");
    assert_eq!(config.export.pdf.labels.font_size, 10.0);
    assert_eq!(config.export.pdf.labels.margin, 0.0);
    assert_eq!(config.export.pdf.labels.padding_x, 120.0);
    assert_eq!(config.export.pdf.labels.text_color, [0.1, 0.0, 1.0, 0.5]);
    assert_eq!(
        config.export.pdf.labels.background_color,
        [0.2, 1.0, 0.0, 1.0]
    );
}

#[test]
fn validate_and_clamp_rejects_path_escaping_save_names() {
    let mut config = Config::default();
    config.capture.filename_template = "../evil_%Y".to_string();
    config.capture.format = "png/../../x".to_string();
    config.export.pdf.filename_template = Some("foo/bar".to_string());
    config.export.pdf.all_boards_filename_template = Some("/tmp/x".to_string());

    config.validate_and_clamp();

    assert_eq!(
        config.capture.filename_template,
        Config::default().capture.filename_template
    );
    assert_eq!(config.capture.format, "png");
    assert_eq!(config.export.pdf.filename_template, None);
    assert_eq!(config.export.pdf.all_boards_filename_template, None);
}

#[test]
fn validate_export_pdf_ignores_template_when_label_content_is_not_custom() {
    let mut config = Config::default();
    config.export.pdf.labels.content = PdfLabelContentMode::DocumentPage;
    config.export.pdf.labels.template = "{missing}".to_string();

    config.validate_and_clamp();

    assert_eq!(config.export.pdf.labels.template, "{missing}");
}

#[test]
fn validate_clamps_history_delays() {
    let mut config = Config::default();
    config.history.undo_all_delay_ms = 0;
    config.history.redo_all_delay_ms = 1;
    config.history.custom_undo_delay_ms = 0;
    config.history.custom_redo_delay_ms = 10_000;
    config.history.custom_undo_steps = 0;
    config.history.custom_redo_steps = 1_000;
    config.validate_and_clamp();
    assert_eq!(config.history.undo_all_delay_ms, 50);
    assert_eq!(config.history.redo_all_delay_ms, 50);
    assert_eq!(config.history.custom_undo_delay_ms, 50);
    assert_eq!(config.history.custom_redo_delay_ms, 5_000);
    assert_eq!(config.history.custom_undo_steps, 1);
    assert_eq!(config.history.custom_redo_steps, 500);

    config.history.undo_all_delay_ms = 20_000;
    config.history.redo_all_delay_ms = 10_000;
    config.history.custom_undo_delay_ms = 20_000;
    config.history.custom_redo_delay_ms = 10_000;
    config.history.custom_undo_steps = 9999;
    config.history.custom_redo_steps = 9999;
    config.validate_and_clamp();
    assert_eq!(config.history.undo_all_delay_ms, 5_000);
    assert_eq!(config.history.redo_all_delay_ms, 5_000);
    assert_eq!(config.history.custom_undo_delay_ms, 5_000);
    assert_eq!(config.history.custom_redo_delay_ms, 5_000);
    assert_eq!(config.history.custom_undo_steps, 500);
    assert_eq!(config.history.custom_redo_steps, 500);
}

#[test]
fn validate_clamps_preset_fields() {
    let mut config = Config::default();
    config.presets.slot_count = 1;
    let tool_setting = |size| PresetToolSettingConfig {
        color: ColorSpec::Name("red".to_string()),
        size,
    };
    config.presets.slot_1 = Some(ToolPresetConfig {
        name: None,
        tool: crate::input::Tool::Pen,
        color: ColorSpec::Name("red".to_string()),
        size: 120.0,
        tool_settings: Some(PresetToolStatesConfig {
            pen: tool_setting(-10.0),
            line: tool_setting(120.0),
            rect: tool_setting(-10.0),
            ellipse: tool_setting(120.0),
            arrow: tool_setting(-10.0),
            blur: tool_setting(120.0),
            marker: tool_setting(-10.0),
            step_marker: tool_setting(120.0),
            eraser_size: -10.0,
        }),
        eraser_kind: None,
        eraser_mode: None,
        marker_opacity: Some(1.2),
        fill_enabled: None,
        font_size: Some(2.0),
        text_background_enabled: None,
        arrow_length: Some(100.0),
        arrow_angle: Some(5.0),
        arrow_head_at_end: None,
        polygon_sides: Some(2),
        show_status_bar: None,
        drag_tools: None,
    });

    config.validate_and_clamp();

    assert_eq!(config.presets.slot_count, PRESET_SLOTS_MIN);
    let preset = config.presets.slot_1.as_ref().expect("slot_1 preset");
    assert_eq!(preset.size, MAX_STROKE_THICKNESS);
    let tool_settings = preset.tool_settings.as_ref().expect("tool settings");
    assert_eq!(tool_settings.pen.size, MIN_STROKE_THICKNESS);
    assert_eq!(tool_settings.line.size, MAX_STROKE_THICKNESS);
    assert_eq!(tool_settings.rect.size, MIN_STROKE_THICKNESS);
    assert_eq!(tool_settings.ellipse.size, MAX_STROKE_THICKNESS);
    assert_eq!(tool_settings.arrow.size, MIN_STROKE_THICKNESS);
    assert_eq!(tool_settings.blur.size, MAX_STROKE_THICKNESS);
    assert_eq!(tool_settings.marker.size, MIN_STROKE_THICKNESS);
    assert_eq!(tool_settings.step_marker.size, MAX_STROKE_THICKNESS);
    assert_eq!(tool_settings.eraser_size, MIN_STROKE_THICKNESS);
    assert_eq!(preset.marker_opacity, Some(0.9));
    assert_eq!(preset.font_size, Some(8.0));
    assert_eq!(preset.arrow_length, Some(50.0));
    assert_eq!(preset.arrow_angle, Some(15.0));
    assert_eq!(preset.polygon_sides, Some(3));
}

#[test]
fn validate_and_clamp_clamps_input_hud_fields() {
    let mut config = Config::default();
    config.ui.input_hud.display_ms = 10;
    config.ui.input_hud.fade_ms = 60_000;
    config.ui.input_hud.max_entries = 0;
    config.ui.input_hud.font_size = 500.0;

    config.validate_and_clamp();

    assert_eq!(config.ui.input_hud.display_ms, 200);
    assert_eq!(config.ui.input_hud.fade_ms, 5_000);
    assert_eq!(config.ui.input_hud.max_entries, 1);
    assert_eq!(config.ui.input_hud.font_size, 72.0);

    let mut upper = Config::default();
    upper.ui.input_hud.display_ms = 120_000;
    upper.ui.input_hud.max_entries = 99;
    upper.ui.input_hud.font_size = f64::NAN;
    upper.validate_and_clamp();
    assert_eq!(upper.ui.input_hud.display_ms, 30_000);
    assert_eq!(upper.ui.input_hud.max_entries, 16);
    assert_eq!(upper.ui.input_hud.font_size, 18.0);
}

#[test]
fn validate_and_clamp_clamps_ui_and_session_fields() {
    let mut config = Config::default();
    config.drawing.marker_opacity = 2.0;
    config.drawing.hit_test_tolerance = 0.5;
    config.drawing.hit_test_linear_threshold = 0;
    config.drawing.undo_stack_limit = 5;
    config.ui.click_highlight.radius = 5.0;
    config.ui.click_highlight.outline_thickness = 50.0;
    config.ui.click_highlight.duration_ms = 10;
    config.ui.command_palette_toast_duration_ms = 50;
    config.ui.click_highlight.fill_color = [2.0, -1.0, 0.5, 0.5];
    config.ui.click_highlight.outline_color = [-0.2, 2.0, 0.5, 1.2];
    config.ui.toolbar.scale = 5.0;
    config.session.max_shapes_per_frame = 0;
    config.session.max_file_size_mb = 2048;
    config.session.auto_compress_threshold_kb = 0;
    config.session.autosave_idle_ms = 0;
    config.session.autosave_interval_ms = 0;
    config.session.autosave_failure_backoff_ms = 0;
    config.session.storage = SessionStorageMode::Custom;
    config.session.custom_directory = Some("  ".to_string());
    config.keybindings.core.exit = vec!["Ctrl+Shift".to_string()];

    config.validate_and_clamp();

    assert_eq!(config.drawing.marker_opacity, 0.9);
    assert_eq!(config.drawing.hit_test_tolerance, 1.0);
    assert_eq!(config.drawing.hit_test_linear_threshold, 400);
    assert_eq!(config.drawing.undo_stack_limit, 10);
    assert_eq!(config.ui.click_highlight.radius, 16.0);
    assert_eq!(config.ui.click_highlight.outline_thickness, 12.0);
    assert_eq!(config.ui.click_highlight.duration_ms, 150);
    assert_eq!(config.ui.command_palette_toast_duration_ms, 300);
    assert!(
        config
            .ui
            .click_highlight
            .fill_color
            .iter()
            .all(|c| (0.0..=1.0).contains(c))
    );
    assert!(
        config
            .ui
            .click_highlight
            .outline_color
            .iter()
            .all(|c| (0.0..=1.0).contains(c))
    );
    assert_eq!(config.ui.toolbar.scale, 3.0);
    assert_eq!(config.session.max_shapes_per_frame, 1);
    assert_eq!(config.session.max_file_size_mb, 1024);
    assert_eq!(config.session.auto_compress_threshold_kb, 1);
    assert_eq!(config.session.autosave_idle_ms, 1000);
    assert_eq!(config.session.autosave_interval_ms, 1000);
    assert_eq!(config.session.autosave_failure_backoff_ms, 1000);
    assert!(matches!(config.session.storage, SessionStorageMode::Auto));
    assert!(config.session.custom_directory.is_none());
    // A binding string we cannot parse is a typo for the user to fix. It
    // binds nothing at runtime, so the session drops it and keeps every other
    // shortcut; leaving it in used to fail the whole keymap and get the
    // shipped defaults installed over the user's section (#293). The file
    // still holds the typo, and validation reports it.
    assert!(config.keybindings.core.exit.is_empty());
}

#[test]
fn validate_and_clamp_resets_non_finite_toolbar_scale() {
    let mut config = Config::default();
    config.ui.toolbar.scale = f64::NAN;
    config.validate_and_clamp();
    assert_eq!(config.ui.toolbar.scale, 1.0);

    let mut config = Config::default();
    config.ui.toolbar.scale = f64::INFINITY;
    config.validate_and_clamp();
    assert_eq!(config.ui.toolbar.scale, 1.0);

    let mut config = Config::default();
    config.ui.toolbar.scale = f64::NEG_INFINITY;
    config.validate_and_clamp();
    assert_eq!(config.ui.toolbar.scale, 1.0);
}

#[test]
fn validate_and_clamp_resets_non_finite_hit_test_tolerance() {
    let default = Config::default().drawing.hit_test_tolerance;

    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut config = Config::default();
        config.drawing.hit_test_tolerance = invalid;

        config.validate_and_clamp();

        assert_eq!(config.drawing.hit_test_tolerance, default);
    }
}

#[test]
fn validate_and_clamp_resets_non_finite_spotlight_settings() {
    let defaults = Config::default().spotlight;

    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut config = Config::default();
        config.spotlight.dim_opacity = invalid;
        config.spotlight.feather = invalid;
        config.spotlight.magnification = invalid;

        config.validate_and_clamp();

        assert_eq!(config.spotlight.dim_opacity, defaults.dim_opacity);
        assert_eq!(config.spotlight.feather, defaults.feather);
        assert_eq!(config.spotlight.magnification, defaults.magnification);
    }
}

#[test]
fn validate_and_clamp_limits_spotlight_magnification_to_one_through_four_x() {
    let mut config = Config::default();
    assert_eq!(config.spotlight.magnification, 1.0);

    config.spotlight.magnification = 0.25;
    config.validate_and_clamp();
    assert_eq!(config.spotlight.magnification, 1.0);

    config.spotlight.magnification = 9.0;
    config.validate_and_clamp();
    assert_eq!(config.spotlight.magnification, 4.0);
}

/// The migration recipes are no longer part of loading — they are the material
/// an explicit configurator review proposes — so they are exercised directly
/// from here on.
#[test]
fn legacy_command_palette_and_capture_defaults_migrate_as_a_pair() {
    let mut config = Config {
        config_revision: 0,
        ..Config::default()
    };
    config.keybindings.ui.toggle_command_palette = vec!["Ctrl+K".to_string()];
    config.keybindings.capture.capture_full_screen = vec!["Ctrl+Shift+P".to_string()];

    config.apply_keybinding_migrations();

    assert_eq!(
        config.keybindings.ui.toggle_command_palette,
        ["Ctrl+K", "Ctrl+Shift+P"]
    );
    assert_eq!(
        config.keybindings.capture.capture_full_screen,
        ["Ctrl+Alt+F"]
    );
    assert_eq!(config.config_revision, CURRENT_CONFIG_REVISION);
}

#[test]
fn legacy_shortcut_migration_handles_one_serde_filled_current_default() {
    let mut legacy_command = Config {
        config_revision: 0,
        ..Config::default()
    };
    legacy_command.keybindings.ui.toggle_command_palette = vec!["Ctrl+K".to_string()];
    legacy_command.apply_keybinding_migrations();
    assert_eq!(
        legacy_command.keybindings.ui.toggle_command_palette,
        ["Ctrl+K", "Ctrl+Shift+P"]
    );
    assert_eq!(
        legacy_command.keybindings.capture.capture_full_screen,
        ["Ctrl+Alt+F"]
    );

    let mut legacy_capture = Config {
        config_revision: 0,
        ..Config::default()
    };
    legacy_capture.keybindings.capture.capture_full_screen = vec!["Ctrl+Shift+P".to_string()];
    legacy_capture.apply_keybinding_migrations();
    assert_eq!(
        legacy_capture.keybindings.ui.toggle_command_palette,
        ["Ctrl+K", "Ctrl+Shift+P"]
    );
    assert_eq!(
        legacy_capture.keybindings.capture.capture_full_screen,
        ["Ctrl+Alt+F"]
    );
}

#[test]
fn legacy_shortcut_migration_preserves_customized_pairs() {
    let mut custom_command = Config {
        config_revision: 0,
        ..Config::default()
    };
    custom_command.keybindings.ui.toggle_command_palette = vec!["Ctrl+Space".to_string()];
    custom_command.keybindings.capture.capture_full_screen = vec!["Ctrl+Shift+P".to_string()];
    custom_command.apply_keybinding_migrations();
    assert_eq!(
        custom_command.keybindings.ui.toggle_command_palette,
        ["Ctrl+Space"]
    );
    assert_eq!(
        custom_command.keybindings.capture.capture_full_screen,
        ["Ctrl+Shift+P"]
    );
    assert_eq!(custom_command.config_revision, CURRENT_CONFIG_REVISION);

    let mut custom_capture = Config {
        config_revision: 0,
        ..Config::default()
    };
    custom_capture.keybindings.ui.toggle_command_palette = vec!["Ctrl+K".to_string()];
    custom_capture.keybindings.capture.capture_full_screen = vec!["Ctrl+Alt+G".to_string()];
    custom_capture.apply_keybinding_migrations();
    assert_eq!(
        custom_capture.keybindings.ui.toggle_command_palette,
        ["Ctrl+K"]
    );
    assert_eq!(
        custom_capture.keybindings.capture.capture_full_screen,
        ["Ctrl+Alt+G"]
    );
    assert_eq!(custom_capture.config_revision, CURRENT_CONFIG_REVISION);
}

/// #315 shipped `toggle_input_hud = ["Ctrl+Shift+K"]` without a revision bump,
/// so serde hands that shortcut to the input HUD in every file written before
/// the action existed — including files that bound it to something else.
/// Revision 3 leaves the authored side alone and unbinds the newcomer.
#[test]
fn input_hud_default_yields_to_a_shortcut_the_file_already_claims() {
    let mut config = Config {
        config_revision: 2,
        ..Config::default()
    };
    // Written the way the file has it: the comparison parses both sides, so
    // modifier order and case cannot hide the collision.
    config.keybindings.capture.capture_clipboard_full = vec!["shift+ctrl+k".to_string()];

    config.apply_keybinding_migrations();

    assert!(config.keybindings.ui.toggle_input_hud.is_empty());
    assert_eq!(
        config.keybindings.capture.capture_clipboard_full,
        ["shift+ctrl+k"],
        "the authored binding keeps both the key and its authored spelling"
    );
    assert_eq!(config.config_revision, CURRENT_CONFIG_REVISION);
}

#[test]
fn input_hud_migration_preserves_a_customized_binding() {
    let mut config = Config {
        config_revision: 2,
        ..Config::default()
    };
    config.keybindings.capture.capture_clipboard_full = vec!["Ctrl+Shift+K".to_string()];
    config.keybindings.ui.toggle_input_hud = vec!["Ctrl+Alt+K".to_string()];

    config.apply_keybinding_migrations();

    assert_eq!(config.keybindings.ui.toggle_input_hud, ["Ctrl+Alt+K"]);
    assert_eq!(
        config.keybindings.capture.capture_clipboard_full,
        ["Ctrl+Shift+K"]
    );
    assert_eq!(config.config_revision, CURRENT_CONFIG_REVISION);
}

#[test]
fn input_hud_migration_keeps_the_default_when_no_one_contests_it() {
    let mut config = Config {
        config_revision: 2,
        ..Config::default()
    };

    config.apply_keybinding_migrations();

    assert_eq!(config.keybindings.ui.toggle_input_hud, ["Ctrl+Shift+K"]);
    assert_eq!(config.config_revision, CURRENT_CONFIG_REVISION);
}

/// Validation is not a migration: an old revision comes out of it exactly as
/// old as it went in, so nothing but an explicit configurator review can claim
/// a file has been upgraded.
#[test]
fn validation_never_advances_the_config_revision() {
    for revision in [0, 1, 2] {
        let mut config = Config {
            config_revision: revision,
            ..Config::default()
        };
        config.keybindings.ui.toggle_command_palette = vec!["Ctrl+K".to_string()];

        config.validate_and_clamp();

        assert_eq!(config.config_revision, revision);
        assert_eq!(
            config.keybindings.ui.toggle_command_palette,
            ["Ctrl+K"],
            "the legacy value stays until the user accepts a migration"
        );
    }
}

/// A file already stamped with revision 3 settled the input HUD question once;
/// a shortcut it later points at `Ctrl+Shift+K` is the user's business, not the
/// migration's. Session-level conflict resolution still applies on load.
#[test]
fn recorded_revision_three_stops_the_input_hud_migration_from_rerunning() {
    let mut config = Config::default();
    config.keybindings.capture.capture_clipboard_full = vec!["Ctrl+Shift+K".to_string()];

    config.apply_keybinding_migrations();

    assert_eq!(config.keybindings.ui.toggle_input_hud, ["Ctrl+Shift+K"]);
}

#[test]
fn intentional_legacy_shortcut_pair_is_preserved_after_current_default_edit() {
    let mut config = Config::default();
    config.keybindings.ui.toggle_command_palette = vec!["Ctrl+K".to_string()];
    config.keybindings.capture.capture_full_screen = vec!["Ctrl+Shift+P".to_string()];

    config.validate_and_clamp();

    assert_eq!(config.keybindings.ui.toggle_command_palette, ["Ctrl+K"]);
    assert_eq!(
        config.keybindings.capture.capture_full_screen,
        ["Ctrl+Shift+P"]
    );
}

#[cfg(feature = "tablet-input")]
#[test]
fn validate_clamps_pressure_thickness_scale_step() {
    let mut config = Config::default();
    config.tablet.pressure_thickness_scale_step = 0.0;
    config.validate_and_clamp();
    assert_eq!(config.tablet.pressure_thickness_scale_step, 0.0);

    config.tablet.pressure_thickness_scale_step = 1.5;
    config.validate_and_clamp();
    assert_eq!(config.tablet.pressure_thickness_scale_step, 1.0);
}

#[test]
fn validate_does_not_clamp_autosave_interval_to_idle() {
    let mut config = Config::default();
    config.session.autosave_idle_ms = 60_000;
    config.session.autosave_interval_ms = 5_000;

    config.validate_and_clamp();

    assert_eq!(config.session.autosave_idle_ms, 60_000);
    assert_eq!(config.session.autosave_interval_ms, 5_000);
}

#[test]
fn drawing_drag_tool_defaults_match_legacy_mapping() {
    let config = Config::default();

    assert_eq!(
        config.drawing.drag_tool,
        crate::input::DragBindableTool::Pen
    );
    assert_eq!(
        config.drawing.shift_drag_tool,
        crate::input::DragBindableTool::Line
    );
    assert_eq!(
        config.drawing.ctrl_drag_tool,
        crate::input::DragBindableTool::Rect
    );
    assert_eq!(
        config.drawing.ctrl_shift_drag_tool,
        crate::input::DragBindableTool::Arrow
    );
    assert_eq!(
        config.drawing.tab_drag_tool,
        crate::input::DragBindableTool::Ellipse
    );
}

/// Adding alpha must not rewrite anyone's config: an opaque color has to keep
/// serializing in exactly the form it did before `Rgba` existed.
#[test]
fn opaque_colors_still_serialize_without_an_alpha_component() {
    use crate::config::ColorSpec;
    use crate::draw::Color;

    let opaque = Color {
        r: 1.0,
        g: 0.5,
        b: 0.0,
        a: 1.0,
    };
    assert_eq!(ColorSpec::from(opaque), ColorSpec::Rgb([255, 128, 0]));

    let translucent = Color { a: 0.5, ..opaque };
    assert_eq!(
        ColorSpec::from(translucent),
        ColorSpec::Rgba([255, 128, 0, 128])
    );
}

#[test]
fn color_spec_round_trips_alpha_through_hex_and_arrays() {
    use crate::config::ColorSpec;

    let from_hex = ColorSpec::Name("#FF800080".to_string()).to_color();
    assert!((from_hex.a - 128.0 / 255.0).abs() < 1e-9);
    assert!((from_hex.r - 1.0).abs() < 1e-9);

    // Six-digit hex and three-component arrays stay fully opaque.
    assert!((ColorSpec::Name("#FF8000".to_string()).to_color().a - 1.0).abs() < 1e-9);
    assert!((ColorSpec::Rgb([255, 128, 0]).to_color().a - 1.0).abs() < 1e-9);
    assert!((ColorSpec::Rgba([255, 128, 0, 64]).to_color().a - 64.0 / 255.0).abs() < 1e-9);
}

/// The reporter's case (#293): an authored `toggle_toolbar` collides with the
/// `cycle_toolbar_display` default the file never mentions. The authored side
/// must come through untouched, and the loss belongs to the default — so it is
/// reported as a skipped default, not as a conflict the user has to settle.
#[test]
fn an_omitted_default_loses_only_the_key_an_authored_list_claims() {
    let mut config = config_from_toml(
        "[keybindings]\ntoggle_toolbar = [\"F2\", \"F9\"]\nexit = [\"Escape\", \"Q\"]\n",
    );

    let report = config.validate_and_clamp();

    assert_eq!(config.keybindings.ui.toggle_toolbar, ["F2", "F9"]);
    assert!(config.keybindings.ui.cycle_toolbar_display.is_empty());
    assert_eq!(config.keybindings.core.exit, ["Escape", "Q"]);
    assert!(config.keybindings.build_action_map().is_ok());

    assert!(report.keybinding_conflicts.is_empty());
    assert_eq!(report.skipped_default_shortcuts.len(), 1);
    let skipped = &report.skipped_default_shortcuts[0];
    assert_eq!(skipped.binding(), "F2");
    assert_eq!(skipped.action(), Action::CycleToolbarDisplay);
    assert_eq!(skipped.claimed_by(), Action::ToggleToolbar);
    assert_eq!(skipped.config_key(), Some("cycle_toolbar_display"));
    let rendered = skipped.to_string();
    assert!(
        rendered.contains("`F2`")
            && rendered.contains("Cycle Toolbar Display")
            && rendered.contains("Toggle Toolbar"),
        "the report must name the key and both actions: {rendered}"
    );
    assert!(!report.is_empty(), "the user has to be told");
}

/// The upgrade case for the `Ctrl+Shift+C` handover. A config written before
/// interactive capture took that chord spells out `capture_clipboard_selection`
/// and says nothing about `capture_region_interactive`. The authored one-step
/// copy has to survive untouched: the new default stands down and the user is
/// told, rather than a shortcut they chose being taken away by an upgrade.
#[test]
fn an_authored_region_copy_keeps_the_chord_the_new_interactive_default_wants() {
    let mut config =
        config_from_toml("[keybindings]\ncapture_clipboard_selection = [\"Ctrl+Shift+C\"]\n");
    assert_eq!(
        config.keybindings.capture.capture_region_interactive,
        ["Ctrl+Shift+C"],
        "fixture depends on the interactive default owning the same chord"
    );

    let report = config.validate_and_clamp();

    assert_eq!(
        config.keybindings.capture.capture_clipboard_selection,
        ["Ctrl+Shift+C"],
        "the authored binding is untouched"
    );
    assert!(
        config
            .keybindings
            .capture
            .capture_region_interactive
            .is_empty(),
        "the omitted default stands down"
    );
    assert_eq!(
        config
            .keybindings
            .build_action_map()
            .unwrap()
            .get(&Shortcut::parse("Ctrl+Shift+C").unwrap()),
        Some(&Action::CaptureClipboardSelection)
    );

    assert!(
        report.keybinding_conflicts.is_empty(),
        "nothing for the user to settle: they never wrote the collision"
    );
    assert_eq!(report.skipped_default_shortcuts.len(), 1);
    let skipped = &report.skipped_default_shortcuts[0];
    assert_eq!(skipped.action(), Action::CaptureRegionInteractive);
    assert_eq!(skipped.claimed_by(), Action::CaptureClipboardSelection);
    assert_eq!(skipped.binding(), "Ctrl+Shift+C");
    assert_eq!(skipped.config_key(), Some("capture_region_interactive"));
}

/// A default that only overlaps part of an authored list keeps the rest of its
/// own keys: the offer is made one key at a time, not per action.
#[test]
fn an_omitted_defaults_other_keys_are_installed_normally() {
    let mut config = config_from_toml("[keybindings]\ncapture_full_screen = [\"Ctrl+Shift+P\"]\n");
    assert_eq!(
        config.keybindings.ui.toggle_command_palette,
        ["Ctrl+K", "Ctrl+Shift+P"],
        "fixture depends on the palette default owning both keys"
    );

    let report = config.validate_and_clamp();

    assert_eq!(
        config.keybindings.capture.capture_full_screen,
        ["Ctrl+Shift+P"]
    );
    assert_eq!(config.keybindings.ui.toggle_command_palette, ["Ctrl+K"]);
    assert!(report.keybinding_conflicts.is_empty());
    assert_eq!(report.skipped_default_shortcuts.len(), 1);
    assert_eq!(
        report.skipped_default_shortcuts[0].action(),
        Action::ToggleCommandPalette
    );
}

/// Two authored bindings that truly collide are settled by the keymap
/// traversal order, and everything they do not contest survives on both sides.
/// The returned resolution is what the toast, the desktop notification, the
/// configurator diagnostic, and the `log::warn` all render.
#[test]
fn keybinding_conflict_between_two_authored_actions_keeps_the_earlier_one() {
    let mut config = Config::default();
    config.keybindings.core.undo = vec![
        "Ctrl+Alt+Shift+U".to_string(),
        "Ctrl+Alt+Shift+Z".to_string(),
    ];
    config.keybindings.ui.toggle_help = vec![
        "Ctrl+Alt+Shift+Z".to_string(),
        "Ctrl+Alt+Shift+H".to_string(),
    ];

    let report = config.validate_and_clamp();

    // `core` is traversed before `ui`, so undo keeps the contested key.
    assert_eq!(
        config.keybindings.core.undo,
        ["Ctrl+Alt+Shift+U", "Ctrl+Alt+Shift+Z"]
    );
    assert_eq!(config.keybindings.ui.toggle_help, ["Ctrl+Alt+Shift+H"]);
    assert!(config.keybindings.build_action_map().is_ok());

    assert_eq!(report.keybinding_conflicts.len(), 1);
    let resolution = &report.keybinding_conflicts[0];
    assert_eq!(resolution.kept(), Action::Undo);
    assert_eq!(resolution.dropped(), Action::ToggleHelp);
    assert!(!resolution.is_self_duplicate());
    // The reported key is normalized, so it reads the same however the file
    // ordered the modifiers.
    assert_eq!(resolution.key(), "Ctrl+Shift+Alt+Z");
    let rendered = resolution.to_string();
    assert!(
        rendered.contains("Ctrl+Shift+Alt+Z"),
        "unexpected: {rendered}"
    );
    assert!(rendered.contains("Undo"), "unexpected: {rendered}");
    assert!(rendered.contains("Help"), "unexpected: {rendered}");
}

/// Authorship, not traversal position, decides whether a list is on offer:
/// `core.exit` is traversed first, but the file never mentions it, so the
/// authored capture binding keeps `Escape` even though it is visited later.
#[test]
fn an_authored_list_outranks_an_omitted_one_traversed_before_it() {
    let mut config = config_from_toml("[keybindings]\ncapture_selection = [\"Escape\"]\n");

    let report = config.validate_and_clamp();

    assert_eq!(config.keybindings.capture.capture_selection, ["Escape"]);
    assert_eq!(config.keybindings.core.exit, ["Ctrl+Q"]);
    assert!(report.keybinding_conflicts.is_empty());
    assert_eq!(report.skipped_default_shortcuts.len(), 1);
    assert_eq!(report.skipped_default_shortcuts[0].action(), Action::Exit);
    assert_eq!(
        report.skipped_default_shortcuts[0].claimed_by(),
        Action::CaptureSelection
    );
}

/// The same fixture with both keys spelled out is a different question: two
/// authored lists contest `Escape`, and the traversal order settles it.
#[test]
fn two_authored_lists_contesting_a_key_fall_back_to_traversal_order() {
    let mut config = config_from_toml(
        "[keybindings]\nexit = [\"Escape\", \"Ctrl+Q\"]\ncapture_selection = [\"Escape\"]\n",
    );

    let report = config.validate_and_clamp();

    assert_eq!(config.keybindings.core.exit, ["Escape", "Ctrl+Q"]);
    assert!(config.keybindings.capture.capture_selection.is_empty());
    assert!(report.skipped_default_shortcuts.is_empty());
    assert_eq!(report.keybinding_conflicts.len(), 1);
    assert_eq!(report.keybinding_conflicts[0].kept(), Action::Exit);
    assert_eq!(
        report.keybinding_conflicts[0].dropped(),
        Action::CaptureSelection
    );
}

#[test]
fn keybinding_validation_never_replaces_the_whole_section() {
    let mut config = Config::default();
    config.keybindings.core.exit =
        vec!["Escape".to_string(), "Ctrl+Q".to_string(), "Q".to_string()];
    config.keybindings.core.clear_canvas = vec!["Ctrl+Alt+Shift+C".to_string()];
    config.keybindings.tools.select_pen_tool = vec!["Ctrl+Alt+Shift+C".to_string()];

    config.validate_and_clamp();

    assert_eq!(config.keybindings.core.exit, ["Escape", "Ctrl+Q", "Q"]);
    assert_eq!(config.keybindings.core.clear_canvas, ["Ctrl+Alt+Shift+C"]);
    assert!(config.keybindings.tools.select_pen_tool.is_empty());
}

#[test]
fn keybinding_listed_twice_for_one_action_is_deduplicated() {
    let mut config = Config::default();
    config.keybindings.core.exit = vec!["Ctrl+Alt+Q".to_string(), "Ctrl+Alt+Q".to_string()];

    let report = config.validate_and_clamp();

    assert_eq!(config.keybindings.core.exit, ["Ctrl+Alt+Q"]);
    assert!(config.keybindings.build_action_map().is_ok());
    assert_eq!(report.keybinding_conflicts.len(), 1);
    assert!(report.keybinding_conflicts[0].is_self_duplicate());
}

#[test]
fn same_action_sequence_prefix_is_resolved_so_the_keymap_still_builds() {
    let prefix = "Ctrl+Shift+Alt+K";
    let sequence = "Ctrl+Shift+Alt+K > Ctrl+Shift+Alt+C";
    let mut config = Config::default();
    config.keybindings.core.undo = vec![prefix.to_string(), sequence.to_string()];

    let report = config.validate_and_clamp();

    assert_eq!(config.keybindings.core.undo, [prefix]);
    config
        .keybindings
        .build_action_map()
        .expect("self-prefix must not discard the rest of the keymap");
    assert_eq!(report.keybinding_conflicts.len(), 1);
    let resolution = &report.keybinding_conflicts[0];
    assert!(!resolution.is_self_duplicate());
    assert_eq!(resolution.kept_key(), prefix);
    assert_eq!(resolution.dropped_key(), sequence);

    let mut config = Config::default();
    config.keybindings.core.undo = vec![sequence.to_string(), prefix.to_string()];
    config.validate_and_clamp();
    assert_eq!(config.keybindings.core.undo, [sequence]);
    config
        .keybindings
        .build_action_map()
        .expect("keeping the earlier sequence must still build");
}

#[test]
fn same_action_prefix_is_resolved_after_deduplicating_the_prefix() {
    let prefix = "Ctrl+Shift+Alt+K";
    let sequence = "Ctrl+Shift+Alt+K > Ctrl+Shift+Alt+C";
    let mut config = Config::default();
    config.keybindings.core.undo =
        vec![sequence.to_string(), prefix.to_string(), prefix.to_string()];

    let report = config.validate_and_clamp();

    assert_eq!(config.keybindings.core.undo, [sequence]);
    config
        .keybindings
        .build_action_map()
        .expect("a leftover prefix after dedup must not discard the keymap");
    assert!(!report.keybinding_conflicts.is_empty());
}

#[test]
fn same_action_prefix_is_resolved_when_another_action_also_claims_the_prefix() {
    let prefix = "Ctrl+Shift+Alt+K";
    let sequence = "Ctrl+Shift+Alt+K > Ctrl+Shift+Alt+C";

    let mut config = Config::default();
    config.keybindings.core.undo = vec![sequence.to_string(), prefix.to_string()];
    config.keybindings.core.redo = vec![prefix.to_string()];
    let report = config.validate_and_clamp();
    assert_eq!(config.keybindings.core.undo, [sequence]);
    assert!(config.keybindings.core.redo.is_empty());
    config
        .keybindings
        .build_action_map()
        .expect("cross-action prefix must not leave a same-action pair in the winner");
    assert!(
        report
            .keybinding_conflicts
            .iter()
            .any(|resolution| resolution.dropped() == Action::Redo),
        "{:?}",
        report.keybinding_conflicts
    );

    let mut config = Config::default();
    config.keybindings.core.undo = vec![prefix.to_string(), sequence.to_string()];
    config.keybindings.core.redo = vec![prefix.to_string()];
    config.validate_and_clamp();
    assert_eq!(config.keybindings.core.undo, [prefix]);
    assert!(config.keybindings.core.redo.is_empty());
    config
        .keybindings
        .build_action_map()
        .expect("keeping the winner's earlier prefix must still build");

    let mut config = Config::default();
    config.keybindings.core.undo = vec![sequence.to_string(), prefix.to_string()];
    config.keybindings.core.redo = vec![sequence.to_string()];
    config.validate_and_clamp();
    assert_eq!(config.keybindings.core.undo, [sequence]);
    assert!(config.keybindings.core.redo.is_empty());
    config
        .keybindings
        .build_action_map()
        .expect("keeping the winner's earlier sequence must still build");
}

#[test]
fn prefix_conflict_report_names_the_shortcuts_each_action_actually_held() {
    let prefix = "Ctrl+Shift+Alt+K";
    let sequence = "Ctrl+Shift+Alt+K > Ctrl+Shift+Alt+C";
    let mut config = Config::default();
    config.keybindings.core.undo = vec![prefix.to_string()];
    config.keybindings.core.redo = vec![sequence.to_string()];

    let report = config.validate_and_clamp();

    let resolution = report
        .keybinding_conflicts
        .first()
        .expect("prefix conflict must be reported");
    assert_eq!(resolution.key(), sequence);
    assert_eq!(resolution.kept_key(), prefix);
    assert_eq!(resolution.dropped_key(), sequence);
    assert_eq!(
        resolution.summary(),
        format!("{prefix} kept for Undo; conflicting {sequence} dropped from Redo.")
    );
    assert!(!resolution.is_self_duplicate());

    let mut config = Config::default();
    config.keybindings.core.undo = vec![prefix.to_string(), sequence.to_string()];
    let report = config.validate_and_clamp();
    let resolution = report
        .keybinding_conflicts
        .first()
        .expect("same-action prefix conflict must be reported");
    assert_eq!(resolution.kept_key(), prefix);
    assert_eq!(resolution.dropped_key(), sequence);
    assert_eq!(
        resolution.summary(),
        format!("{prefix} kept for Undo; conflicting {sequence} dropped from that action.")
    );
    assert!(!resolution.is_self_duplicate());
}

#[test]
fn validating_the_defaults_reports_nothing_to_surface() {
    assert!(Config::default().validate_and_clamp().is_empty());
}

/// The config file promises case-insensitive key names, so two spellings of
/// one chord are one shortcut: the collision is arbitrated like any other
/// duplicate instead of surviving as two map entries dispatch picks between.
#[test]
fn keybindings_differing_only_in_key_case_are_one_conflict() {
    let mut config = Config::default();
    config.keybindings.core.undo = vec!["ctrl+alt+u".to_string()];
    config.keybindings.core.redo = vec!["Ctrl+Alt+U".to_string()];

    let report = config.validate_and_clamp();

    assert_eq!(report.keybinding_conflicts.len(), 1);
    assert_eq!(report.keybinding_conflicts[0].kept(), Action::Undo);
    assert_eq!(report.keybinding_conflicts[0].dropped(), Action::Redo);
    assert_eq!(config.keybindings.core.undo, ["ctrl+alt+u"]);
    assert!(config.keybindings.core.redo.is_empty());
    config
        .keybindings
        .build_action_map()
        .expect("one spelling of the chord remains");
}

/// The repeat in the winner's own list is removed either way, so it is
/// reported even when other actions contest the same key — hearing only about
/// the cross-action side would leave that edit unexplained.
#[test]
fn a_self_duplicate_is_still_reported_when_the_key_is_also_contested() {
    let mut config = Config::default();
    config.keybindings.core.clear_canvas = vec![
        "Ctrl+Alt+Shift+5".to_string(),
        "Ctrl+Alt+Shift+5".to_string(),
    ];
    config.keybindings.core.undo = vec!["Ctrl+Alt+Shift+5".to_string()];

    let report = config.validate_and_clamp();

    assert_eq!(config.keybindings.core.clear_canvas, ["Ctrl+Alt+Shift+5"]);
    assert!(config.keybindings.core.undo.is_empty());
    assert_eq!(
        report.keybinding_conflicts.len(),
        2,
        "{:?}",
        report.keybinding_conflicts
    );
    let self_duplicate = report
        .keybinding_conflicts
        .iter()
        .find(|resolution| resolution.is_self_duplicate())
        .expect("the repeat in the kept list must be reported");
    assert_eq!(self_duplicate.kept(), Action::ClearCanvas);
    let cross = report
        .keybinding_conflicts
        .iter()
        .find(|resolution| !resolution.is_self_duplicate())
        .expect("the cross-action collision must be reported");
    assert_eq!(cross.kept(), Action::ClearCanvas);
    assert_eq!(cross.dropped(), Action::Undo);
}

/// Losing one key must not change what an omitted list is. The command palette
/// default owns two keys and two authored bindings take both; a list that has
/// been trimmed once is still an offer, not an authored claim.
#[test]
fn trimming_one_key_does_not_promote_an_omitted_list_to_authored() {
    let mut config = config_from_toml(
        "[keybindings]\nexit = [\"Ctrl+K\"]\ncapture_full_screen = [\"Ctrl+Shift+P\"]\n",
    );
    assert_eq!(
        config.keybindings.ui.toggle_command_palette,
        ["Ctrl+K", "Ctrl+Shift+P"],
        "fixture depends on the palette default owning both keys"
    );

    let report = config.validate_and_clamp();

    assert_eq!(config.keybindings.core.exit, ["Ctrl+K"]);
    assert_eq!(
        config.keybindings.capture.capture_full_screen,
        ["Ctrl+Shift+P"]
    );
    assert!(config.keybindings.ui.toggle_command_palette.is_empty());
    assert!(report.keybinding_conflicts.is_empty());
    assert_eq!(report.skipped_default_shortcuts.len(), 2);
    assert!(
        report
            .skipped_default_shortcuts
            .iter()
            .all(|skipped| skipped.action() == Action::ToggleCommandPalette),
        "both keys must come off the omitted side: {:?}",
        report.skipped_default_shortcuts
    );
}

/// An omitted action is offered its defaults in traversal order, and a key the
/// offer already installed for it is not offered twice.
#[test]
fn omitted_defaults_never_take_a_key_from_an_earlier_omitted_action() {
    let mut config = config_from_toml("[keybindings]\nundo = [\"Ctrl+Alt+U\"]\n");

    let report = config.validate_and_clamp();

    // Nothing in the shipped defaults collides, so every omitted action keeps
    // its whole list and there is nothing to report.
    assert_eq!(
        config.keybindings.ui.toggle_command_palette,
        ["Ctrl+K", "Ctrl+Shift+P"]
    );
    assert_eq!(config.keybindings.ui.cycle_toolbar_display, ["F2"]);
    assert!(report.is_empty(), "unexpected report: {report:?}");
    config
        .keybindings
        .build_action_map()
        .expect("the resolved keymap has no duplicates");
}

/// An editor that rebuilds `[keybindings]` from its own fields says so, and
/// the omitted-default pass then has nothing to run on: the same fixture that
/// loses the typed binding as an unauthored offer keeps it as an authored
/// claim, with the traversal order settling the collision it causes.
#[test]
fn marking_the_section_explicit_retires_the_omitted_default_pass() {
    let source = "[keybindings]\nundo = [\"Ctrl+Alt+U\"]\n";

    // How the file reads on its own: `clear_canvas` is absent, so the value it
    // carries is an offer this build made and the authored `undo` outranks it.
    let mut from_file = config_from_toml(source);
    from_file.keybindings.core.clear_canvas = vec!["Ctrl+Alt+U".to_string()];
    let filtered = from_file.validate_and_clamp();
    assert!(from_file.keybindings.core.clear_canvas.is_empty());
    assert_eq!(filtered.skipped_default_shortcuts.len(), 1);

    // The same values from an editor that typed them: both lists are authored.
    let mut edited = config_from_toml(source);
    edited.keybindings.core.clear_canvas = vec!["Ctrl+Alt+U".to_string()];
    edited.mark_keybindings_explicit();
    let report = edited.validate_and_clamp();

    assert!(
        report.skipped_default_shortcuts.is_empty(),
        "nothing was omitted: {:?}",
        report.skipped_default_shortcuts
    );
    // `core` visits `clear_canvas` before `undo`, so the typed list keeps the
    // key and the older one loses it.
    assert_eq!(edited.keybindings.core.clear_canvas, ["Ctrl+Alt+U"]);
    assert!(edited.keybindings.core.undo.is_empty());
    assert_eq!(report.keybinding_conflicts.len(), 1);
    assert_eq!(report.keybinding_conflicts[0].kept(), Action::ClearCanvas);
    assert_eq!(report.keybinding_conflicts[0].dropped(), Action::Undo);
    edited
        .keybindings
        .build_action_map()
        .expect("the resolved keymap has no duplicates");
}

/// A single mistyped string used to fail `build_action_map` for the whole
/// config, and the runtime answered that by installing the complete shipped
/// defaults for the session. Only the typo is dropped now.
#[test]
fn an_unparseable_binding_costs_only_itself() {
    let mut config = Config::default();
    config.keybindings.core.clear_canvas = vec!["Ctrl+Shift".to_string(), "Ctrl+L".to_string()];
    config.keybindings.core.undo = vec!["Ctrl+Alt+U".to_string()];

    let report = config.validate_and_clamp();

    assert_eq!(config.keybindings.core.clear_canvas, ["Ctrl+L"]);
    assert_eq!(config.keybindings.core.undo, ["Ctrl+Alt+U"]);
    assert_eq!(
        config.keybindings.tools.select_pen_tool,
        KeybindingsConfig::default().tools.select_pen_tool,
        "an action the file never touched keeps its default"
    );
    config
        .keybindings
        .build_action_map()
        .expect("the keymap builds once the typo is gone");

    assert_eq!(report.invalid_keybindings.len(), 1);
    assert_eq!(report.invalid_keybindings[0].binding(), "Ctrl+Shift");
    assert_eq!(
        report.invalid_keybindings[0].config_key(),
        Some("clear_canvas")
    );
    let reported = report.invalid_keybindings[0].to_string();
    assert!(
        reported.contains("Ctrl+Shift") && reported.contains("Clear Canvas"),
        "unexpected report: {reported}"
    );
    assert!(!report.is_empty());
    assert!(report.keybinding_conflicts.is_empty());
}

/// The drop happens before duplicates are arbitrated, so a typo cannot make a
/// key look free and it cannot be mistaken for a collision either.
#[test]
fn an_unparseable_binding_does_not_disturb_conflict_resolution() {
    let mut config = Config::default();
    config.keybindings.core.clear_canvas = vec!["Ctrl+Shift".to_string()];
    config.keybindings.core.exit = vec!["Ctrl+Alt+Shift+E".to_string()];
    config.keybindings.tools.select_pen_tool = vec!["Ctrl+Alt+Shift+E".to_string()];

    let report = config.validate_and_clamp();

    assert!(config.keybindings.core.clear_canvas.is_empty());
    assert_eq!(config.keybindings.core.exit, ["Ctrl+Alt+Shift+E"]);
    assert!(config.keybindings.tools.select_pen_tool.is_empty());
    assert_eq!(report.invalid_keybindings.len(), 1);
    assert_eq!(report.keybinding_conflicts.len(), 1);
    assert_eq!(
        report.keybinding_conflicts[0].dropped(),
        Action::SelectPenTool
    );
}

#[test]
fn validate_and_clamp_drops_blank_and_repeated_font_cycle_entries() {
    let mut config = Config::default();
    config.drawing.font_cycle = vec![
        "  Sans  ".to_string(),
        String::new(),
        "Serif".to_string(),
        "   ".to_string(),
        "Sans".to_string(),
    ];

    config.validate_and_clamp();

    assert_eq!(config.drawing.font_cycle, ["Sans", "Serif"]);
}

#[test]
fn a_font_cycle_repeat_in_another_case_is_still_a_repeat() {
    // Fontconfig resolves a family name without regard to case, so these two
    // entries are one font. Keeping both would leave a step that changes the
    // spelling and nothing a viewer can see.
    let mut config = Config::default();
    config.drawing.font_cycle = vec!["Sans".to_string(), "sans".to_string()];

    config.validate_and_clamp();

    assert_eq!(config.drawing.font_cycle, ["Sans"]);
}
