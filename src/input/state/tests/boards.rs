use super::*;
use crate::draw::{EmbeddedImage, ShapeId};
use crate::input::state::core::board_picker::BoardPickerState;
use crate::input::{BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD, BoardManager};
use crate::session::{CompressionMode, SessionOptions};
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

    if let BoardPickerState::Open { hover_index, .. } = &mut state.board_picker.state {
        *hover_index = Some(0);
    }

    state.switch_board("blackboard");

    assert_eq!(state.board_id(), "blackboard");
    assert_eq!(
        state.board_picker_selected_index(),
        state.board_picker_row_for_board(state.boards.active_index())
    );
    match &state.board_picker.state {
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
    assert!(!state.pointer_drag_active());
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
    assert!(state.pointer_drag_button_matches(MouseButton::Left));
    assert!(!state.needs_redraw);
}

#[test]
fn switch_board_cancels_text_edit_on_source_board_before_switching() {
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 40,
        y: 80,
        text: "Original".to_string(),
        color: state.style.current_color,
        size: state.style.current_font_size,
        font_descriptor: state.style.font_descriptor.clone(),
        background_enabled: state.style.text_background_enabled,
        wrap_width: None,
    });
    state.set_selection(vec![shape_id]);
    assert!(state.edit_selected_text());
    assert_board_text(&state, BOARD_ID_TRANSPARENT, shape_id, "");

    state.switch_board(BOARD_ID_WHITEBOARD);

    assert_eq!(state.board_id(), BOARD_ID_WHITEBOARD);
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.text_editing.edit_target().is_none());
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
        color: state.style.current_color,
        thick: state.style.current_thickness,
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
    assert!(!state.pointer_drag_active());

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
    state.style.text_wrap_width = Some(240);
    state.state = DrawingState::text_input(10, 20, "draft".to_string());
    state.needs_redraw = false;

    state.duplicate_board();

    assert_eq!(state.boards.board_count(), initial_count + 1);
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.style.text_wrap_width.is_none());
    assert!(state.needs_redraw);
}

#[test]
fn duplicate_board_blocks_when_clone_would_exceed_persisted_session_limit() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    let before_count = state.boards.board_count();
    add_active_image_shape(&mut state, 2048);

    let mut options = duplicate_preflight_options_base();
    options.max_file_size_bytes = 1024;
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
fn duplicate_board_skips_session_preflight_for_single_empty_page_board() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    let before_count = state.boards.board_count();

    let mut options = duplicate_preflight_options_base();
    options.max_file_size_bytes = 1;
    state.set_session_preflight_options(Some(options));

    state.duplicate_board();

    assert_eq!(state.boards.board_count(), before_count + 1);
    assert_ne!(state.board_id(), BOARD_ID_WHITEBOARD);
    assert!(
        state
            .ui_toast
            .as_ref()
            .is_some_and(|toast| toast.message.contains("Board duplicated"))
    );
}

#[test]
fn duplicate_board_ignores_history_only_page_when_history_persistence_disabled() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    let before_count = state.boards.board_count();
    make_active_page_history_only(&mut state);

    let mut options = duplicate_preflight_options_base();
    options.persist_history = false;
    options.max_file_size_bytes = 1;
    state.set_session_preflight_options(Some(options));

    state.duplicate_board();

    assert_eq!(state.boards.board_count(), before_count + 1);
    assert_ne!(state.board_id(), BOARD_ID_WHITEBOARD);
    assert!(
        state
            .ui_toast
            .as_ref()
            .is_some_and(|toast| toast.message.contains("Board duplicated"))
    );
}

#[test]
fn duplicate_board_ignores_history_only_page_for_visible_save_preflight() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    let before_count = state.boards.board_count();
    make_active_page_history_only(&mut state);

    let mut options = duplicate_preflight_options_base();
    options.persist_history = true;
    options.max_file_size_bytes = 1;
    state.set_session_preflight_options(Some(options));

    state.duplicate_board();

    assert_eq!(state.boards.board_count(), before_count + 1);
    assert_ne!(state.board_id(), BOARD_ID_WHITEBOARD);
    assert!(
        state
            .ui_toast
            .as_ref()
            .is_some_and(|toast| toast.message.contains("Board duplicated"))
    );
}

#[test]
fn duplicate_board_preflight_handles_existing_copy_board_when_over_image_limit() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    state.duplicate_board();
    state.switch_board(BOARD_ID_WHITEBOARD);
    let before_count = state.boards.board_count();
    add_active_image_shape(&mut state, 2048);

    let mut options = duplicate_preflight_options_base();
    options.max_file_size_bytes = 1024;
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
        color: state.style.current_color,
        size: state.style.current_font_size,
        font_descriptor: state.style.font_descriptor.clone(),
        background_enabled: state.style.text_background_enabled,
        wrap_width: None,
    });
    state.set_selection(vec![shape_id]);
    assert!(state.edit_selected_text());
    assert_board_text(&state, BOARD_ID_WHITEBOARD, shape_id, "");

    state.duplicate_board();

    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.text_editing.edit_target().is_none());
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
            bytes: pseudo_random_bytes(bytes).into(),
        },
    })
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

fn make_active_page_history_only(state: &mut InputState) {
    let frame = state.boards.active_frame_mut();
    let id = frame.add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 20,
        y2: 20,
        color: Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 2.0,
    });
    let index = frame.find_index(id).expect("shape index");
    let shape = frame.shape(id).expect("shape").clone();
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(index, shape)],
        },
        100,
    );
    frame.undo_last();
    assert!(frame.shapes.is_empty());
    assert_eq!(frame.redo_stack_len(), 1);
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

/// Creating a board is session work: the live set grows and the user is told,
/// and nothing reaches `config.toml` — the board templates are the
/// configurator's.
#[test]
fn create_board_adds_board_and_emits_toast() {
    let mut state = create_test_input_state();
    let initial_count = state.boards.board_count();

    assert!(state.create_board());

    assert_eq!(state.boards.board_count(), initial_count + 1);
    assert!(
        state
            .ui_toast
            .as_ref()
            .is_some_and(|toast| toast.message.starts_with("Board created:"))
    );
}

/// The live board set is session state now: creating, renaming, recoloring, and
/// reordering boards touch the running app and nothing else. `config.toml`
/// holds the templates a new session is seeded from, and a board gesture is not
/// one of the explicit user edit actions that may write it — the configurator's
/// Save, and the overlay's shortcut, preset, and quick-color edits — so the file
/// keeps its bytes, its metadata, and its neighbours through every one of them.
#[test]
fn live_board_edits_leave_the_config_file_untouched() {
    crate::config::test_helpers::with_temp_config_home(|config_root| {
        let config_dir = config_root.join(crate::config::PRIMARY_CONFIG_DIR);
        std::fs::create_dir_all(&config_dir).expect("test config directory");
        let path = config_dir.join("config.toml");
        // `persist_customizations` is still parsed, and it no longer buys the
        // overlay permission to write anything.
        std::fs::write(&path, "[boards]\npersist_customizations = true\n")
            .expect("test config should be written");
        let snapshot = crate::config::test_helpers::ConfigFileSnapshot::capture(&path);

        let configured = crate::config::Config::load()
            .expect("test config should load")
            .config;
        let mut state = create_test_input_state();
        state.boards = BoardManager::from_config(configured.resolved_boards());
        let whiteboard = board_index(&state, BOARD_ID_WHITEBOARD);

        assert!(state.set_board_name(whiteboard, "Renamed this run".to_string()));
        assert!(state.set_board_background_color(
            whiteboard,
            Color {
                r: 0.2,
                g: 0.3,
                b: 0.4,
                a: 1.0,
            },
        ));
        assert!(state.create_board());
        assert!(state.reorder_board(0, 1));

        snapshot.assert_unchanged("renaming, recoloring, creating, and reordering boards");

        // Restart semantics: the next process seeds from the authored template,
        // not from the boards this run edited.
        let restarted = crate::config::Config::load()
            .expect("test config should reload")
            .config;
        assert_eq!(
            restarted
                .resolved_boards()
                .items
                .iter()
                .find(|item| item.id == BOARD_ID_WHITEBOARD)
                .map(|item| item.name.clone()),
            configured
                .resolved_boards()
                .items
                .iter()
                .find(|item| item.id == BOARD_ID_WHITEBOARD)
                .map(|item| item.name.clone()),
            "a fresh load seeds from the authored template, not from this run"
        );
    });
}
