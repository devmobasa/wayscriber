use super::super::*;
use crate::input::state::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};

#[test]
fn validate_and_clamp_clamps_out_of_range_values() {
    let mut config = Config::default();
    config.drawing.default_thickness = 80.0;
    config.drawing.default_font_size = 3.0;
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
            items: vec![
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
                    id: "print".to_string(),
                    name: " ".to_string(),
                    mappings: Vec::new(),
                },
            ],
        },
        ..Config::default()
    };

    config.validate_and_clamp();

    assert_eq!(config.render_profiles.active.as_deref(), Some("print"));
    assert_eq!(config.render_profiles.items[0].id, "print");
    assert_eq!(config.render_profiles.items[0].name, "Print Friendly");
    assert_eq!(config.render_profiles.items[1].id, "print-2");
    assert_eq!(config.render_profiles.items[1].name, "Profile 2");
    assert_eq!(
        config.render_profiles.items[0].mappings,
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
            items: vec![RenderProfileConfig {
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
    assert_eq!(
        config.keybindings.core.exit,
        KeybindingsConfig::default().core.exit
    );
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

#[cfg(tablet)]
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

    assert_eq!(config.drawing.drag_tool, crate::input::Tool::Pen);
    assert_eq!(config.drawing.shift_drag_tool, crate::input::Tool::Line);
    assert_eq!(config.drawing.ctrl_drag_tool, crate::input::Tool::Rect);
    assert_eq!(
        config.drawing.ctrl_shift_drag_tool,
        crate::input::Tool::Arrow
    );
    assert_eq!(config.drawing.tab_drag_tool, crate::input::Tool::Ellipse);
}
