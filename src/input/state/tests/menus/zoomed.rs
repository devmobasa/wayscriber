//! The context menu over a zoomed view: right-click and the keyboard
//! shortcut open it, and the canvas menu leads with the zoom controls.

use super::*;
use crate::input::ZoomAction;

fn zoomed_state(locked: bool) -> InputState {
    let mut state = create_test_input_state();
    state.set_zoom_status(true, locked, 2.0, (0.0, 0.0));
    state
}

fn labels(state: &InputState) -> Vec<String> {
    state
        .context_menu_entries()
        .into_iter()
        .map(|entry| entry.label)
        .collect()
}

/// Through the action the shortcut dispatches, so a zoom guard on that path
/// fails here.
#[test]
fn the_keyboard_shortcut_opens_the_menu_while_zoomed() {
    let measurer = crate::draw::TextMeasurer::default();
    let ui_engine = crate::ui_text::UiTextEngine::default();
    let mut state = zoomed_state(false);

    state.handle_action_with_resources(
        crate::input::state::InputTextResources {
            measurer: &measurer,
            ui_engine: &ui_engine,
        },
        Action::OpenContextMenu,
    );

    assert!(state.is_context_menu_open());
}

#[test]
fn a_zoomed_canvas_menu_leads_with_the_zoom_controls() {
    let mut state = zoomed_state(false);
    state.open_context_menu((12, 34), Vec::new(), ContextMenuKind::Canvas, None);

    let entries = state.context_menu_entries();
    assert_eq!(
        labels(&state)[..6],
        [
            "Zoom 200%",
            "Zoom In",
            "Zoom Out",
            "Exit Zoom",
            "Lock View",
            "Undo"
        ]
    );
    assert!(
        entries[0].disabled && entries[0].command.is_none(),
        "header"
    );
    for entry in &entries[1..5] {
        assert!(!entry.disabled, "{} is usable while zoomed", entry.label);
    }
    assert!(entries[5].separator_before, "the zoom group stands apart");
    assert!(
        entries
            .iter()
            .all(|entry| entry.submenu != Some(ContextMenuKind::Zoom)),
        "no second way into the same controls"
    );
    let boards = entries
        .iter()
        .position(|entry| entry.label == "Boards")
        .expect("boards row");
    let view_group = entries[..boards]
        .iter()
        .rposition(|entry| entry.separator_before)
        .expect("view group");
    assert!(
        entries[view_group].label != "Undo",
        "the view group keeps its own separator without the Zoom row"
    );
}

#[test]
fn an_unzoomed_canvas_menu_keeps_zoom_in_its_submenu() {
    let mut state = create_test_input_state();
    state.open_context_menu((12, 34), Vec::new(), ContextMenuKind::Canvas, None);

    let entries = state.context_menu_entries();
    assert_eq!(entries[0].label, "Undo");
    let zoom = entries
        .iter()
        .find(|entry| entry.label == "Zoom")
        .expect("zoom row");
    assert_eq!(zoom.submenu, Some(ContextMenuKind::Zoom));
    assert!(zoom.separator_before, "it still opens the view group");
    assert!(!labels(&state).iter().any(|label| label.ends_with("View")));
}

#[test]
fn the_lock_row_names_what_it_will_do_and_queues_the_toggle() {
    let mut state = zoomed_state(true);
    state.open_context_menu((12, 34), Vec::new(), ContextMenuKind::Zoom, None);
    assert!(labels(&state).contains(&"Unlock View".to_string()));

    state.execute_menu_command(MenuCommand::ToggleZoomLock);

    assert_eq!(
        state.take_pending_zoom_action(),
        Some(ZoomAction::ToggleLock)
    );
    assert!(!state.is_context_menu_open());
}
