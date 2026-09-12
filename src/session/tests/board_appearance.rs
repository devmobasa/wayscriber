use super::helpers::dummy_input_state;
use crate::domain::{BoardGrid, BoardGridKind};
use crate::draw::{BLACK, RED};
use crate::input::boards::{BoardAppearance, BoardPenOrigin};
use crate::session::{SessionOptions, apply_snapshot, snapshot_from_input};
use std::path::PathBuf;

fn options() -> SessionOptions {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "appearance");
    options.persist_whiteboard = true;
    options.restore_tool_state = false;
    options
}

#[test]
fn board_appearance_empty_templates_do_not_displace_recovery_but_explicit_paper_does() {
    let mut input = dummy_input_state();
    let options = options();
    assert!(snapshot_from_input(&input, &options).is_none());
    input.switch_board_force("whiteboard");
    let board = input.boards.active_board_mut();
    board.spec.grid = BoardGrid::new(BoardGridKind::Isometric, 20);
    board.appearance_explicit = true;
    let snapshot = snapshot_from_input(&input, &options).unwrap();
    assert!(snapshot.has_board_data());
    assert_eq!(snapshot.boards.len(), 1);
    assert!(snapshot.boards[0].appearance.as_ref().unwrap().explicit);
    assert!(input.boards.active_board().appearance_explicit);
    let mut restored = dummy_input_state();
    apply_snapshot(&mut restored, snapshot, &options);
    assert_eq!(
        restored.boards.active_board().spec.grid,
        BoardGrid::new(BoardGridKind::Isometric, 20)
    );
    assert!(restored.boards.active_board().appearance_explicit);
}

#[test]
fn board_appearance_same_id_restore_resolves_pen_then_tool_state_wins() {
    let mut source = dummy_input_state();
    source.switch_board_force("whiteboard");
    let board = source.boards.active_board_mut();
    board.spec.default_pen_color = Some(RED);
    board.pen_origin = BoardPenOrigin::RuntimeContrast;
    board.appearance_explicit = true;
    let options = options();
    let snapshot = snapshot_from_input(&source, &options).unwrap();
    let mut target = dummy_input_state();
    target.switch_board_force("whiteboard");
    apply_snapshot(&mut target, snapshot, &options);
    assert_eq!(target.color_for_tool(crate::input::Tool::Pen), RED);
    assert_eq!(
        target.boards.active_board().pen_origin,
        BoardPenOrigin::RuntimeContrast
    );

    let mut options = options;
    options.restore_tool_state = true;
    let mut snapshot = snapshot_from_input(&source, &options).unwrap();
    let tools = snapshot.tool_state.as_mut().unwrap();
    tools.current_color = BLACK;
    tools.tool_settings = None;
    tools.board_previous_color = Some(RED);
    apply_snapshot(&mut target, snapshot, &options);
    assert_eq!(target.color_for_tool(crate::input::Tool::Pen), BLACK);
    assert_eq!(target.board_previous_color(), Some(RED));
}

#[test]
fn board_appearance_legacy_restore_uses_immutable_seed() {
    let mut input = dummy_input_state();
    input.switch_board_force("whiteboard");
    let seed = BoardAppearance::from_spec(&input.boards.active_board().spec);
    input.set_board_background_color(input.boards.active_index(), RED);
    let options = options();
    let mut snapshot = snapshot_from_input(&input, &options).unwrap();
    snapshot.boards[0].appearance = None;
    apply_snapshot(&mut input, snapshot, &options);
    assert_eq!(
        BoardAppearance::from_spec(&input.boards.active_board().spec),
        seed
    );
    assert!(!input.boards.active_board().appearance_explicit);
}

#[test]
fn board_appearance_named_replacement_clears_absent_overrides_and_failed_replace_keeps_draft() {
    let mut input = dummy_input_state();
    input.switch_board_force("whiteboard");
    let seed = BoardAppearance::from_spec(&input.boards.active_board().spec);
    input.set_board_background_color(input.boards.active_index(), RED);
    input.open_board_picker_with_measurer(&crate::draw::TextMeasurer::default());
    input.board_picker_edit_color_selected_with_measurer(&crate::draw::TextMeasurer::default());
    let empty = crate::session::SessionSnapshot {
        active_board_id: "whiteboard".into(),
        boards: Vec::new(),
        tool_state: None,
    };
    let mut too_many = empty.clone();
    for index in 0..input.boards.max_count() + 1 {
        too_many.boards.push(crate::session::BoardSnapshot {
            id: format!("overflow-{index}"),
            appearance: None,
            pages: crate::session::BoardPagesSnapshot {
                pages: vec![crate::draw::Frame::new()],
                active: 0,
            },
        });
    }
    assert!(
        crate::session::apply_snapshot_replacing_boards(
            &mut input,
            &crate::draw::TextMeasurer::default(),
            too_many,
            &options()
        )
        .is_err()
    );
    assert!(input.board_appearance_edit().is_some());
    crate::session::apply_snapshot_replacing_boards(
        &mut input,
        &crate::draw::TextMeasurer::default(),
        empty,
        &options(),
    )
    .unwrap();
    assert!(input.board_appearance_edit().is_none());
    assert_eq!(
        BoardAppearance::from_spec(&input.boards.active_board().spec),
        seed
    );
    assert!(!input.boards.active_board().appearance_explicit);
}

#[test]
fn board_appearance_with_drawings_is_frozen_without_promoting_override_ownership() {
    let mut source = dummy_input_state();
    source.switch_board_force("whiteboard");
    source.boards.active_board_mut().spec.grid = BoardGrid::new(BoardGridKind::Cartesian, 20);
    source
        .boards
        .active_frame_mut()
        .set_page_name(Some("Drawing reference".into()));
    let snapshot = snapshot_from_input(&source, &options()).unwrap();
    assert!(!snapshot.boards[0].appearance.as_ref().unwrap().explicit);
    let mut target = dummy_input_state();
    target.switch_board_force("whiteboard");
    target.boards.active_board_mut().spec.grid = BoardGrid::new(BoardGridKind::Isometric, 80);
    apply_snapshot(&mut target, snapshot, &options());
    assert_eq!(
        target.boards.active_board().spec.grid,
        BoardGrid::new(BoardGridKind::Cartesian, 20)
    );
    assert!(!target.boards.active_board().appearance_explicit);
}
