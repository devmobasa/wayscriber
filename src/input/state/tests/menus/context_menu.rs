use super::*;
use crate::draw::{BoardPages, Frame};
use crate::input::{BOARD_ID_BLACKBOARD, BOARD_ID_TRANSPARENT};

fn board_index(state: &InputState, id: &str) -> usize {
    state
        .boards
        .board_states()
        .iter()
        .position(|board| board.spec.id == id)
        .expect("board index")
}

fn set_named_pages(
    state: &mut InputState,
    board_index: usize,
    names: &[Option<&str>],
    active: usize,
) {
    let pages = names
        .iter()
        .map(|name| {
            let mut frame = Frame::new();
            if let Some(name) = name {
                frame.set_page_name(Some((*name).to_string()));
            }
            frame
        })
        .collect();
    state.boards.board_states_mut()[board_index].pages = BoardPages::from_pages(pages, active);
}

#[test]
fn context_menu_respects_enable_flag() {
    let mut state = create_test_input_state();
    state.set_context_menu_enabled(false);
    state.toggle_context_menu_via_keyboard();
    assert!(!state.is_context_menu_open());

    state.set_context_menu_enabled(true);
    state.toggle_context_menu_via_keyboard();
    assert!(state.is_context_menu_open());
}

#[test]
fn shape_menu_includes_select_this_entry_whenever_hovered() {
    let mut state = create_test_input_state();
    let first = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 10,
        w: 20,
        h: 20,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let second = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 40,
        y: 40,
        w: 20,
        h: 20,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.set_selection(vec![first, second]);
    state.open_context_menu(
        (0, 0),
        vec![first, second],
        ContextMenuKind::Shape,
        Some(first),
    );

    let entries = state.context_menu_entries();
    assert!(
        entries
            .iter()
            .any(|entry| entry.label == "Select This Shape"),
        "Expected Select This Shape entry to be present for multi-selection"
    );

    state.set_selection(vec![first]);
    state.open_context_menu((0, 0), vec![first], ContextMenuKind::Shape, Some(first));

    let entries_single = state.context_menu_entries();
    assert!(
        entries_single
            .iter()
            .any(|entry| entry.label == "Select This Shape"),
        "Expected Select This Shape entry even for single selection"
    );
}

#[test]
fn select_this_shape_command_focuses_single_shape() {
    let mut state = create_test_input_state();
    let first = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 10,
        w: 20,
        h: 20,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let second = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 40,
        y: 40,
        w: 20,
        h: 20,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.set_selection(vec![first, second]);
    state.open_context_menu(
        (10, 10),
        vec![first, second],
        ContextMenuKind::Shape,
        Some(second),
    );

    state.execute_menu_command(MenuCommand::SelectHoveredShape);
    assert_eq!(state.selected_shape_ids(), &[second]);

    assert!(
        matches!(state.context_menu_state, ContextMenuState::Hidden),
        "Context menu should close after selecting hovered shape"
    );
}

#[test]
fn properties_command_opens_panel() {
    let mut state = create_test_input_state();
    let shape_id = {
        let frame = state.boards.active_frame_mut();
        frame.add_shape(Shape::Rect {
            x: 10,
            y: 10,
            w: 40,
            h: 30,
            fill: false,
            color: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thick: 2.0,
        })
    };

    state.set_selection(vec![shape_id]);
    state.execute_menu_command(MenuCommand::Properties);
    assert!(state.properties_panel().is_some());
    assert!(!state.is_context_menu_open());
}

#[test]
fn keyboard_context_menu_sets_initial_focus() {
    let mut state = create_test_input_state();
    state.toggle_context_menu_via_keyboard();
    match &state.context_menu_state {
        ContextMenuState::Open { keyboard_focus, .. } => {
            assert!(keyboard_focus.is_some());
        }
        ContextMenuState::Hidden => panic!("Context menu should be open"),
    }
}

#[test]
fn keyboard_context_menu_focuses_edit_for_selected_text() {
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 40,
        y: 60,
        text: "Hello".to_string(),
        color: state.current_color,
        size: state.current_font_size,
        font_descriptor: state.font_descriptor.clone(),
        background_enabled: state.text_background_enabled,
        wrap_width: None,
    });

    state.set_selection(vec![shape_id]);
    state.toggle_context_menu_via_keyboard();

    let focus_index = match &state.context_menu_state {
        ContextMenuState::Open {
            keyboard_focus: Some(index),
            ..
        } => index,
        _ => panic!("Context menu should be open with focus"),
    };
    let entries = state.context_menu_entries();
    assert_eq!(entries[*focus_index].command, Some(MenuCommand::EditText));
}

#[test]
fn context_menu_help_entry_prefers_f1_shortcut_label() {
    let mut state = create_test_input_state();
    state.toggle_context_menu_via_keyboard();

    let entries = state.context_menu_entries();
    let help_entry = entries
        .iter()
        .find(|entry| entry.label == "Help")
        .expect("help entry should exist in context menu");
    assert_eq!(help_entry.shortcut.as_deref(), Some("F1"));
}

#[test]
fn context_menu_includes_radial_menu_entry() {
    let mut state = create_test_input_state();
    state.toggle_context_menu_via_keyboard();

    let entries = state.context_menu_entries();
    let radial_entry = entries
        .iter()
        .find(|entry| entry.label == "Radial Menu")
        .expect("radial menu entry should exist in context menu");
    assert_eq!(radial_entry.command, Some(MenuCommand::OpenRadialMenu));
}

#[test]
fn context_menu_radial_entry_shows_default_mouse_shortcut() {
    let mut state = create_test_input_state();
    state.toggle_context_menu_via_keyboard();

    let entries = state.context_menu_entries();
    let radial_entry = entries
        .iter()
        .find(|entry| entry.label == "Radial Menu")
        .expect("radial menu entry should exist in context menu");
    assert_eq!(radial_entry.shortcut.as_deref(), Some("Middle Click"));
}

#[test]
fn context_menu_radial_entry_shows_mouse_and_keyboard_shortcut() {
    let mut keybindings = crate::config::KeybindingsConfig::default();
    keybindings.ui.toggle_radial_menu = vec!["Ctrl+R".to_string()];
    let mut state = create_test_input_state_with_keybindings(keybindings);
    state.toggle_context_menu_via_keyboard();

    let entries = state.context_menu_entries();
    let radial_entry = entries
        .iter()
        .find(|entry| entry.label == "Radial Menu")
        .expect("radial menu entry should exist in context menu");
    assert_eq!(
        radial_entry.shortcut.as_deref(),
        Some("Middle Click / Ctrl+R")
    );
}

#[test]
fn context_menu_radial_entry_shows_right_click_shortcut_when_configured() {
    let mut state = create_test_input_state();
    state.radial_menu_mouse_binding = crate::config::RadialMenuMouseBinding::Right;
    state.toggle_context_menu_via_keyboard();

    let entries = state.context_menu_entries();
    let radial_entry = entries
        .iter()
        .find(|entry| entry.label == "Radial Menu")
        .expect("radial menu entry should exist in context menu");
    assert_eq!(radial_entry.shortcut.as_deref(), Some("Right Click"));
}

#[test]
fn context_menu_radial_entry_shows_keyboard_shortcut_when_mouse_binding_disabled() {
    let mut keybindings = crate::config::KeybindingsConfig::default();
    keybindings.ui.toggle_radial_menu = vec!["Ctrl+R".to_string()];
    let mut state = create_test_input_state_with_keybindings(keybindings);
    state.radial_menu_mouse_binding = crate::config::RadialMenuMouseBinding::Disabled;
    state.toggle_context_menu_via_keyboard();

    let entries = state.context_menu_entries();
    let radial_entry = entries
        .iter()
        .find(|entry| entry.label == "Radial Menu")
        .expect("radial menu entry should exist in context menu");
    assert_eq!(radial_entry.shortcut.as_deref(), Some("Ctrl+R"));
}

#[test]
fn context_menu_open_radial_command_opens_radial_and_closes_context_menu() {
    let mut state = create_test_input_state();
    state.toggle_context_menu_via_keyboard();
    assert!(state.is_context_menu_open());

    state.execute_menu_command(MenuCommand::OpenRadialMenu);

    assert!(state.is_radial_menu_open());
    assert!(!state.is_context_menu_open());
}

#[test]
fn canvas_menu_uses_clear_unlocked_label_when_canvas_has_locked_shapes() {
    let mut state = create_test_input_state();
    let locked = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 0,
        y: 0,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 20,
        y: 20,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let locked_index = state
        .boards
        .active_frame()
        .find_index(locked)
        .expect("locked index");
    state.boards.active_frame_mut().shapes[locked_index].locked = true;

    state.open_context_menu((0, 0), Vec::new(), ContextMenuKind::Canvas, None);

    let clear_entry = state
        .context_menu_entries()
        .into_iter()
        .find(|entry| entry.command == Some(MenuCommand::ClearAll))
        .expect("clear entry");
    assert_eq!(clear_entry.label, "Clear Unlocked");
    assert!(!clear_entry.disabled);
}

#[test]
fn page_context_menu_header_uses_page_name_and_enables_move_submenu() {
    let mut state = create_test_input_state();
    let blackboard = board_index(&state, BOARD_ID_BLACKBOARD);
    set_named_pages(&mut state, blackboard, &[None, Some("Agenda")], 1);

    state.open_page_context_menu((5, 5), blackboard, 1);

    let entries = state.context_menu_entries();
    assert_eq!(entries[0].label, "Agenda — Page 2 (2/2)");
    let move_entry = entries
        .iter()
        .find(|entry| entry.command == Some(MenuCommand::OpenPageMoveMenu))
        .expect("move entry");
    assert!(move_entry.has_submenu);
    assert!(!move_entry.disabled);
}

#[test]
fn page_move_menu_excludes_source_board_and_lists_other_boards() {
    let mut state = create_test_input_state();
    let blackboard = board_index(&state, BOARD_ID_BLACKBOARD);
    state.open_page_context_menu((5, 5), blackboard, 0);

    state.execute_menu_command(MenuCommand::OpenPageMoveMenu);

    let entries = state.context_menu_entries();
    assert!(entries.iter().any(|entry| entry.label == "Overlay"));
    assert!(entries.iter().any(|entry| entry.label == "Whiteboard"));
    assert!(!entries.iter().any(|entry| entry.label == "Blackboard"));
}

#[test]
fn pages_menu_shows_window_indicators_around_active_page() {
    let mut state = create_test_input_state();
    let blackboard = board_index(&state, BOARD_ID_BLACKBOARD);
    let pages = (0..10)
        .map(|index| {
            let mut frame = Frame::new();
            frame.set_page_name(Some(format!("Page {index}")));
            frame
        })
        .collect();
    state.boards.board_states_mut()[blackboard].pages = BoardPages::from_pages(pages, 5);
    state.switch_board(BOARD_ID_BLACKBOARD);
    state.open_context_menu((0, 0), Vec::new(), ContextMenuKind::Pages, None);

    let entries = state.context_menu_entries();
    assert!(entries.iter().any(|entry| entry.label == "  ... 1 above"));
    assert!(entries.iter().any(|entry| entry.label == "  ... 1 below"));
    assert!(
        entries
            .iter()
            .any(|entry| entry.label == "  Page 6 (current)" && entry.disabled)
    );
}

#[test]
fn boards_menu_disables_delete_for_transparent_board_and_shows_overflow_entry() {
    let mut state = create_test_input_state();
    state.switch_board_slot(8);
    state.switch_board_slot(5);

    state.open_context_menu((0, 0), Vec::new(), ContextMenuKind::Boards, None);
    let overflow_entries = state
        .context_menu_entries()
        .into_iter()
        .filter(|entry| entry.command == Some(MenuCommand::OpenBoardPicker))
        .collect::<Vec<_>>();
    assert_eq!(overflow_entries.len(), 1);
    assert!(overflow_entries[0].label.contains("open picker"));

    state.switch_board(BOARD_ID_TRANSPARENT);
    state.open_context_menu((0, 0), Vec::new(), ContextMenuKind::Boards, None);
    let delete_entry = state
        .context_menu_entries()
        .into_iter()
        .find(|entry| entry.command == Some(MenuCommand::BoardDelete))
        .expect("delete board entry");
    assert!(delete_entry.disabled);
}

#[test]
fn open_pages_menu_command_switches_to_pages_submenu_with_actionable_focus() {
    let mut state = create_test_input_state();
    state.open_context_menu((12, 34), Vec::new(), ContextMenuKind::Canvas, None);

    state.execute_menu_command(MenuCommand::OpenPagesMenu);

    let focus_index = match &state.context_menu_state {
        ContextMenuState::Open {
            kind: ContextMenuKind::Pages,
            keyboard_focus,
            ..
        } => keyboard_focus.expect("pages submenu focus"),
        _ => panic!("expected pages submenu to be open"),
    };
    let entries = state.context_menu_entries();
    assert!(!entries[focus_index].disabled);
    assert!(entries[focus_index].command.is_some());
}

#[test]
fn open_boards_menu_command_switches_to_boards_submenu_with_actionable_focus() {
    let mut state = create_test_input_state();
    state.open_context_menu((12, 34), Vec::new(), ContextMenuKind::Canvas, None);

    state.execute_menu_command(MenuCommand::OpenBoardsMenu);

    let focus_index = match &state.context_menu_state {
        ContextMenuState::Open {
            kind: ContextMenuKind::Boards,
            keyboard_focus,
            ..
        } => keyboard_focus.expect("boards submenu focus"),
        _ => panic!("expected boards submenu to be open"),
    };
    let entries = state.context_menu_entries();
    assert!(!entries[focus_index].disabled);
    assert!(entries[focus_index].command.is_some());
}

#[test]
fn page_duplicate_from_context_duplicates_target_page_and_closes_menu() {
    let mut state = create_test_input_state();
    let blackboard = board_index(&state, BOARD_ID_BLACKBOARD);
    set_named_pages(&mut state, blackboard, &[Some("Only page")], 0);
    state.open_page_context_menu((5, 5), blackboard, 0);

    state.execute_menu_command(MenuCommand::PageDuplicateFromContext);

    assert_eq!(
        state.boards.board_states()[blackboard].pages.page_count(),
        2
    );
    assert!(!state.is_context_menu_open());
}

#[test]
fn page_move_to_board_command_moves_page_switches_board_and_closes_menu() {
    let mut state = create_test_input_state();
    let blackboard = board_index(&state, BOARD_ID_BLACKBOARD);
    let whiteboard = board_index(&state, "whiteboard");
    set_named_pages(&mut state, blackboard, &[Some("Keep"), Some("Move me")], 1);
    set_named_pages(&mut state, whiteboard, &[Some("Target")], 0);
    state.open_page_context_menu((5, 5), blackboard, 1);

    state.execute_menu_command(MenuCommand::PageMoveToBoard {
        id: "whiteboard".to_string(),
    });

    assert_eq!(state.board_id(), "whiteboard");
    assert_eq!(
        state.boards.board_states()[blackboard].pages.page_count(),
        1
    );
    assert_eq!(
        state.boards.board_states()[whiteboard].pages.page_count(),
        2
    );
    assert_eq!(
        state.boards.board_states()[whiteboard].pages.page_name(1),
        Some("Move me")
    );
    assert!(!state.is_context_menu_open());
}

#[test]
fn open_board_picker_command_closes_context_menu_and_opens_picker() {
    let mut state = create_test_input_state();
    state.open_context_menu((0, 0), Vec::new(), ContextMenuKind::Canvas, None);
    assert!(state.is_context_menu_open());

    state.execute_menu_command(MenuCommand::OpenBoardPicker);

    assert!(!state.is_context_menu_open());
    assert!(state.is_board_picker_open());
}
