use super::super::*;
use crate::input::state::MAX_STROKE_THICKNESS;

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
    config.presets.slot_1 = Some(ToolPresetConfig {
        name: None,
        tool: crate::input::Tool::Pen,
        color: ColorSpec::Name("red".to_string()),
        size: 120.0,
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
    });

    config.validate_and_clamp();

    assert_eq!(config.presets.slot_count, PRESET_SLOTS_MIN);
    let preset = config.presets.slot_1.as_ref().expect("slot_1 preset");
    assert_eq!(preset.size, MAX_STROKE_THICKNESS);
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
    config.ui.click_highlight.fill_color = [2.0, -1.0, 0.5, 0.5];
    config.ui.click_highlight.outline_color = [-0.2, 2.0, 0.5, 1.2];
    config.session.max_shapes_per_frame = 0;
    config.session.max_file_size_mb = 2048;
    config.session.auto_compress_threshold_kb = 0;
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
    assert_eq!(config.session.max_shapes_per_frame, 1);
    assert_eq!(config.session.max_file_size_mb, 1024);
    assert_eq!(config.session.auto_compress_threshold_kb, 1);
    assert!(matches!(config.session.storage, SessionStorageMode::Auto));
    assert!(config.session.custom_directory.is_none());
    assert_eq!(
        config.keybindings.core.exit,
        KeybindingsConfig::default().core.exit
    );
}
