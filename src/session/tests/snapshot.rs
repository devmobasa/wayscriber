use super::super::*;
use super::helpers::dummy_input_state;
use crate::config::{Action, KeybindingsConfig};
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
fn non_default_pen_smoothing_survives_snapshot_serialization_and_restore() {
    let mut source = dummy_input_state();
    let _ = source.set_pen_smoothing(5);
    assert_eq!(source.pen_smoothing, 5, "the fixture must be non-default");

    let captured = ToolStateSnapshot::from_input_state(&source);
    let encoded = serde_json::to_vec(&captured).expect("serialize tool snapshot");
    let decoded: ToolStateSnapshot =
        serde_json::from_slice(&encoded).expect("deserialize tool snapshot");

    let mut restored = dummy_input_state();
    let _ = restored.set_pen_smoothing(1);
    apply_tool_state_snapshot(&mut restored, decoded);

    assert_eq!(restored.pen_smoothing, 5);
}

#[test]
fn legacy_snapshot_without_pen_smoothing_preserves_the_configured_level() {
    let source = dummy_input_state();
    let mut legacy =
        serde_json::to_value(ToolStateSnapshot::from_input_state(&source)).expect("tool snapshot");
    let object = legacy.as_object_mut().expect("tool snapshot is an object");
    assert!(
        object.remove("pen_smoothing").is_some(),
        "the fixture must remove a field current sessions write"
    );
    let decoded: ToolStateSnapshot =
        serde_json::from_value(legacy).expect("legacy tool snapshot still loads");
    assert_eq!(decoded.pen_smoothing, None);

    let mut restored = dummy_input_state();
    let _ = restored.set_pen_smoothing(4);
    apply_tool_state_snapshot(&mut restored, decoded);

    assert_eq!(
        restored.pen_smoothing, 4,
        "a missing legacy field must leave the config-seeded value alone"
    );
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
    input.ui_visibility.show_status_bar = true;

    input.handle_action(Action::ToggleLightMode);
    assert!(input.light_mode);
    assert_eq!(input.tool_override(), Some(Tool::Pen));
    assert!(!input.ui_visibility.show_status_bar);

    let snapshot = snapshot_from_input(&input, &options).expect("snapshot present");
    let tool_state = snapshot.tool_state.expect("tool state present");

    assert_eq!(tool_state.tool_override, Some(Tool::Marker));
    assert_eq!(tool_state.current_color, desired_color);
    assert_eq!(tool_state.current_thickness, 14.0);
}

/// Toggling the status bar from the toolbar promises "applies to this run",
/// and nothing writes it to `config.toml`. A session saved afterwards — which
/// happens as soon as the user draws — must not bring the toggle back on the
/// next start, or the session would be a side channel that makes it durable
/// and outranks `[ui] show_status_bar`.
#[test]
fn restoring_a_session_keeps_the_configured_status_bar_visibility() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display-status-bar");
    options.restore_tool_state = true;

    let mut source = dummy_input_state();
    source.ui_visibility.show_status_bar = false;
    let snapshot = snapshot_from_input(&source, &options).expect("snapshot present");

    // The next start: chrome seeded from a configuration that shows the bar.
    let mut input = dummy_input_state();
    input.ui_visibility.show_status_bar = true;

    apply_snapshot(&mut input, snapshot, &options);

    assert!(
        input.ui_visibility.show_status_bar,
        "the configured value owns chrome; a session must not override it"
    );
}

/// A shortcut rebound in the overlay is keymap state for this run, not session
/// state: `ToolStateSnapshot` has no bindings field at all. This pins the
/// consequence — a session saved after a rebind carries nothing that could
/// reinstate it, and applying such a snapshot leaves the next run's configured
/// keymap exactly as it was seeded.
#[test]
fn restoring_a_session_never_carries_a_rebound_shortcut() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display-keymap");
    options.restore_tool_state = true;

    // The run that rebinds, installing the maps the same way the backend's
    // shortcut handler does.
    let mut source = dummy_input_state();
    let mut rebound = KeybindingsConfig::default();
    rebound
        .set_bindings_for_action(Action::SelectPenTool, vec!["Ctrl+Alt+Shift+K".to_string()])
        .expect("the pen tool stores a shortcut");
    source.set_keybinding_maps(
        rebound.build_action_map().expect("action map"),
        rebound.build_action_bindings().expect("action bindings"),
    );
    // Label spelling is the formatter's business (it fixes modifier order); all
    // this needs is that the rebind actually moved the binding.
    let rebound_labels = source.action_binding_labels(Action::SelectPenTool);
    assert_eq!(rebound_labels.len(), 1);
    assert!(
        rebound_labels[0].to_ascii_lowercase().ends_with("+k"),
        "the rebind should have landed: {rebound_labels:?}"
    );
    let snapshot = snapshot_from_input(&source, &options).expect("snapshot present");

    // The next start, seeded from the unchanged configuration.
    let mut input = dummy_input_state();
    let configured = KeybindingsConfig::default();
    input.set_keybinding_maps(
        configured.build_action_map().expect("action map"),
        configured.build_action_bindings().expect("action bindings"),
    );
    let before = input.action_binding_labels(Action::SelectPenTool);
    assert!(!before.is_empty(), "the fixture must bind the pen tool");
    assert_ne!(
        before, rebound_labels,
        "the two runs must disagree, or the test proves nothing"
    );

    apply_snapshot(&mut input, snapshot, &options);

    assert_eq!(
        input.action_binding_labels(Action::SelectPenTool),
        before,
        "a session restore must not change any shortcut"
    );
}

/// Focus Mode holds the value chrome returns to when it releases it. A restore
/// that carries no chrome has to leave that pending value alone as well, not
/// just the live one.
#[test]
fn restoring_a_session_leaves_focus_modes_pending_status_bar_value_alone() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display-focus-apply");
    options.restore_tool_state = true;

    let mut source = dummy_input_state();
    source.ui_visibility.show_status_bar = false;
    let snapshot = snapshot_from_input(&source, &options).expect("snapshot present");

    let mut input = dummy_input_state();
    input.ui_visibility.show_status_bar = true;
    input.handle_action(Action::ToggleFocusMode);
    assert!(input.focus_mode_active());
    assert!(!input.ui_visibility.show_status_bar);

    apply_snapshot(&mut input, snapshot, &options);
    assert!(input.focus_mode_active());
    assert!(
        !input.ui_visibility.show_status_bar,
        "session restore must not reveal chrome through Focus Mode"
    );

    input.handle_action(Action::ToggleFocusMode);
    assert!(
        input.ui_visibility.show_status_bar,
        "leaving Focus Mode returns this run's own value, not one from a session file"
    );
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
    let _ = input.set_spotlight_magnification(2.25);
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
    input.ui_visibility.show_status_bar = false;

    let snapshot = snapshot_from_input(&input, &options).expect("snapshot present");
    assert_eq!(
        snapshot
            .tool_state
            .as_ref()
            .and_then(|state| state.font_descriptor.as_ref()),
        Some(&desired_font)
    );

    let mut restored = dummy_input_state();
    restored.ui_visibility.show_status_bar = true;
    apply_snapshot(&mut restored, snapshot, &options);

    assert_eq!(restored.current_color, desired_color);
    assert_eq!(restored.current_thickness, 18.0);
    assert_eq!(restored.eraser_size, 22.0);
    assert_eq!(restored.eraser_mode, EraserMode::Stroke);
    assert_eq!(restored.marker_opacity, 0.55);
    assert_eq!(restored.spotlight_magnification, 2.25);
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
    assert!(
        restored.ui_visibility.show_status_bar,
        "the source hid the status bar, but chrome is not tool state and does not travel"
    );
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
            blur_style: Default::default(),
            recent_colors: Vec::new(),
            pen_smoothing: None,
            marker_opacity: Some(0.32),
            spotlight_magnification: None,
            fill_enabled: Some(false),
            tool_override: None,
            current_font_size: 40.0,
            font_descriptor: None,
            text_background_enabled: false,
            arrow_length: 20.0,
            arrow_angle: 30.0,
            arrow_head_at_end: Some(false),
            arrow_style: None,
            arrow_label_enabled: Some(false),
            polygon_sides: crate::draw::REGULAR_POLYGON_DEFAULT_SIDES,
            board_previous_color: None,
            tool_settings: None,
        }),
    };

    let mut restored = dummy_input_state();
    let _ = restored.set_font_descriptor(config_font.clone());
    restored.spotlight_magnification = 2.1;

    apply_snapshot(&mut restored, snapshot, &options);

    assert_eq!(restored.font_descriptor, config_font);
    assert_eq!(restored.current_font_size, 40.0);
    assert_eq!(restored.spotlight_magnification, 2.1);
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
            blur_style: Default::default(),
            recent_colors: Vec::new(),
            pen_smoothing: None,
            marker_opacity: Some(0.32),
            spotlight_magnification: None,
            fill_enabled: Some(false),
            tool_override: Some(Tool::Pen),
            current_font_size: 32.0,
            font_descriptor: Some(FontDescriptor::default()),
            text_background_enabled: false,
            arrow_length: 20.0,
            arrow_angle: 30.0,
            arrow_head_at_end: Some(false),
            arrow_style: None,
            arrow_label_enabled: Some(false),
            polygon_sides: crate::draw::REGULAR_POLYGON_DEFAULT_SIDES,
            board_previous_color: None,
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
            blur_style: Default::default(),
            recent_colors: Vec::new(),
            pen_smoothing: None,
            marker_opacity: Some(0.32),
            spotlight_magnification: None,
            fill_enabled: Some(false),
            tool_override: Some(Tool::StepMarker),
            current_font_size: 48.0,
            font_descriptor: None,
            text_background_enabled: false,
            arrow_length: 20.0,
            arrow_angle: 30.0,
            arrow_head_at_end: Some(false),
            arrow_style: None,
            arrow_label_enabled: Some(false),
            polygon_sides: crate::draw::REGULAR_POLYGON_DEFAULT_SIDES,
            board_previous_color: None,
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
            .is_some_and(|action| action.dispatch_action() == Some(Action::BoardDelete))
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
            .is_some_and(|action| action.dispatch_action() == Some(Action::PageDelete))
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
