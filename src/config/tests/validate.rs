use super::super::*;
use crate::input::state::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};

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
    // A binding string we cannot parse is a typo for the user to fix. It is
    // not grounds for discarding every other shortcut they configured (#293),
    // so the authored text survives and only the runtime keymap ignores it.
    assert_eq!(config.keybindings.core.exit, ["Ctrl+Shift"]);
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
fn validate_and_clamp_resets_non_finite_spotlight_settings() {
    let defaults = Config::default().spotlight;

    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut config = Config::default();
        config.spotlight.dim_opacity = invalid;
        config.spotlight.feather = invalid;

        config.validate_and_clamp();

        assert_eq!(config.spotlight.dim_opacity, defaults.dim_opacity);
        assert_eq!(config.spotlight.feather, defaults.feather);
    }
}

#[test]
fn legacy_command_palette_and_capture_defaults_migrate_as_a_pair() {
    let mut config = Config {
        config_revision: 0,
        ..Config::default()
    };
    config.keybindings.ui.toggle_command_palette = vec!["Ctrl+K".to_string()];
    config.keybindings.capture.capture_full_screen = vec!["Ctrl+Shift+P".to_string()];

    config.validate_and_clamp();

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
    legacy_command.validate_and_clamp();
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
    legacy_capture.validate_and_clamp();
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
    custom_command.validate_and_clamp();
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
    custom_capture.validate_and_clamp();
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

    config.validate_and_clamp();

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

    config.validate_and_clamp();

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

    config.validate_and_clamp();

    assert_eq!(config.keybindings.ui.toggle_input_hud, ["Ctrl+Shift+K"]);
    assert_eq!(config.config_revision, CURRENT_CONFIG_REVISION);
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
/// must come through untouched.
#[test]
fn keybinding_conflict_costs_the_serde_filled_default_only_the_contested_key() {
    let mut config = Config::default();
    config.keybindings.ui.toggle_toolbar = vec!["F2".to_string(), "F9".to_string()];
    config.keybindings.core.exit = vec!["Escape".to_string(), "Q".to_string()];

    let report = config.validate_and_clamp();

    assert_eq!(config.keybindings.ui.toggle_toolbar, ["F2", "F9"]);
    assert!(config.keybindings.ui.cycle_toolbar_display.is_empty());
    assert_eq!(config.keybindings.core.exit, ["Escape", "Q"]);
    assert!(config.keybindings.build_action_map().is_ok());

    assert_eq!(report.keybinding_conflicts.len(), 1);
    let resolution = &report.keybinding_conflicts[0];
    assert_eq!(resolution.key(), "F2");
    assert_eq!(resolution.kept(), Action::ToggleToolbar);
    assert_eq!(resolution.dropped(), Action::CycleToolbarDisplay);
    assert_eq!(
        resolution.dropped_config_key(),
        Some("cycle_toolbar_display")
    );
}

/// A default that only overlaps part of an authored list keeps the rest of its
/// own keys: resolution is per binding, not per action.
#[test]
fn keybinding_conflict_leaves_the_defaults_other_keys_alone() {
    let mut config = Config::default();
    assert_eq!(
        config.keybindings.ui.toggle_command_palette,
        ["Ctrl+K", "Ctrl+Shift+P"]
    );
    config.keybindings.capture.capture_full_screen = vec!["Ctrl+Shift+P".to_string()];

    let report = config.validate_and_clamp();

    assert_eq!(
        config.keybindings.capture.capture_full_screen,
        ["Ctrl+Shift+P"]
    );
    assert_eq!(config.keybindings.ui.toggle_command_palette, ["Ctrl+K"]);
    assert_eq!(report.keybinding_conflicts.len(), 1);
    assert_eq!(
        report.keybinding_conflicts[0].dropped(),
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

/// Resolution order does not depend on which side happens to be listed first
/// in the file: an authored list always outranks a defaulted one.
#[test]
fn keybinding_conflict_prefers_the_authored_side_over_traversal_order() {
    let mut config = Config::default();
    // `core.exit` is traversed first but still holds its shipped default, so
    // the authored capture binding wins even though it is visited later.
    config.keybindings.capture.capture_selection = vec!["Escape".to_string()];

    let report = config.validate_and_clamp();

    assert_eq!(config.keybindings.capture.capture_selection, ["Escape"]);
    assert_eq!(config.keybindings.core.exit, ["Ctrl+Q"]);
    assert_eq!(report.keybinding_conflicts.len(), 1);
    assert_eq!(
        report.keybinding_conflicts[0].kept(),
        Action::CaptureSelection
    );
    assert_eq!(report.keybinding_conflicts[0].dropped(), Action::Exit);
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
fn validating_the_defaults_reports_nothing_to_surface() {
    assert!(Config::default().validate_and_clamp().is_empty());
}

/// Resolving one key must not reclassify the list it just trimmed. The command
/// palette default owns two keys; losing the first one to an authored binding
/// cannot make it look authored when the second is arbitrated.
#[test]
fn resolving_one_key_does_not_promote_a_trimmed_default_over_an_authored_binding() {
    let mut config = Config::default();
    assert_eq!(
        config.keybindings.ui.toggle_command_palette,
        ["Ctrl+K", "Ctrl+Shift+P"],
        "fixture depends on the palette default owning both keys"
    );
    config.keybindings.core.exit = vec!["Ctrl+K".to_string()];
    config.keybindings.capture.capture_full_screen = vec!["Ctrl+Shift+P".to_string()];

    let report = config.validate_and_clamp();

    assert_eq!(config.keybindings.core.exit, ["Ctrl+K"]);
    assert_eq!(
        config.keybindings.capture.capture_full_screen,
        ["Ctrl+Shift+P"]
    );
    assert!(config.keybindings.ui.toggle_command_palette.is_empty());
    assert_eq!(report.keybinding_conflicts.len(), 2);
    assert!(
        report
            .keybinding_conflicts
            .iter()
            .all(|resolution| resolution.dropped() == Action::ToggleCommandPalette),
        "both keys must come off the defaulted side: {:?}",
        report.keybinding_conflicts
    );
}
