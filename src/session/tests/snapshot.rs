use super::super::*;
use super::helpers::dummy_input_state;
use crate::draw::{Color, Shape};
use crate::input::{EraserMode, Tool};
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
    input.canvas_set.active_frame_mut().add_shape(Shape::Line {
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
    assert!(snapshot.transparent.is_some());
    assert!(snapshot.tool_state.is_some());
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
    let _ = input.set_color(desired_color);
    let _ = input.set_thickness(18.0);
    let _ = input.set_eraser_size(22.0);
    let _ = input.set_eraser_mode(EraserMode::Stroke);
    let _ = input.set_marker_opacity(0.55);
    let _ = input.set_fill_enabled(true);
    let _ = input.set_font_size(48.0);
    let _ = input.set_tool_override(Some(Tool::Rect));
    input.text_background_enabled = true;
    input.arrow_length = 40.0;
    input.arrow_angle = 45.0;
    input.arrow_head_at_end = true;
    input.board_previous_color = Some(Color {
        r: 0.9,
        g: 0.2,
        b: 0.1,
        a: 1.0,
    });
    input.show_status_bar = false;

    let snapshot = snapshot_from_input(&input, &options).expect("snapshot present");

    let mut restored = dummy_input_state();
    apply_snapshot(&mut restored, snapshot, &options);

    assert_eq!(restored.current_color, desired_color);
    assert_eq!(restored.current_thickness, 18.0);
    assert_eq!(restored.eraser_size, 22.0);
    assert_eq!(restored.eraser_mode, EraserMode::Stroke);
    assert_eq!(restored.marker_opacity, 0.55);
    assert!(restored.fill_enabled);
    assert_eq!(restored.current_font_size, 48.0);
    assert_eq!(restored.tool_override(), Some(Tool::Rect));
    assert!(restored.text_background_enabled);
    assert_eq!(restored.arrow_length, 40.0);
    assert_eq!(restored.arrow_angle, 45.0);
    assert!(restored.arrow_head_at_end);
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
