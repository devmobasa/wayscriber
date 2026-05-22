use super::*;
use crate::draw::{EmbeddedImage, ShapeId};
use crate::input::state::core::board_picker::BoardPickerState;
use crate::input::{BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD, BoardManager};
use crate::session::{self, BoardSnapshot, CompressionMode, SessionOptions};
use std::path::PathBuf;

fn board_index(state: &InputState, id: &str) -> usize {
    state
        .boards
        .board_states()
        .iter()
        .position(|board| board.spec.id == id)
        .expect("board index")
}

fn assert_board_text(state: &InputState, board_id: &str, shape_id: ShapeId, expected: &str) {
    let index = board_index(state, board_id);
    let shape = state.boards.board_states()[index]
        .pages
        .active_frame()
        .shape(shape_id)
        .expect("text shape");
    match &shape.shape {
        Shape::Text { text, .. } => assert_eq!(text, expected),
        _ => panic!("Expected text shape"),
    }
}

fn disable_board_auto_create(state: &mut InputState) {
    let mut config = state.boards.to_config();
    config.auto_create = false;
    state.boards = BoardManager::from_config(config);
}

#[test]
fn switch_board_force_does_not_toggle_back_to_transparent() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    assert_eq!(state.board_id(), BOARD_ID_WHITEBOARD);

    state.switch_board_force(BOARD_ID_WHITEBOARD);
    assert_eq!(state.board_id(), BOARD_ID_WHITEBOARD);
}

#[test]
fn switch_board_recent_skips_current_and_missing_entries() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    state.board_recent = vec![
        BOARD_ID_WHITEBOARD.to_string(),
        "missing".to_string(),
        "blackboard".to_string(),
    ];

    state.switch_board_recent();

    assert_eq!(state.board_id(), "blackboard");
}

#[test]
fn switch_board_recent_shows_toast_when_no_other_recent_board_exists() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    state.board_recent = vec![BOARD_ID_WHITEBOARD.to_string(), "missing".to_string()];

    state.switch_board_recent();

    assert_eq!(state.board_id(), BOARD_ID_WHITEBOARD);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("No recent board to switch to.")
    );
}

#[test]
fn switch_board_updates_open_board_picker_selection_and_clears_hover() {
    let mut state = create_test_input_state();
    state.open_board_picker();

    if let BoardPickerState::Open { hover_index, .. } = &mut state.board_picker_state {
        *hover_index = Some(0);
    }

    state.switch_board("blackboard");

    assert_eq!(state.board_id(), "blackboard");
    assert_eq!(
        state.board_picker_selected_index(),
        state.board_picker_row_for_board(state.boards.active_index())
    );
    match &state.board_picker_state {
        BoardPickerState::Open { hover_index, .. } => assert!(hover_index.is_none()),
        BoardPickerState::Hidden => panic!("board picker should remain open"),
    }
}

#[test]
fn switch_board_cancels_active_drawing_through_lifecycle_transition() {
    let mut state = create_test_input_state();
    state.state = DrawingState::Drawing {
        tool: Tool::Pen,
        start_x: 10,
        start_y: 20,
        points: vec![(10, 20), (30, 40)],
        point_thicknesses: vec![1.0, 1.0],
    };
    state.begin_pointer_drag(MouseButton::Left, None);
    state.needs_redraw = false;

    state.switch_board(BOARD_ID_WHITEBOARD);

    assert_eq!(state.board_id(), BOARD_ID_WHITEBOARD);
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.active_drag_button.is_none());
    assert!(state.needs_redraw);
}

#[test]
fn failed_switch_board_preserves_active_interaction() {
    let mut state = create_test_input_state();
    disable_board_auto_create(&mut state);
    state.state = DrawingState::Drawing {
        tool: Tool::Pen,
        start_x: 10,
        start_y: 20,
        points: vec![(10, 20), (30, 40)],
        point_thicknesses: vec![1.0, 1.0],
    };
    state.begin_pointer_drag(MouseButton::Left, None);
    state.needs_redraw = false;

    state.switch_board("missing-board");

    assert_eq!(state.board_id(), BOARD_ID_TRANSPARENT);
    assert!(matches!(state.state, DrawingState::Drawing { .. }));
    assert_eq!(state.active_drag_button, Some(MouseButton::Left));
    assert!(!state.needs_redraw);
}

#[test]
fn switch_board_cancels_text_edit_on_source_board_before_switching() {
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 40,
        y: 80,
        text: "Original".to_string(),
        color: state.current_color,
        size: state.current_font_size,
        font_descriptor: state.font_descriptor.clone(),
        background_enabled: state.text_background_enabled,
        wrap_width: None,
    });
    state.set_selection(vec![shape_id]);
    assert!(state.edit_selected_text());
    assert_board_text(&state, BOARD_ID_TRANSPARENT, shape_id, "");

    state.switch_board(BOARD_ID_WHITEBOARD);

    assert_eq!(state.board_id(), BOARD_ID_WHITEBOARD);
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.text_edit_target.is_none());
    assert_board_text(&state, BOARD_ID_TRANSPARENT, shape_id, "Original");
}

#[test]
fn switch_board_cancels_selection_move_on_source_board_before_switching() {
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 40,
        y: 80,
        w: 30,
        h: 20,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![shape_id]);
    let snapshots = state.capture_movable_selection_snapshots();
    assert!(state.apply_translation_to_selection(25, 35));
    state.state = DrawingState::MovingSelection {
        last_x: 25,
        last_y: 35,
        snapshots,
        moved: true,
    };
    state.begin_pointer_drag(MouseButton::Left, None);

    state.switch_board(BOARD_ID_WHITEBOARD);

    assert_eq!(state.board_id(), BOARD_ID_WHITEBOARD);
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.active_drag_button.is_none());

    let source_index = board_index(&state, BOARD_ID_TRANSPARENT);
    let source_shape = state.boards.board_states()[source_index]
        .pages
        .active_frame()
        .shape(shape_id)
        .expect("source shape");
    match &source_shape.shape {
        Shape::Rect { x, y, w, h, .. } => assert_eq!((*x, *y, *w, *h), (40, 80, 30, 20)),
        _ => panic!("Expected rect shape"),
    }
}

#[test]
fn duplicate_board_from_transparent_shows_info_toast_without_creating_board() {
    let mut state = create_test_input_state();
    let initial_count = state.boards.board_count();
    assert_eq!(state.board_id(), BOARD_ID_TRANSPARENT);

    state.duplicate_board();

    assert_eq!(state.boards.board_count(), initial_count);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Overlay board cannot be duplicated.")
    );
}

#[test]
fn duplicate_board_cancels_text_input_through_lifecycle_transition() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    let initial_count = state.boards.board_count();
    state.text_wrap_width = Some(240);
    state.state = DrawingState::TextInput {
        x: 10,
        y: 20,
        buffer: "draft".to_string(),
    };
    state.needs_redraw = false;

    state.duplicate_board();

    assert_eq!(state.boards.board_count(), initial_count + 1);
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.text_wrap_width.is_none());
    assert!(state.needs_redraw);
}

#[test]
fn duplicate_board_blocks_when_clone_would_exceed_persisted_session_limit() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    let before_count = state.boards.board_count();
    add_active_image_shape(&mut state, 2048);

    let options = duplicate_preflight_options_for_current_state(&state);
    state.set_session_preflight_options(Some(options));

    state.duplicate_board();

    assert_eq!(state.boards.board_count(), before_count);
    assert_eq!(state.board_id(), BOARD_ID_WHITEBOARD);
    assert!(
        state
            .ui_toast
            .as_ref()
            .is_some_and(|toast| toast.message.contains("Board duplicate blocked"))
    );
}

#[test]
fn duplicate_board_preflight_uses_collision_unique_id() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    state.duplicate_board();
    state.switch_board(BOARD_ID_WHITEBOARD);
    let before_count = state.boards.board_count();
    add_active_image_shape(&mut state, 512);

    let mut options = duplicate_preflight_options_base();
    let snapshot =
        session::snapshot_from_input(&state, &options).expect("snapshot before duplicate");
    let source_index = snapshot
        .boards
        .iter()
        .position(|board| board.id == BOARD_ID_WHITEBOARD)
        .expect("persisted source board");

    let mut old_id_snapshot = snapshot.clone();
    old_id_snapshot.boards.insert(
        source_index + 1,
        BoardSnapshot {
            id: format!("{BOARD_ID_WHITEBOARD}-copy"),
            pages: snapshot.boards[source_index].pages.clone(),
        },
    );
    let old_id_estimate = session::estimate_snapshot_save(&old_id_snapshot, &options)
        .expect("old id estimate")
        .visible_without_history
        .written_size;

    let mut runtime_id_snapshot = snapshot.clone();
    runtime_id_snapshot.boards.insert(
        source_index + 1,
        BoardSnapshot {
            id: format!("{BOARD_ID_WHITEBOARD}-copy-2"),
            pages: snapshot.boards[source_index].pages.clone(),
        },
    );
    let runtime_id_estimate = session::estimate_snapshot_save(&runtime_id_snapshot, &options)
        .expect("runtime id estimate")
        .visible_without_history
        .written_size;
    assert!(runtime_id_estimate > old_id_estimate);

    options.max_file_size_bytes = runtime_id_estimate as u64 - 1;
    assert!(old_id_estimate as u64 <= options.max_file_size_bytes);
    state.set_session_preflight_options(Some(options));

    state.duplicate_board();

    assert_eq!(state.boards.board_count(), before_count);
    assert_eq!(state.board_id(), BOARD_ID_WHITEBOARD);
    assert!(
        state
            .ui_toast
            .as_ref()
            .is_some_and(|toast| toast.message.contains("Board duplicate blocked"))
    );
}

#[test]
fn duplicate_board_cancels_text_edit_before_cloning_board() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 40,
        y: 80,
        text: "Original".to_string(),
        color: state.current_color,
        size: state.current_font_size,
        font_descriptor: state.font_descriptor.clone(),
        background_enabled: state.text_background_enabled,
        wrap_width: None,
    });
    state.set_selection(vec![shape_id]);
    assert!(state.edit_selected_text());
    assert_board_text(&state, BOARD_ID_WHITEBOARD, shape_id, "");

    state.duplicate_board();

    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.text_edit_target.is_none());
    assert_board_text(&state, BOARD_ID_WHITEBOARD, shape_id, "Original");

    let duplicated_id = state.board_id().to_string();
    assert_ne!(duplicated_id, BOARD_ID_WHITEBOARD);
    assert_board_text(&state, &duplicated_id, shape_id, "Original");
}

fn add_active_image_shape(state: &mut InputState, bytes: usize) -> ShapeId {
    state.boards.active_frame_mut().add_shape(Shape::Image {
        x: 10,
        y: 20,
        w: 120,
        h: 90,
        data: EmbeddedImage {
            mime_type: "image/png".to_string(),
            width: 240,
            height: 180,
            bytes: pseudo_random_bytes(bytes),
        },
    })
}

fn duplicate_preflight_options_for_current_state(state: &InputState) -> SessionOptions {
    let mut options = duplicate_preflight_options_base();
    let snapshot =
        session::snapshot_from_input(state, &options).expect("snapshot before duplicate");
    let estimate = session::estimate_snapshot_without_history_payload(&snapshot, &options)
        .expect("estimate before duplicate");
    options.max_file_size_bytes = estimate.written_size as u64 + 64;
    options
}

fn duplicate_preflight_options_base() -> SessionOptions {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "board-duplicate-preflight");
    options.persist_transparent = true;
    options.persist_whiteboard = true;
    options.persist_blackboard = true;
    options.persist_history = false;
    options.restore_tool_state = false;
    options.compression = CompressionMode::Off;
    options.max_file_size_bytes = u64::MAX;
    options
}

fn pseudo_random_bytes(len: usize) -> Vec<u8> {
    let mut value = 0x2468_ace0_u32;
    (0..len)
        .map(|_| {
            value ^= value << 13;
            value ^= value >> 17;
            value ^= value << 5;
            value as u8
        })
        .collect()
}

#[test]
fn create_board_adds_board_queues_config_save_and_emits_toast() {
    let mut state = create_test_input_state();
    let initial_count = state.boards.board_count();

    assert!(state.create_board());

    assert_eq!(state.boards.board_count(), initial_count + 1);
    assert!(state.take_pending_board_config().is_some());
    assert!(
        state
            .ui_toast
            .as_ref()
            .is_some_and(|toast| toast.message.starts_with("Board created:"))
    );
}
