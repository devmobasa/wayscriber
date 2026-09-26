use super::*;
use crate::draw::{BoardPages, Frame};
use crate::input::state::core::board_picker::{BoardPickerFocus, BoardPickerPageNavMode};
use crate::input::{BOARD_ID_BLACKBOARD, BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD};

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
        color: state.style.current_color,
        thick: state.style.current_thickness,
    });
    let second = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 40,
        y: 40,
        w: 20,
        h: 20,
        fill: false,
        color: state.style.current_color,
        thick: state.style.current_thickness,
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
fn shape_menu_includes_reset_canvas_position_on_solid_boards() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_BLACKBOARD);
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 10,
        w: 20,
        h: 20,
        fill: false,
        color: state.style.current_color,
        thick: state.style.current_thickness,
    });

    state.set_selection(vec![shape_id]);
    state.open_context_menu(
        (0, 0),
        vec![shape_id],
        ContextMenuKind::Shape,
        Some(shape_id),
    );

    let entries = state.context_menu_entries();
    let entry = entries
        .iter()
        .find(|entry| entry.label == "Reset Canvas Position")
        .expect("reset canvas position entry should exist on solid-board shape menus");
    assert_eq!(entry.shortcut.as_deref(), Some("Space+Drag"));
    assert!(entry.disabled);
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
        color: state.style.current_color,
        thick: state.style.current_thickness,
    });
    let second = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 40,
        y: 40,
        w: 20,
        h: 20,
        fill: false,
        color: state.style.current_color,
        thick: state.style.current_thickness,
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
        matches!(state.context_menu.state, ContextMenuState::Hidden),
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
    match &state.context_menu.state {
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
        color: state.style.current_color,
        size: state.style.current_font_size,
        font_descriptor: state.style.font_descriptor.clone(),
        background_enabled: state.style.text_background_enabled,
        wrap_width: None,
    });

    state.set_selection(vec![shape_id]);
    state.toggle_context_menu_via_keyboard();

    let focus_index = match &state.context_menu.state {
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
fn context_menu_includes_zoom_submenu_entry() {
    let mut state = create_test_input_state();
    state.toggle_context_menu_via_keyboard();

    let zoom_entry = state
        .context_menu_entries()
        .into_iter()
        .find(|entry| entry.label == "Zoom")
        .expect("zoom submenu entry should exist in context menu");
    assert_eq!(zoom_entry.submenu, Some(ContextMenuKind::Zoom));
    assert_eq!(zoom_entry.shortcut.as_deref(), Some("100%"));
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
    state.radial_menu.mouse_binding = crate::config::RadialMenuMouseBinding::Right;
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
    state.radial_menu.mouse_binding = crate::config::RadialMenuMouseBinding::Disabled;
    state.toggle_context_menu_via_keyboard();

    let entries = state.context_menu_entries();
    let radial_entry = entries
        .iter()
        .find(|entry| entry.label == "Radial Menu")
        .expect("radial menu entry should exist in context menu");
    assert_eq!(radial_entry.shortcut.as_deref(), Some("Ctrl+R"));
}

/// The canvas menu stays open under the submenu, whose keyboard focus sits on
/// an entry it can run.
fn assert_submenu_has_actionable_focus(state: &InputState, kind: ContextMenuKind) {
    assert!(matches!(
        state.context_menu.state,
        ContextMenuState::Open {
            kind: ContextMenuKind::Canvas,
            ..
        }
    ));
    let submenu = state.context_submenu().expect("submenu open");
    assert_eq!(submenu.kind, kind);
    let focus = submenu.keyboard_focus.expect("submenu focus");
    let entries = state.context_submenu_entries();
    assert!(!entries[focus].disabled);
    assert!(entries[focus].command.is_some());
}

#[test]
fn open_zoom_menu_command_opens_zoom_submenu_with_actionable_focus() {
    let mut state = create_test_input_state();
    state.open_context_menu((12, 34), Vec::new(), ContextMenuKind::Canvas, None);

    state.execute_menu_command(MenuCommand::OpenZoomMenu);

    assert_submenu_has_actionable_focus(&state, ContextMenuKind::Zoom);
}

#[test]
fn zoom_menu_disables_out_and_reset_when_zoom_is_inactive() {
    let mut state = create_test_input_state();
    state.open_context_menu((12, 34), Vec::new(), ContextMenuKind::Zoom, None);

    let entries = state.context_menu_entries();
    let zoom_out = entries
        .iter()
        .find(|entry| entry.command == Some(MenuCommand::ZoomOut))
        .expect("zoom out entry");
    let reset_zoom = entries
        .iter()
        .find(|entry| entry.command == Some(MenuCommand::ResetZoom))
        .expect("reset zoom entry");

    assert!(zoom_out.disabled);
    assert!(reset_zoom.disabled);
}

#[test]
fn zoom_in_command_queues_zoom_action_and_closes_menu() {
    let mut state = create_test_input_state();
    state.open_context_menu((12, 34), Vec::new(), ContextMenuKind::Zoom, None);
    assert!(!state.pending_onboarding_usage.used_zoom_control);

    state.execute_menu_command(MenuCommand::ZoomIn);

    assert_eq!(state.take_pending_zoom_action(), Some(ZoomAction::In));
    assert!(state.pending_onboarding_usage.used_zoom_control);
    assert!(!state.is_context_menu_open());
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
        color: state.style.current_color,
        thick: state.style.current_thickness,
    });
    state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 20,
        y: 20,
        w: 10,
        h: 10,
        fill: false,
        color: state.style.current_color,
        thick: state.style.current_thickness,
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
        .find(|entry| entry.submenu == Some(ContextMenuKind::PageMove))
        .expect("move entry");
    assert!(!move_entry.disabled);
}

#[test]
fn page_move_submenu_excludes_source_board_and_moves_the_menu_page() {
    let mut state = create_test_input_state();
    let blackboard = board_index(&state, BOARD_ID_BLACKBOARD);
    let whiteboard = board_index(&state, BOARD_ID_WHITEBOARD);
    set_named_pages(&mut state, blackboard, &[Some("Keep"), Some("Move me")], 1);
    state.open_page_context_menu((5, 5), blackboard, 1);

    state.execute_menu_command(MenuCommand::OpenPageMoveMenu);

    assert!(matches!(
        state.context_menu.state,
        ContextMenuState::Open {
            kind: ContextMenuKind::Page,
            ..
        }
    ));
    let entries = state.context_submenu_entries();
    assert!(entries.iter().any(|entry| entry.label == "Overlay"));
    assert!(entries.iter().any(|entry| entry.label == "Whiteboard"));
    assert!(!entries.iter().any(|entry| entry.label == "Blackboard"));

    // The submenu keeps the page its parent menu was opened for.
    let pages = state.boards.board_states()[whiteboard].pages.page_count();
    state.execute_menu_command(MenuCommand::PageMoveToBoard {
        id: BOARD_ID_WHITEBOARD.to_string(),
    });
    assert_eq!(
        state.boards.board_states()[whiteboard].pages.page_count(),
        pages + 1
    );
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
    // Overflow rows lead to the board picker's page panel.
    assert!(entries.iter().any(|entry| {
        entry.label == "  ... 1 above (open picker)"
            && entry.command == Some(MenuCommand::OpenBoardPicker)
            && !entry.disabled
    }));
    assert!(
        entries
            .iter()
            .any(|entry| entry.label == "  ... 1 below (open picker)")
    );
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

fn menu_entry(state: &InputState, command: MenuCommand) -> ContextMenuEntry {
    state
        .context_menu_entries()
        .into_iter()
        .find(|entry| entry.command == Some(command.clone()))
        .unwrap_or_else(|| panic!("missing {command:?} entry"))
}

#[test]
fn boards_menu_edits_active_board_paper_and_skips_the_overlay() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);

    state.open_context_menu((0, 0), Vec::new(), ContextMenuKind::Boards, None);
    assert!(!menu_entry(&state, MenuCommand::BoardEditPaper).disabled);
    state.execute_menu_command(MenuCommand::BoardEditPaper);

    assert!(!state.is_context_menu_open());
    assert!(state.is_board_picker_open());
    assert_eq!(
        state.board_appearance_edit().map(|edit| edit.board_id()),
        Some(BOARD_ID_WHITEBOARD)
    );

    state.close_board_picker();
    state.switch_board(BOARD_ID_TRANSPARENT);
    state.open_context_menu((0, 0), Vec::new(), ContextMenuKind::Boards, None);
    assert!(menu_entry(&state, MenuCommand::BoardEditPaper).disabled);
}

#[test]
fn board_row_menu_edits_renames_and_pins_its_own_board() {
    let mut state = create_test_input_state();
    let blackboard = board_index(&state, BOARD_ID_BLACKBOARD);
    let overlay = board_index(&state, BOARD_ID_TRANSPARENT);
    state.open_board_picker_with_measurer(&crate::draw::TextMeasurer::default());

    state.open_board_context_menu((5, 5), overlay);
    assert_eq!(state.context_menu_entries()[0].label, "Overlay");
    assert!(menu_entry(&state, MenuCommand::BoardEditPaperFromContext).disabled);

    state.open_board_context_menu((5, 5), blackboard);
    assert!(
        state.is_board_picker_open(),
        "row menus keep the picker open"
    );
    assert_eq!(state.context_menu_entries()[0].label, "Blackboard");
    state.execute_menu_command(MenuCommand::BoardEditPaperFromContext);
    assert!(!state.is_context_menu_open());
    assert_eq!(
        state.board_appearance_edit().map(|edit| edit.board_id()),
        Some(BOARD_ID_BLACKBOARD)
    );

    state.board_picker_cancel_edit();
    state.open_board_context_menu((5, 5), blackboard);
    state.execute_menu_command(MenuCommand::BoardRenameFromContext);
    let row = state.board_picker_row_for_board(blackboard).unwrap();
    assert_eq!(
        state
            .board_picker_edit_state()
            .map(|(mode, index, _)| (mode, index)),
        Some((crate::input::state::BoardPickerEditMode::Name, row))
    );

    state.board_picker_cancel_edit();
    let _ = state.take_pending_board_runtime_ui_actions();
    state.open_board_context_menu((5, 5), blackboard);
    state.execute_menu_command(MenuCommand::BoardTogglePinFromContext);
    assert!(matches!(
        state.take_pending_board_runtime_ui_actions().as_slice(),
        [crate::input::boards::PendingBoardRuntimeUiAction::TogglePin { board_id, .. }]
            if board_id == BOARD_ID_BLACKBOARD
    ));
}

#[test]
fn right_clicking_a_board_row_selects_it_and_opens_its_menu() {
    let mut state = create_test_input_state();
    let blackboard = board_index(&state, BOARD_ID_BLACKBOARD);
    state.open_board_picker_with_measurer(&crate::draw::TextMeasurer::default());
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1280, 720).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    state.update_board_picker_layout(&ctx, 1280, 720);
    let layout = *state.board_picker_layout().unwrap();
    let row = state.board_picker_row_for_board(blackboard).unwrap();
    let x = (layout.origin_x + layout.padding_x + 60.0) as i32;
    let y = (layout.origin_y
        + layout.padding_y
        + layout.header_height
        + layout.row_height * (row as f64 + 0.5)) as i32;

    assert!(state.handle_board_picker_press(crate::input::MouseButton::Right, x, y));

    assert!(state.is_board_picker_open());
    assert_eq!(state.board_picker_selected_index(), Some(row));
    assert!(matches!(
        state.context_menu.state,
        ContextMenuState::Open {
            kind: ContextMenuKind::Board,
            ..
        }
    ));
    assert_eq!(state.context_menu_entries()[0].label, "Blackboard");
}

#[test]
fn open_pages_menu_command_opens_pages_submenu_with_actionable_focus() {
    let mut state = create_test_input_state();
    state.open_context_menu((12, 34), Vec::new(), ContextMenuKind::Canvas, None);

    state.execute_menu_command(MenuCommand::OpenPagesMenu);

    assert_submenu_has_actionable_focus(&state, ContextMenuKind::Pages);
}

#[test]
fn open_boards_menu_command_opens_boards_submenu_with_actionable_focus() {
    let mut state = create_test_input_state();
    state.open_context_menu((12, 34), Vec::new(), ContextMenuKind::Canvas, None);

    state.execute_menu_command(MenuCommand::OpenBoardsMenu);

    assert_submenu_has_actionable_focus(&state, ContextMenuKind::Boards);
}

#[test]
fn keyboard_shape_menu_anchor_tracks_panned_board_view_offset() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    assert!(state.boards.active_frame_mut().set_view_offset(100, 50));
    state.update_pointer_position(400, 300);
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 140,
        y: 90,
        w: 20,
        h: 20,
        fill: false,
        color: state.style.current_color,
        thick: state.style.current_thickness,
    });
    state.set_selection(vec![shape_id]);

    state.toggle_context_menu_via_keyboard();

    match state.context_menu.state {
        ContextMenuState::Open {
            kind: ContextMenuKind::Shape,
            anchor,
            ..
        } => assert_eq!(anchor, (50, 50)),
        _ => panic!("expected keyboard shape context menu"),
    }
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
fn page_delete_from_context_reconciles_board_picker_page_search_cursor() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let mut state = create_test_input_state();
    let blackboard = board_index(&state, BOARD_ID_BLACKBOARD);
    state.switch_board(BOARD_ID_BLACKBOARD);
    set_named_pages(
        &mut state,
        blackboard,
        &[Some("Match one"), Some("Match two"), Some("Other")],
        0,
    );
    state.open_board_picker_with_measurer(&crate::draw::TextMeasurer::default());
    state.board_picker_set_focus(BoardPickerFocus::PagePanel);

    state.handle_board_picker_key_with_measurer(&route_measurer, Key::Char('/'));
    for ch in "match".chars() {
        state.handle_board_picker_key_with_measurer(&route_measurer, Key::Char(ch));
    }
    state.handle_board_picker_key_with_measurer(&route_measurer, Key::F3);
    assert_eq!(
        state.board_picker_page_nav_mode(),
        BoardPickerPageNavMode::Search
    );
    assert_eq!(state.board_picker_page_search_cursor(), Some(1));
    assert_eq!(state.board_picker_page_search_active_match(), Some(1));

    state.open_page_context_menu((5, 5), blackboard, 0);
    state.execute_menu_command(MenuCommand::PageDeleteFromContext);
    assert_eq!(state.board_picker_page_search_cursor(), Some(1));
    assert_eq!(
        state.boards.board_states()[blackboard].pages.page_count(),
        3
    );

    state.open_page_context_menu((5, 5), blackboard, 0);
    state.execute_menu_command(MenuCommand::PageDeleteFromContext);

    assert_eq!(
        state.boards.board_states()[blackboard].pages.page_count(),
        2
    );
    assert_eq!(state.board_picker_page_search_match_count(), 1);
    assert_eq!(state.board_picker_page_search_cursor(), Some(0));
    assert_eq!(state.board_picker_page_search_active_match(), Some(0));

    assert!(state.handle_board_picker_key_with_measurer(&route_measurer, Key::Return));
    assert!(!state.is_board_picker_open());
    assert_eq!(
        state.boards.board_states()[blackboard].pages.active_index(),
        0
    );
}

#[test]
fn page_search_active_match_clamps_stale_cursor_after_external_page_delete() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let mut state = create_test_input_state();
    let blackboard = board_index(&state, BOARD_ID_BLACKBOARD);
    state.switch_board(BOARD_ID_BLACKBOARD);
    set_named_pages(
        &mut state,
        blackboard,
        &[Some("Match one"), Some("Match two"), Some("Other")],
        0,
    );
    state.open_board_picker_with_measurer(&crate::draw::TextMeasurer::default());
    state.board_picker_set_focus(BoardPickerFocus::PagePanel);

    state.handle_board_picker_key_with_measurer(&route_measurer, Key::Char('/'));
    for ch in "match".chars() {
        state.handle_board_picker_key_with_measurer(&route_measurer, Key::Char(ch));
    }
    state.handle_board_picker_key_with_measurer(&route_measurer, Key::F3);
    assert_eq!(state.board_picker_page_search_cursor(), Some(1));
    assert_eq!(state.board_picker_page_search_active_match(), Some(1));

    state.delete_page_in_board_with_measurer(&route_measurer, blackboard, 0);
    state.delete_page_in_board_with_measurer(&route_measurer, blackboard, 0);

    assert_eq!(state.board_picker_page_search_match_count(), 1);
    assert_eq!(state.board_picker_page_search_cursor(), Some(1));
    assert_eq!(state.board_picker_page_search_active_match(), Some(0));

    assert!(state.handle_board_picker_key_with_measurer(&route_measurer, Key::Return));
    assert!(!state.is_board_picker_open());
    assert_eq!(
        state.boards.board_states()[blackboard].pages.active_index(),
        0
    );
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

fn canvas_menu_commands(state: &InputState) -> Vec<Option<MenuCommand>> {
    state
        .context_menu_entries()
        .into_iter()
        .map(|entry| entry.command)
        .collect()
}

#[test]
fn canvas_menu_leads_with_history_and_ends_with_clear_then_exit() {
    let mut state = create_test_input_state();
    state.open_context_menu((40, 40), Vec::new(), ContextMenuKind::Canvas, None);

    let entries = state.context_menu_entries();
    let commands = canvas_menu_commands(&state);
    assert_eq!(
        &commands[..4],
        &[
            Some(MenuCommand::Undo),
            Some(MenuCommand::Redo),
            Some(MenuCommand::Paste),
            Some(MenuCommand::CaptureRegion),
        ]
    );
    assert!(entries[0].disabled && entries[1].disabled, "no history yet");
    assert!(entries[2].separator_before, "Paste starts its own group");
    assert_eq!(entries[3].label, "Capture Region…");

    let clear = commands
        .iter()
        .position(|command| *command == Some(MenuCommand::ClearAll))
        .expect("clear entry");
    let exit = entries.last().expect("entries");
    assert_eq!(exit.command, Some(MenuCommand::Exit));
    assert_eq!(exit.label, "Exit");
    assert!(exit.separator_before);
    assert_eq!(clear, entries.len() - 2, "Clear sits just above Exit");
    assert!(entries[clear].separator_before, "Clear is fenced off");
    assert!(clear > 8, "Clear is nowhere near Paste");
}

#[test]
fn highlight_row_uses_the_action_short_label() {
    let mut state = create_test_input_state();
    state.open_context_menu((40, 40), Vec::new(), ContextMenuKind::Canvas, None);

    let highlight = menu_entry(&state, MenuCommand::ToggleHighlightTool);

    assert_eq!(highlight.label, "Highlight");
    assert!(!highlight.label.contains("(tool + click)"));
}

#[test]
fn undo_and_redo_rows_follow_history_and_run_it() {
    let mut state = create_test_input_state();
    state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 0,
        y: 0,
        w: 10,
        h: 10,
        fill: false,
        color: state.style.current_color,
        thick: 2.0,
    });
    let shapes = state.boards.active_frame().shapes.clone();
    state.boards.active_frame_mut().push_undo_action(
        UndoAction::Create {
            shapes: vec![(0, shapes[0].clone())],
        },
        16,
    );
    state.open_context_menu((40, 40), Vec::new(), ContextMenuKind::Canvas, None);
    assert!(!menu_entry(&state, MenuCommand::Undo).disabled);
    assert!(menu_entry(&state, MenuCommand::Redo).disabled);

    state.execute_menu_command(MenuCommand::Undo);

    assert!(!state.is_context_menu_open());
    assert!(state.boards.active_frame().shapes.is_empty());
    state.open_context_menu((40, 40), Vec::new(), ContextMenuKind::Canvas, None);
    assert!(menu_entry(&state, MenuCommand::Undo).disabled);
    assert!(!menu_entry(&state, MenuCommand::Redo).disabled);
}

#[test]
fn exit_row_exits_or_hides_a_daemon_overlay() {
    let mut state = create_test_input_state();
    state.open_context_menu((40, 40), Vec::new(), ContextMenuKind::Canvas, None);

    state.execute_menu_command(MenuCommand::Exit);

    assert!(!state.is_context_menu_open());
    assert!(state.should_exit);

    let mut state = create_test_input_state();
    state.set_context_menu_exit_hides_overlay(true);
    state.open_context_menu((40, 40), Vec::new(), ContextMenuKind::Canvas, None);
    assert_eq!(menu_entry(&state, MenuCommand::Exit).label, "Hide Overlay");
}

#[test]
fn capture_region_row_hands_the_capture_to_the_backend() {
    let mut state = create_test_input_state();
    state.open_context_menu((40, 40), Vec::new(), ContextMenuKind::Canvas, None);

    state.execute_menu_command(MenuCommand::CaptureRegion);

    assert!(!state.is_context_menu_open());
    assert!(matches!(
        state.take_pending_backend_action(),
        Some(crate::input::state::PendingBackendAction::Screenshot(
            Action::CaptureRegionInteractive
        ))
    ));
}

#[test]
fn shape_menu_also_ends_with_exit() {
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 0,
        y: 0,
        w: 10,
        h: 10,
        fill: false,
        color: state.style.current_color,
        thick: 2.0,
    });
    state.set_selection(vec![shape_id]);
    state.open_context_menu(
        (40, 40),
        vec![shape_id],
        ContextMenuKind::Shape,
        Some(shape_id),
    );

    let entries = state.context_menu_entries();
    let exit = entries.last().expect("entries");
    assert_eq!(exit.command, Some(MenuCommand::Exit));
    assert!(exit.separator_before);
    assert!(
        !entries[0].separator_before,
        "the first row never has a divider"
    );
}

#[test]
fn the_footer_lives_inside_the_menu_and_leaves_row_hits_alone() {
    let engine = crate::ui_text::UiTextEngine::default();
    let mut state = create_test_input_state();
    state.open_context_menu((40, 40), Vec::new(), ContextMenuKind::Canvas, None);
    state.update_context_menu_layout_with_engine(&engine, 1280, 900);
    let layout = *state.context_menu_layout().expect("layout");
    let rows = state.context_menu_entries().len();

    assert!(layout.footer_height > 0.0);
    assert!(layout.footer_font_size >= 12.0, "readable footer");
    let rows_bottom = layout.origin_y + layout.padding_y + layout.row_height * rows as f64;
    assert!(
        (layout.origin_y + layout.height - (rows_bottom + layout.footer_height + layout.padding_y))
            .abs()
            < 0.01,
        "the footer is part of the box"
    );
    let x = (layout.origin_x + layout.padding_x) as i32;
    let last_row_y = (rows_bottom - layout.row_height * 0.5) as i32;
    assert_eq!(state.context_menu_index_at(x, last_row_y), Some(rows - 1));
    let footer_y = (rows_bottom + layout.footer_height * 0.5) as i32;
    assert_eq!(state.context_menu_index_at(x, footer_y), None);

    // A submenu has no footer of its own.
    let boards = state
        .context_menu_entries()
        .iter()
        .position(|entry| entry.label == "Boards")
        .expect("boards row");
    assert!(state.open_context_submenu(boards, false));
    state.update_context_menu_layout_with_engine(&engine, 1280, 900);
    assert_eq!(
        state.context_submenu_layout().expect("pane").footer_height,
        0.0
    );
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
