use std::time::{Duration, Instant};

use super::*;
use crate::input::BOARD_ID_WHITEBOARD;
use crate::input::events::Key;
use crate::input::state::{SUBMENU_AIM_GRACE, SUBMENU_HOVER_DELAY, SubmenuSide};

const SCREEN: (u32, u32) = (1280, 720);

fn open_canvas_menu(state: &mut InputState, anchor: (i32, i32)) {
    state.open_context_menu(anchor, Vec::new(), ContextMenuKind::Canvas, None);
    state.update_context_menu_layout(SCREEN.0, SCREEN.1);
}

fn row_of(state: &InputState, command: MenuCommand) -> usize {
    state
        .context_menu_entries()
        .iter()
        .position(|entry| entry.command.as_ref() == Some(&command))
        .expect("menu row")
}

fn parent_row_of(state: &InputState, kind: ContextMenuKind) -> usize {
    state
        .context_menu_entries()
        .iter()
        .position(|entry| entry.submenu == Some(kind))
        .expect("parent row")
}

/// A point inside a row of the menu, near its right side.
fn menu_row(state: &InputState, row: usize) -> (i32, i32) {
    let layout = state.context_menu_layout().expect("menu layout");
    (
        (layout.origin_x + layout.width - 30.0) as i32,
        (layout.origin_y + layout.padding_y + layout.row_height * (row as f64 + 0.5)) as i32,
    )
}

fn submenu_row(state: &InputState, row: usize) -> (i32, i32) {
    let layout = state.context_submenu_layout().expect("submenu layout");
    (
        (layout.origin_x + layout.padding_x) as i32,
        (layout.origin_y + layout.padding_y + layout.row_height * (row as f64 + 0.5)) as i32,
    )
}

fn root_kind(state: &InputState) -> Option<ContextMenuKind> {
    match &state.context_menu.state {
        ContextMenuState::Open { kind, .. } => Some(*kind),
        ContextMenuState::Hidden => None,
    }
}

fn submenu_kind(state: &InputState) -> Option<ContextMenuKind> {
    state.context_submenu().map(|submenu| submenu.kind)
}

fn root_hover(state: &InputState) -> Option<usize> {
    match &state.context_menu.state {
        ContextMenuState::Open { hover_index, .. } => *hover_index,
        ContextMenuState::Hidden => None,
    }
}

/// Moves the pointer over the menu and lets the hover delay elapse, as the
/// event loop does once the pointer rests.
fn hover_and_settle(state: &mut InputState, x: i32, y: i32) {
    state.update_pointer_position_synthetic(x, y);
    state.update_context_menu_hover_from_pointer(x, y);
    settle(state);
}

fn settle(state: &mut InputState) {
    state.tick_context_menu_hover(Instant::now() + SUBMENU_AIM_GRACE + Duration::from_secs(1));
    state.update_context_menu_layout(SCREEN.0, SCREEN.1);
}

fn text_resources() -> (crate::draw::TextMeasurer, crate::ui_text::UiTextEngine) {
    (
        crate::draw::TextMeasurer::default(),
        crate::ui_text::UiTextEngine::default(),
    )
}

#[test]
fn hovering_a_parent_row_opens_its_submenu_beside_it_after_a_pause() {
    let mut state = create_test_input_state();
    open_canvas_menu(&mut state, (100, 100));
    let boards = parent_row_of(&state, ContextMenuKind::Boards);
    let (x, y) = menu_row(&state, boards);

    state.update_pointer_position_synthetic(x, y);
    state.update_context_menu_hover_from_pointer(x, y);
    assert_eq!(root_hover(&state), Some(boards), "hover follows at once");
    assert_eq!(
        submenu_kind(&state),
        None,
        "the pane waits for the pointer to rest"
    );
    assert!(state.context_menu_hover_timeout(Instant::now()) <= Some(SUBMENU_HOVER_DELAY));

    settle(&mut state);

    assert_eq!(root_kind(&state), Some(ContextMenuKind::Canvas));
    let submenu = state.context_submenu().expect("boards submenu");
    assert_eq!(
        (submenu.kind, submenu.parent_index, submenu.keyboard_focus),
        (ContextMenuKind::Boards, boards, None)
    );
    let menu = *state.context_menu_layout().unwrap();
    let pane = *state.context_submenu_layout().unwrap();
    assert!(pane.origin_x >= menu.origin_x + menu.width);
    assert_eq!(
        pane.origin_y + pane.padding_y,
        menu.origin_y + menu.padding_y + menu.row_height * boards as f64,
        "the first submenu entry lines up with its row"
    );
    assert_eq!(state.context_submenu_side(), SubmenuSide::Right);
}

#[test]
fn a_submenu_opens_on_the_left_when_the_right_edge_is_too_close() {
    let mut state = create_test_input_state();
    open_canvas_menu(&mut state, (SCREEN.0 as i32 - 10, 100));
    assert_eq!(
        state.context_submenu_side(),
        SubmenuSide::Left,
        "arrows point left before any pane opens"
    );

    state.execute_menu_command(MenuCommand::OpenBoardsMenu);
    state.update_context_menu_layout(SCREEN.0, SCREEN.1);

    let menu = *state.context_menu_layout().unwrap();
    let pane = *state.context_submenu_layout().unwrap();
    assert!(pane.origin_x + pane.width <= menu.origin_x);
    assert_eq!(state.context_submenu_side(), SubmenuSide::Left);
}

#[test]
fn heading_into_a_submenu_keeps_it_while_other_rows_switch_or_close_it() {
    let mut state = create_test_input_state();
    open_canvas_menu(&mut state, (100, 100));
    let boards = parent_row_of(&state, ContextMenuKind::Boards);
    let pages = parent_row_of(&state, ContextMenuKind::Pages);
    let help = row_of(&state, MenuCommand::ToggleHelp);
    assert_eq!(pages, boards + 1, "Pages sits right below Boards");
    let (x, boards_y) = menu_row(&state, boards);
    let (_, pages_y) = menu_row(&state, pages);
    let (_, help_y) = menu_row(&state, help);

    hover_and_settle(&mut state, x, boards_y);
    assert_eq!(submenu_kind(&state), Some(ContextMenuKind::Boards));

    // A diagonal step toward the Boards submenu crosses the Pages row without
    // moving the highlight off Boards.
    state.update_pointer_position_synthetic(x + 20, pages_y);
    state.update_context_menu_hover_from_pointer(x + 20, pages_y);
    assert_eq!(submenu_kind(&state), Some(ContextMenuKind::Boards));
    assert_eq!(root_hover(&state), Some(boards));

    // Moving within the Pages row without heading into the submenu switches
    // once the pointer rests.
    hover_and_settle(&mut state, x + 20, pages_y + 4);
    assert_eq!(submenu_kind(&state), Some(ContextMenuKind::Pages));

    // A row without a submenu closes it and leaves the menu open.
    hover_and_settle(&mut state, x + 20, help_y);
    assert_eq!(submenu_kind(&state), None);
    assert!(state.is_context_menu_open());
}

#[test]
fn a_pointer_that_stops_inside_the_aim_triangle_settles_on_its_row() {
    let mut state = create_test_input_state();
    open_canvas_menu(&mut state, (100, 100));
    let boards = parent_row_of(&state, ContextMenuKind::Boards);
    let pages = parent_row_of(&state, ContextMenuKind::Pages);
    let (x, boards_y) = menu_row(&state, boards);
    let (_, pages_y) = menu_row(&state, pages);

    hover_and_settle(&mut state, x, boards_y);
    state.update_pointer_position_synthetic(x + 20, pages_y);
    state.update_context_menu_hover_from_pointer(x + 20, pages_y);
    assert_eq!(submenu_kind(&state), Some(ContextMenuKind::Boards));
    let timeout = state
        .context_menu_hover_timeout(Instant::now())
        .expect("the aim grace is bounded");
    assert!(timeout <= SUBMENU_AIM_GRACE);

    // Nothing moves. Before the grace ends the pane stays; after it the row
    // under the resting pointer wins.
    assert!(!state.tick_context_menu_hover(Instant::now()));
    assert_eq!(submenu_kind(&state), Some(ContextMenuKind::Boards));
    assert!(state.tick_context_menu_hover(Instant::now() + SUBMENU_AIM_GRACE * 2));
    assert_eq!(submenu_kind(&state), Some(ContextMenuKind::Pages));
    assert_eq!(root_hover(&state), Some(pages));
    assert!(state.needs_redraw);
    assert_eq!(state.context_menu_hover_timeout(Instant::now()), None);
}

#[test]
fn sweeping_across_a_parent_row_does_not_open_its_submenu() {
    let mut state = create_test_input_state();
    open_canvas_menu(&mut state, (100, 100));
    let boards = parent_row_of(&state, ContextMenuKind::Boards);
    let help = row_of(&state, MenuCommand::ToggleHelp);
    let (x, boards_y) = menu_row(&state, boards);
    let (_, help_y) = menu_row(&state, help);

    state.update_context_menu_hover_from_pointer(x, boards_y);
    state.update_context_menu_hover_from_pointer(x, help_y);
    settle(&mut state);

    assert_eq!(submenu_kind(&state), None);
    assert_eq!(root_hover(&state), Some(help));
}

#[test]
fn right_opens_a_submenu_and_left_or_escape_return_to_its_row() {
    let (measurer, ui_engine) = text_resources();
    let resources = crate::input::state::InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    let press = |state: &mut InputState, key: Key| {
        state.handle_context_menu_key_with_resources(resources, key)
    };
    let mut state = create_test_input_state();
    state.toggle_context_menu_via_keyboard();
    let boards = parent_row_of(&state, ContextMenuKind::Boards);
    state.set_context_menu_focus(Some(boards));

    assert!(press(&mut state, Key::Right));
    let first = state
        .context_submenu()
        .and_then(|submenu| submenu.keyboard_focus)
        .expect("focus moves into the submenu");
    assert!(!state.context_submenu_entries()[first].disabled);
    assert!(state.context_submenu_is_active());
    assert!(press(&mut state, Key::Down));
    assert_ne!(
        state
            .context_submenu()
            .and_then(|submenu| submenu.keyboard_focus),
        Some(first)
    );

    assert!(press(&mut state, Key::Left));
    assert_eq!(submenu_kind(&state), None);
    assert!(matches!(
        state.context_menu.state,
        ContextMenuState::Open {
            keyboard_focus: Some(row),
            ..
        } if row == boards
    ));

    assert!(press(&mut state, Key::Right));
    assert!(press(&mut state, Key::Escape));
    assert_eq!(submenu_kind(&state), None);
    assert!(state.is_context_menu_open());
    assert!(press(&mut state, Key::Escape));
    assert!(!state.is_context_menu_open());
}

#[test]
fn the_menu_swallows_arrow_keys_that_have_nothing_to_do() {
    let (measurer, ui_engine) = text_resources();
    let resources = crate::input::state::InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    let mut state = create_test_input_state();
    state.toggle_context_menu_via_keyboard();
    let help = row_of(&state, MenuCommand::ToggleHelp);
    state.set_context_menu_focus(Some(help));

    assert!(state.handle_context_menu_key_with_resources(resources, Key::Right));
    assert_eq!(submenu_kind(&state), None);
    assert!(state.handle_context_menu_key_with_resources(resources, Key::Left));
    assert!(state.is_context_menu_open());
}

#[test]
fn hovering_the_parent_row_hands_the_selection_back_from_the_submenu() {
    let (measurer, ui_engine) = text_resources();
    let resources = crate::input::state::InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    let mut state = create_test_input_state();
    open_canvas_menu(&mut state, (100, 100));
    let boards = parent_row_of(&state, ContextMenuKind::Boards);
    state.set_context_menu_focus(Some(boards));
    assert!(state.handle_context_menu_key_with_resources(resources, Key::Right));
    assert!(state.context_submenu_is_active());

    // The mouse lands on the parent row: the pane stays, its keyboard focus
    // goes, and Enter acts on the hovered row.
    let (x, y) = menu_row(&state, boards);
    hover_and_settle(&mut state, x, y);
    assert_eq!(submenu_kind(&state), Some(ContextMenuKind::Boards));
    assert_eq!(
        state
            .context_submenu()
            .and_then(|submenu| submenu.keyboard_focus),
        None
    );
    assert!(!state.context_submenu_is_active());
}

#[test]
fn clicking_a_parent_row_opens_it_and_clicking_an_entry_runs_it() {
    let (measurer, ui_engine) = text_resources();
    let resources = crate::input::state::InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    open_canvas_menu(&mut state, (100, 100));
    let pages = parent_row_of(&state, ContextMenuKind::Pages);
    let page_count = state.boards.page_count();

    let (x, y) = menu_row(&state, pages);
    assert!(state.handle_context_menu_release_at_with_resources(resources, x, y));
    assert_eq!(submenu_kind(&state), Some(ContextMenuKind::Pages));
    state.update_context_menu_layout(SCREEN.0, SCREEN.1);

    let new_page = state
        .context_submenu_entries()
        .iter()
        .position(|entry| entry.command == Some(MenuCommand::PageNew))
        .expect("New Page entry");
    let (x, y) = submenu_row(&state, new_page);
    assert!(state.handle_context_menu_release_at_with_resources(resources, x, y));
    assert_eq!(state.boards.page_count(), page_count + 1);
    assert!(!state.is_context_menu_open());
}

#[test]
fn clicking_an_expanded_parent_row_collapses_it_until_the_pointer_leaves() {
    let (measurer, ui_engine) = text_resources();
    let resources = crate::input::state::InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    let mut state = create_test_input_state();
    open_canvas_menu(&mut state, (100, 100));
    let boards = parent_row_of(&state, ContextMenuKind::Boards);
    let pages = parent_row_of(&state, ContextMenuKind::Pages);
    let (x, boards_y) = menu_row(&state, boards);
    let (_, pages_y) = menu_row(&state, pages);
    hover_and_settle(&mut state, x, boards_y);
    assert_eq!(submenu_kind(&state), Some(ContextMenuKind::Boards));

    assert!(state.handle_context_menu_release_at_with_resources(resources, x, boards_y));
    assert_eq!(submenu_kind(&state), None);
    assert!(state.is_context_menu_open());

    // A jiggle on the same row does not reopen it.
    hover_and_settle(&mut state, x + 1, boards_y + 1);
    assert_eq!(submenu_kind(&state), None);

    // Leaving and returning does.
    hover_and_settle(&mut state, x, pages_y);
    assert_eq!(submenu_kind(&state), Some(ContextMenuKind::Pages));
    hover_and_settle(&mut state, x, boards_y);
    assert_eq!(submenu_kind(&state), Some(ContextMenuKind::Boards));
}

#[test]
fn keyboard_navigation_cancels_a_pending_hover_open() {
    let (measurer, ui_engine) = text_resources();
    let resources = crate::input::state::InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    let mut state = create_test_input_state();
    open_canvas_menu(&mut state, (100, 100));
    let boards = parent_row_of(&state, ContextMenuKind::Boards);
    let pages = parent_row_of(&state, ContextMenuKind::Pages);
    let (x, y) = menu_row(&state, boards);
    state.update_pointer_position_synthetic(x, y);
    state.update_context_menu_hover_from_pointer(x, y);

    // Down before the delay elapses moves the selection to Pages.
    assert!(state.handle_context_menu_key_with_resources(resources, Key::Down));
    assert_eq!(state.context_menu_hover_timeout(Instant::now()), None);
    settle(&mut state);

    assert_eq!(
        submenu_kind(&state),
        None,
        "the resting pointer must not undo the keyboard"
    );
    assert!(state.handle_context_menu_key_with_resources(resources, Key::Right));
    assert_eq!(submenu_kind(&state), Some(ContextMenuKind::Pages));
    assert_eq!(
        state.context_submenu().map(|submenu| submenu.parent_index),
        Some(pages)
    );
}

#[test]
fn leaving_the_menu_lifts_a_collapsed_parent_rows_suppression() {
    let (measurer, ui_engine) = text_resources();
    let resources = crate::input::state::InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    let mut state = create_test_input_state();
    open_canvas_menu(&mut state, (100, 100));
    let boards = parent_row_of(&state, ContextMenuKind::Boards);
    let (x, y) = menu_row(&state, boards);
    hover_and_settle(&mut state, x, y);
    assert!(state.handle_context_menu_release_at_with_resources(resources, x, y));
    assert_eq!(submenu_kind(&state), None);

    // Out of the menu and straight back onto the same row.
    let menu = *state.context_menu_layout().unwrap();
    let outside_x = (menu.origin_x + menu.width + 200.0) as i32;
    hover_and_settle(&mut state, outside_x, y);
    assert!(state.is_context_menu_open());
    hover_and_settle(&mut state, x, y);

    assert_eq!(submenu_kind(&state), Some(ContextMenuKind::Boards));
}

#[test]
fn page_overflow_from_the_picker_keeps_the_picker_open() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    state.open_board_picker_with_measurer(&crate::draw::TextMeasurer::default());
    state.execute_menu_command(MenuCommand::OpenPagesMenu);
    assert!(state.is_board_picker_open() && state.is_context_menu_open());

    state.execute_menu_command(MenuCommand::OpenBoardPicker);

    assert!(!state.is_context_menu_open());
    assert!(
        state.is_board_picker_open(),
        "the picker underneath stays open"
    );
}

#[test]
fn a_menu_command_with_no_parent_row_opens_that_menu_on_its_own() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    state.open_board_picker_with_measurer(&crate::draw::TextMeasurer::default());
    state.update_pointer_position_synthetic(300, 200);

    // The board picker's page overflow link runs this with no menu open.
    state.execute_menu_command(MenuCommand::OpenPagesMenu);

    assert_eq!(root_kind(&state), Some(ContextMenuKind::Pages));
    assert_eq!(submenu_kind(&state), None);
    assert!(matches!(
        state.context_menu.state,
        ContextMenuState::Open {
            anchor: (300, 200),
            ..
        }
    ));
    let entries = state.context_menu_entries();
    assert!(
        entries[0].disabled && entries[0].label.ends_with("Page 1/1"),
        "a standalone menu keeps its header: {}",
        entries[0].label
    );
}

#[test]
fn a_submenu_drops_the_header_its_parent_row_already_shows() {
    let mut state = create_test_input_state();
    open_canvas_menu(&mut state, (100, 100));
    let boards = parent_row_of(&state, ContextMenuKind::Boards);
    let summary = state.context_menu_entries()[boards].shortcut.clone();
    state.execute_menu_command(MenuCommand::OpenBoardsMenu);

    let entries = state.context_submenu_entries();
    let summary = summary.expect("the parent row summarises the submenu");
    assert!(summary.starts_with("Overlay (1/"), "{summary}");
    assert!(
        entries[0].command.is_some(),
        "the row beside the parent is a board, not a header: {}",
        entries[0].label
    );
    assert!(entries.iter().all(|entry| entry.label != summary));
}

#[test]
fn opening_a_submenu_keeps_the_menu_without_repainting_the_whole_surface() {
    let mut state = create_test_input_state();
    open_canvas_menu(&mut state, (100, 100));
    let _ = state.dirty_tracker.take_regions(1280, 720);

    state.execute_menu_command(MenuCommand::OpenBoardsMenu);

    assert_eq!(root_kind(&state), Some(ContextMenuKind::Canvas));
    assert_eq!(submenu_kind(&state), Some(ContextMenuKind::Boards));
    assert!(state.needs_redraw);
    let regions = state.dirty_tracker.take_regions(1280, 720);
    assert!(
        !regions
            .iter()
            .any(|rect| rect.width >= 1280 && rect.height >= 720)
    );
}
