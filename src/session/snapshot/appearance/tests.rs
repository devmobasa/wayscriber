use super::*;
use crate::domain::{BoardGrid, BoardGridKind};
use crate::draw::{Frame, RED};
use crate::session::{
    BoardPagesSnapshot, BoardSnapshot, SessionOptions, SessionSnapshot, load_snapshot,
    save_snapshot,
};

fn snapshot(explicit: bool) -> SessionSnapshot {
    let mut boards =
        crate::input::BoardManager::from_config(crate::config::BoardsConfig::default());
    boards.switch_to_id("whiteboard");
    let board = boards.active_board_mut();
    board.spec.grid = BoardGrid::new(BoardGridKind::IsometricDots, 20);
    board.spec.default_pen_color = None;
    board.appearance_explicit = explicit;
    SessionSnapshot {
        active_board_id: "whiteboard".into(),
        tool_state: None,
        boards: vec![BoardSnapshot {
            id: "whiteboard".into(),
            appearance: Some(BoardAppearanceSnapshot::capture(board)),
            pages: BoardPagesSnapshot {
                pages: vec![Frame::new()],
                active: 0,
            },
        }],
    }
}

#[test]
fn appearance_only_roundtrip_and_backup_recovery_preserve_exact_pen_none() {
    let temp = crate::test_temp::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().into(), "paper");
    options.persist_whiteboard = true;
    let original = snapshot(true);
    save_snapshot(&original, &options).unwrap();
    let loaded = load_snapshot(&options).unwrap().unwrap();
    assert!(loaded.has_board_data());
    let appearance = loaded.boards[0].appearance.as_ref().unwrap();
    assert!(appearance.explicit);
    assert_eq!(appearance.default_pen_color, None);
    assert_eq!(
        appearance.grid.kind,
        crate::config::BoardGridKindConfig::IsometricDots
    );
    std::fs::rename(options.session_file_path(), options.backup_file_path()).unwrap();
    assert!(load_snapshot(&options).unwrap().unwrap().has_board_data());
}

#[test]
fn appearance_history_trimming_retains_only_explicit_empty_paper() {
    use crate::session::snapshot::save::snapshot_without_history;
    for explicit in [false, true] {
        let mut original = snapshot(explicit);
        let frame = &mut original.boards[0].pages.pages[0];
        let id = frame.add_shape(crate::draw::Shape::Line {
            x1: 0,
            y1: 0,
            x2: 10,
            y2: 10,
            thick: 2.0,
            color: RED,
        });
        frame.push_undo_action(
            crate::draw::frame::UndoAction::Create {
                shapes: vec![(0, frame.shape(id).unwrap().clone())],
            },
            100,
        );
        frame.undo_last();
        assert!(original.has_board_data());
        let trimmed = snapshot_without_history(&original);
        assert_eq!(trimmed.has_board_data(), explicit);
        assert_eq!(trimmed.boards.len(), usize::from(explicit));
    }
}

#[test]
fn appearance_invalid_metadata_cannot_make_an_empty_board_recoverable() {
    let mut original = snapshot(true);
    original.boards[0].appearance.as_mut().unwrap().grid.spacing = -1;
    assert!(!original.has_board_data());
    let value = serde_json::json!({"id":"whiteboard","pages":[],"active_page":0,"appearance":{"explicit":true,"grid":{"kind":"unknown"}}});
    let board: crate::session::snapshot::types::BoardFile = serde_json::from_value(value).unwrap();
    assert!(board.appearance.is_none());
}

#[test]
fn appearance_only_named_backup_survives_a_tool_only_primary() {
    let temp = crate::test_temp::tempdir().unwrap();
    let mut options = SessionOptions::new(temp.path().into(), "paper");
    options.set_named_file_target(temp.path().join("paper.wayscriber"));
    options.persist_whiteboard = true;
    options.restore_tool_state = true;
    save_snapshot(&snapshot(true), &options).unwrap();
    let tools_only = SessionSnapshot {
        active_board_id: "whiteboard".into(),
        boards: Vec::new(),
        tool_state: Some(crate::session::ToolStateSnapshot::from_config(
            &crate::config::Config::default(),
        )),
    };
    super::super::save::save_snapshot_with_report_and_clear_boundary(&tools_only, &options, false)
        .unwrap();
    let outcome = super::super::load::load_named_session_candidate(&options).unwrap();
    assert!(outcome.has_board_data());
    assert!(matches!(
        outcome,
        super::super::load::LoadSnapshotOutcome::LoadedFromBackup(_)
    ));
}
