use super::super::*;
use super::helpers::dummy_input_state;
use crate::config::Action;
use crate::draw::{Color, FontDescriptor, Frame, PageDeleteOutcome, Shape};
use crate::input::BOARD_ID_BLACKBOARD;
use crate::input::state::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};
use crate::input::{EraserMode, PerToolDrawingSettings, Tool};
use std::path::PathBuf;

#[test]
fn snapshot_skips_when_empty_and_no_tool_state() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "test");
    options.persist_transparent = true;
    options.restore_tool_state = false;
    options.max_shapes_per_frame = 100;
    options.max_file_size_bytes = 1024 * 1024;
    options.compression = CompressionMode::Off;
    options.auto_compress_threshold_bytes = DEFAULT_AUTO_COMPRESS_THRESHOLD_BYTES;

    let input = dummy_input_state();
    assert!(snapshot_from_input(&input, &options).is_none());
}

#[test]
fn snapshot_includes_frames_and_tool_state() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display");
    options.persist_transparent = true;

    let mut input = dummy_input_state();
    input.boards.active_frame_mut().add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 10,
        y2: 10,
        color: Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 2.0,
    });

    let snapshot = snapshot_from_input(&input, &options).expect("snapshot present");
    assert!(
        snapshot
            .boards
            .iter()
            .any(|board| board.id == "transparent")
    );
    assert!(snapshot.tool_state.is_some());
}

#[test]
fn snapshot_uses_pre_light_mode_tool_state() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display-light");
    options.restore_tool_state = true;

    let mut input = dummy_input_state();
    input.compositor_capabilities.layer_shell = true;
    let desired_color = Color {
        r: 0.1,
        g: 0.2,
        b: 0.7,
        a: 1.0,
    };
    let _ = input.set_tool_override(Some(Tool::Marker));
    let _ = input.set_color(desired_color);
    let _ = input.set_thickness(14.0);
    input.show_status_bar = true;

    input.handle_action(Action::ToggleLightMode);
    assert!(input.light_mode);
    assert_eq!(input.tool_override(), Some(Tool::Pen));
    assert!(!input.show_status_bar);

    let snapshot = snapshot_from_input(&input, &options).expect("snapshot present");
    let tool_state = snapshot.tool_state.expect("tool state present");

    assert_eq!(tool_state.tool_override, Some(Tool::Marker));
    assert_eq!(tool_state.current_color, desired_color);
    assert_eq!(tool_state.current_thickness, 14.0);
    assert!(tool_state.show_status_bar);
}

#[test]
fn apply_snapshot_restores_tool_state() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display-tools");
    options.restore_tool_state = true;

    let mut input = dummy_input_state();
    let desired_color = Color {
        r: 0.2,
        g: 0.4,
        b: 0.6,
        a: 1.0,
    };
    let _ = input.set_tool_override(Some(Tool::Rect));
    let _ = input.set_color(desired_color);
    let _ = input.set_thickness(18.0);
    let _ = input.set_eraser_size(22.0);
    let _ = input.set_eraser_mode(EraserMode::Stroke);
    let _ = input.set_marker_opacity(0.55);
    let _ = input.set_fill_enabled(true);
    let desired_font = FontDescriptor::new(
        "Monospace".to_string(),
        "normal".to_string(),
        "italic".to_string(),
    );
    let _ = input.set_font_descriptor(desired_font.clone());
    let _ = input.set_font_size(48.0);
    input.text_background_enabled = true;
    input.arrow_length = 40.0;
    input.arrow_angle = 45.0;
    input.arrow_head_at_end = true;
    input.arrow_label_enabled = true;
    input.polygon_sides = 9;
    input.board_previous_color = Some(Color {
        r: 0.9,
        g: 0.2,
        b: 0.1,
        a: 1.0,
    });
    input.show_status_bar = false;

    let snapshot = snapshot_from_input(&input, &options).expect("snapshot present");
    assert_eq!(
        snapshot
            .tool_state
            .as_ref()
            .and_then(|state| state.font_descriptor.as_ref()),
        Some(&desired_font)
    );

    let mut restored = dummy_input_state();
    apply_snapshot(&mut restored, snapshot, &options);

    assert_eq!(restored.current_color, desired_color);
    assert_eq!(restored.current_thickness, 18.0);
    assert_eq!(restored.eraser_size, 22.0);
    assert_eq!(restored.eraser_mode, EraserMode::Stroke);
    assert_eq!(restored.marker_opacity, 0.55);
    assert!(restored.fill_enabled);
    assert_eq!(restored.font_descriptor, desired_font);
    assert_eq!(restored.current_font_size, 48.0);
    assert_eq!(restored.tool_override(), Some(Tool::Rect));
    assert!(restored.text_background_enabled);
    assert_eq!(restored.arrow_length, 40.0);
    assert_eq!(restored.arrow_angle, 45.0);
    assert!(restored.arrow_head_at_end);
    assert!(restored.arrow_label_enabled);
    assert_eq!(restored.polygon_sides, 9);
    assert_eq!(
        restored.board_previous_color,
        Some(Color {
            r: 0.9,
            g: 0.2,
            b: 0.1,
            a: 1.0,
        })
    );
    assert!(!restored.show_status_bar);
}

#[test]
fn apply_snapshot_clamps_restored_polygon_sides() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display-polygon-sides");
    options.restore_tool_state = true;

    let mut input = dummy_input_state();
    input.polygon_sides = 255;
    let snapshot = snapshot_from_input(&input, &options).expect("snapshot present");

    let mut restored = dummy_input_state();
    apply_snapshot(&mut restored, snapshot, &options);

    assert_eq!(restored.polygon_sides, 12);
}

#[test]
fn apply_legacy_snapshot_preserves_config_initialized_font_descriptor() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display-legacy-font");
    options.restore_tool_state = true;

    let config_font = FontDescriptor::new(
        "JetBrains Mono".to_string(),
        "medium".to_string(),
        "normal".to_string(),
    );
    let snapshot = SessionSnapshot {
        active_board_id: "transparent".to_string(),
        boards: vec![],
        tool_state: Some(ToolStateSnapshot {
            current_color: Color {
                r: 0.0,
                g: 1.0,
                b: 0.0,
                a: 1.0,
            },
            current_thickness: 4.0,
            eraser_size: 12.0,
            eraser_kind: crate::draw::EraserKind::Circle,
            eraser_mode: EraserMode::Brush,
            marker_opacity: Some(0.32),
            fill_enabled: Some(false),
            tool_override: None,
            current_font_size: 40.0,
            font_descriptor: None,
            text_background_enabled: false,
            arrow_length: 20.0,
            arrow_angle: 30.0,
            arrow_head_at_end: Some(false),
            arrow_label_enabled: Some(false),
            polygon_sides: crate::draw::REGULAR_POLYGON_DEFAULT_SIDES,
            board_previous_color: None,
            show_status_bar: true,
            tool_settings: None,
        }),
    };

    let mut restored = dummy_input_state();
    let _ = restored.set_font_descriptor(config_font.clone());

    apply_snapshot(&mut restored, snapshot, &options);

    assert_eq!(restored.font_descriptor, config_font);
    assert_eq!(restored.current_font_size, 40.0);
}

#[test]
fn apply_snapshot_clamps_restored_per_tool_thicknesses() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display-tool-clamp");
    options.restore_tool_state = true;

    let desired_color = Color {
        r: 0.2,
        g: 0.4,
        b: 0.6,
        a: 1.0,
    };
    let mut tool_settings = PerToolDrawingSettings::new(desired_color, 3.0);
    tool_settings.pen.thickness = MAX_STROKE_THICKNESS + 100.0;
    tool_settings.marker.thickness = MIN_STROKE_THICKNESS - 100.0;

    let snapshot = SessionSnapshot {
        active_board_id: "transparent".to_string(),
        boards: vec![],
        tool_state: Some(ToolStateSnapshot {
            current_color: desired_color,
            current_thickness: 3.0,
            eraser_size: 12.0,
            eraser_kind: crate::draw::EraserKind::Circle,
            eraser_mode: EraserMode::Brush,
            marker_opacity: Some(0.32),
            fill_enabled: Some(false),
            tool_override: Some(Tool::Pen),
            current_font_size: 32.0,
            font_descriptor: Some(FontDescriptor::default()),
            text_background_enabled: false,
            arrow_length: 20.0,
            arrow_angle: 30.0,
            arrow_head_at_end: Some(false),
            arrow_label_enabled: Some(false),
            polygon_sides: crate::draw::REGULAR_POLYGON_DEFAULT_SIDES,
            board_previous_color: None,
            show_status_bar: true,
            tool_settings: Some(tool_settings),
        }),
    };

    let mut restored = dummy_input_state();
    apply_snapshot(&mut restored, snapshot, &options);

    assert_eq!(restored.thickness_for_tool(Tool::Pen), MAX_STROKE_THICKNESS);
    assert_eq!(
        restored.thickness_for_tool(Tool::Marker),
        MIN_STROKE_THICKNESS
    );
}

#[test]
fn apply_legacy_snapshot_uses_font_derived_step_marker_size() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display-legacy-step-size");
    options.restore_tool_state = true;

    let color = Color {
        r: 0.2,
        g: 0.4,
        b: 0.6,
        a: 1.0,
    };
    let snapshot = SessionSnapshot {
        active_board_id: "transparent".to_string(),
        boards: vec![],
        tool_state: Some(ToolStateSnapshot {
            current_color: color,
            current_thickness: 3.0,
            eraser_size: 12.0,
            eraser_kind: crate::draw::EraserKind::Circle,
            eraser_mode: EraserMode::Brush,
            marker_opacity: Some(0.32),
            fill_enabled: Some(false),
            tool_override: Some(Tool::StepMarker),
            current_font_size: 48.0,
            font_descriptor: None,
            text_background_enabled: false,
            arrow_length: 20.0,
            arrow_angle: 30.0,
            arrow_head_at_end: Some(false),
            arrow_label_enabled: Some(false),
            polygon_sides: crate::draw::REGULAR_POLYGON_DEFAULT_SIDES,
            board_previous_color: None,
            show_status_bar: true,
            tool_settings: None,
        }),
    };

    let mut restored = dummy_input_state();
    apply_snapshot(&mut restored, snapshot, &options);

    assert!((restored.thickness_for_tool(Tool::StepMarker) - 28.8).abs() < 1e-9);
    assert!((restored.next_step_marker_label().size - 28.8).abs() < 1e-9);
}

#[test]
fn apply_snapshot_keeps_current_board_when_active_board_is_missing() {
    let options = SessionOptions::new(PathBuf::from("/tmp"), "display-missing-board");
    let mut input = dummy_input_state();
    input.switch_board_force("whiteboard");

    let snapshot = SessionSnapshot {
        active_board_id: "missing".to_string(),
        boards: vec![BoardSnapshot {
            id: "transparent".to_string(),
            pages: BoardPagesSnapshot {
                pages: vec![Frame::new()],
                active: 0,
            },
        }],
        tool_state: None,
    };

    apply_snapshot(&mut input, snapshot, &options);

    assert_eq!(input.board_id(), "whiteboard");
}

#[test]
fn apply_snapshot_clears_pending_board_delete_confirmation() {
    let options = SessionOptions::new(PathBuf::from("/tmp"), "display-board-confirm");
    let mut input = dummy_input_state();
    input.switch_board_force(BOARD_ID_BLACKBOARD);
    let board_count = input.boards.board_count();

    input.delete_active_board();
    assert!(input.has_pending_board_delete());
    assert!(
        input
            .ui_toast
            .as_ref()
            .and_then(|toast| toast.action.as_ref())
            .is_some_and(|action| action.action == Action::BoardDelete)
    );
    input.ui_toast_bounds = Some((10.0, 20.0, 100.0, 40.0));

    let snapshot = SessionSnapshot {
        active_board_id: BOARD_ID_BLACKBOARD.to_string(),
        boards: vec![BoardSnapshot {
            id: BOARD_ID_BLACKBOARD.to_string(),
            pages: BoardPagesSnapshot {
                pages: vec![Frame::new()],
                active: 0,
            },
        }],
        tool_state: None,
    };

    apply_snapshot(&mut input, snapshot, &options);

    assert!(!input.has_pending_board_delete());
    assert!(input.ui_toast.is_none());
    assert!(input.ui_toast_bounds.is_none());
    input.delete_active_board();
    assert_eq!(input.boards.board_count(), board_count);
    assert!(input.has_pending_board_delete());
}

#[test]
fn apply_snapshot_clears_pending_page_delete_confirmation() {
    let options = SessionOptions::new(PathBuf::from("/tmp"), "display-page-confirm");
    let mut input = dummy_input_state();
    input.switch_board_force(BOARD_ID_BLACKBOARD);
    input.page_new();

    assert_eq!(input.page_delete(), PageDeleteOutcome::Pending);
    assert!(input.has_pending_page_delete());
    assert!(
        input
            .ui_toast
            .as_ref()
            .and_then(|toast| toast.action.as_ref())
            .is_some_and(|action| action.action == Action::PageDelete)
    );
    input.ui_toast_bounds = Some((10.0, 20.0, 100.0, 40.0));

    let snapshot = SessionSnapshot {
        active_board_id: BOARD_ID_BLACKBOARD.to_string(),
        boards: vec![BoardSnapshot {
            id: BOARD_ID_BLACKBOARD.to_string(),
            pages: BoardPagesSnapshot {
                pages: vec![Frame::new()],
                active: 0,
            },
        }],
        tool_state: None,
    };

    apply_snapshot(&mut input, snapshot, &options);

    assert!(!input.has_pending_page_delete());
    assert!(input.ui_toast.is_none());
    assert!(input.ui_toast_bounds.is_none());
    assert_eq!(input.boards.page_count(), 1);
}
