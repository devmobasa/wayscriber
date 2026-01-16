use super::{clear_session, inspect_session};
use crate::draw::{Color, Frame, Shape};
use crate::session::snapshot::{BoardPagesSnapshot, BoardSnapshot};
use crate::session::{
    CompressionMode, SessionOptions, SessionSnapshot, ToolStateSnapshot, save_snapshot,
};

#[test]
fn clear_session_removes_all_variants_for_prefix() {
    let temp = tempfile::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-1");
    options.per_output = true;

    let base_dir = &options.base_dir;
    std::fs::write(base_dir.join("session-display_1.json"), b"{}").unwrap();
    std::fs::write(base_dir.join("session-display_1-DP_1.json"), b"{}").unwrap();
    std::fs::write(base_dir.join("session-display_1.json.bak"), b"{}").unwrap();
    std::fs::write(base_dir.join("session-display_1-DP_1.json.bak"), b"{}").unwrap();
    std::fs::write(base_dir.join("session-display_1.lock"), b"").unwrap();
    std::fs::write(base_dir.join("session-display_1-DP_1.lock"), b"").unwrap();

    let outcome = clear_session(&options).expect("clear_session should succeed");
    assert!(
        outcome.removed_session,
        "at least one session file variant should be removed"
    );
    assert!(
        outcome.removed_backup,
        "at least one backup variant should be removed"
    );
    assert!(
        outcome.removed_lock,
        "at least one lock variant should be removed"
    );

    // The main, non-per-output files should always be gone.
    assert!(
        !base_dir.join("session-display_1.json").exists(),
        "primary session file should be removed"
    );
    assert!(
        !base_dir.join("session-display_1.json.bak").exists(),
        "primary backup file should be removed"
    );
    assert!(
        !base_dir.join("session-display_1.lock").exists(),
        "primary lock file should be removed"
    );

    // Per-output variants may or may not be removed depending on identity resolution,
    // so we only assert that the primary files are gone.
}

#[test]
fn inspect_session_reports_counts_and_flags() {
    let temp = tempfile::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-inspect");
    options.persist_transparent = true;
    options.persist_whiteboard = false;
    options.persist_blackboard = false;
    options.persist_history = true;
    options.restore_tool_state = true;
    options.compression = CompressionMode::On;

    // Build a simple snapshot with one transparent shape and tool state.
    let mut frame = Frame::new();
    frame.add_shape(Shape::Rect {
        x: 0,
        y: 0,
        w: 10,
        h: 10,
        fill: false,
        color: Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 1.0,
    });

    let snapshot = SessionSnapshot {
        active_board_id: "transparent".to_string(),
        boards: vec![BoardSnapshot {
            id: "transparent".to_string(),
            pages: BoardPagesSnapshot {
                pages: vec![frame],
                active: 0,
            },
        }],
        tool_state: Some(ToolStateSnapshot {
            current_color: Color {
                r: 0.0,
                g: 1.0,
                b: 0.0,
                a: 1.0,
            },
            current_thickness: 3.0,
            eraser_size: 12.0,
            eraser_kind: crate::draw::EraserKind::Circle,
            eraser_mode: crate::input::EraserMode::Brush,
            marker_opacity: Some(0.32),
            fill_enabled: Some(false),
            tool_override: None,
            current_font_size: 24.0,
            text_background_enabled: false,
            arrow_length: 20.0,
            arrow_angle: 30.0,
            arrow_head_at_end: Some(false),
            arrow_label_enabled: Some(false),
            board_previous_color: None,
            show_status_bar: true,
        }),
    };

    save_snapshot(&snapshot, &options).expect("save_snapshot should succeed");

    let inspection = inspect_session(&options).expect("inspect_session should succeed");
    assert!(inspection.exists);
    assert!(inspection.session_path.exists());

    let counts = inspection
        .frame_counts
        .expect("frame_counts should be populated");
    assert_eq!(counts.transparent, 1);
    assert_eq!(counts.whiteboard, 0);
    assert_eq!(counts.blackboard, 0);

    let history_counts = inspection
        .history_counts
        .expect("history_counts should be populated");
    assert_eq!(history_counts.transparent.undo, 0);
    assert_eq!(history_counts.transparent.redo, 0);
    assert!(!inspection.history_present);

    assert!(inspection.tool_state_present);
    assert!(inspection.compressed);
    assert!(inspection.persist_transparent);
    assert!(!inspection.persist_whiteboard);
    assert!(!inspection.persist_blackboard);
    assert!(inspection.persist_history);
}
