use super::*;
use crate::draw::frame::UndoAction;
use crate::input::state::core::properties::SelectionPropertyKind;

#[test]
fn explicit_duplicate_and_nested_history_restore_decorated_hits_and_damage() {
    for (shape, probe) in fixtures() {
        let owner = TextMeasurer::default();
        let (mut state, original_id, locked_id) = state_with(shape);
        let locked_bounds = bounds(&state, &owner, locked_id);
        assert_eq!(
            state.hit_test_at_with(&owner, probe.0, probe.1),
            Some(original_id)
        );
        assert!(state.duplicate_selection_with(&owner));
        let duplicated_id = state.selected_shape_ids()[0];
        assert_ne!(duplicated_id, original_id);
        assert_eq!(state.selected_shape_ids().len(), 1);
        assert_eq!(state.boards.active_frame().shapes.len(), 3);
        assert_eq!(bounds(&state, &owner, locked_id), locked_bounds);
        let duplicated_bounds = bounds(&state, &owner, duplicated_id);
        assert_dirty_covers(&state.take_dirty_regions(), duplicated_bounds);
        let shifted_probe = (probe.0 + 12, probe.1 + 12);
        assert_eq!(
            state.hit_test_at_with(&owner, shifted_probe.0, shifted_probe.1),
            Some(duplicated_id)
        );

        let action = state
            .boards
            .active_frame_mut()
            .undo_last()
            .expect("duplicate undo");
        // Both recursive traversals must reach a grandchild action after frame mutation.
        let nested = UndoAction::Compound {
            actions: vec![UndoAction::Compound {
                actions: vec![action],
            }],
        };
        state.apply_action_side_effects_with(&owner, &nested);
        assert!(state.boards.active_frame().shape(duplicated_id).is_none());
        assert!(state.selected_shape_ids().is_empty());
        assert_dirty_covers(&state.take_dirty_regions(), duplicated_bounds);
        assert_ne!(
            state.hit_test_at_with(&owner, shifted_probe.0, shifted_probe.1),
            Some(duplicated_id)
        );

        let action = state
            .boards
            .active_frame_mut()
            .redo_last()
            .expect("duplicate redo");
        state.apply_action_side_effects_with(&owner, &action);
        assert_eq!(bounds(&state, &owner, duplicated_id), duplicated_bounds);
        assert_dirty_covers(&state.take_dirty_regions(), duplicated_bounds);
        assert_eq!(
            state.hit_test_at_with(&owner, shifted_probe.0, shifted_probe.1),
            Some(duplicated_id)
        );
    }
}

#[test]
fn explicit_font_property_updates_wrapped_bounds_and_undo_keeps_locked_text() {
    let owner = TextMeasurer::default();
    let (shape, probe) = fixtures().remove(0);
    let (mut state, id, locked_id) = state_with(shape);
    let original = bounds(&state, &owner, id);
    let locked = bounds(&state, &owner, locked_id);
    state.hit_test_at_with(&owner, probe.0, probe.1);
    let generation = state.canvas_content_generation();
    assert!(state.adjust_selection_property_kind_with(&owner, SelectionPropertyKind::FontSize, 1));
    let changed = bounds(&state, &owner, id);
    assert_ne!(changed, original);
    assert_eq!(bounds(&state, &owner, locked_id), locked);
    assert!(state.canvas_content_generation() > generation);
    let dirty = state.take_dirty_regions();
    assert_dirty_covers(&dirty, original);
    assert_dirty_covers(&dirty, changed);
    let action = state
        .boards
        .active_frame_mut()
        .undo_last()
        .expect("font undo");
    state.apply_action_side_effects_with(&owner, &action);
    assert_eq!(bounds(&state, &owner, id), original);
    assert_eq!(bounds(&state, &owner, locked_id), locked);
    let dirty = state.take_dirty_regions();
    assert_dirty_covers(&dirty, original);
    assert_dirty_covers(&dirty, changed);
    assert_eq!(state.hit_test_at_with(&owner, probe.0, probe.1), Some(id));
}

#[test]
fn explicit_clipboard_paste_rejects_superseded_request_without_mutation() {
    let owner = TextMeasurer::default();
    let (shape, _) = fixtures().remove(0);
    let (mut state, id, _) = state_with(shape.clone());
    let stale = state.request_clipboard_paste();
    let current = state.request_clipboard_paste();
    let count = state.boards.active_frame().shapes.len();
    let generation = state.canvas_content_generation();
    assert_eq!(
        state.paste_clipboard_shapes_from_request_with(&owner, &stale, vec![shape.clone()]),
        0
    );
    assert_eq!(state.boards.active_frame().shapes.len(), count);
    assert_eq!(state.canvas_content_generation(), generation);
    assert!(state.take_dirty_regions().is_empty());
    assert_eq!(
        state.paste_clipboard_shapes_from_request_with(&owner, &current, vec![shape]),
        1
    );
    let pasted_id = state.selected_shape_ids()[0];
    assert_ne!(pasted_id, id);
    assert_eq!(state.boards.active_frame().shapes.len(), count + 1);
    assert_dirty_covers(
        &state.take_dirty_regions(),
        bounds(&state, &owner, pasted_id),
    );
}

#[test]
fn explicit_keyboard_menu_anchor_projects_decorated_selection_with_pan_and_zoom() {
    use crate::input::state::core::menus::ContextMenuState;

    let owner = TextMeasurer::default();
    let (shape, _) = fixtures().remove(0);
    let (mut state, id, _) = state_with(shape);
    state.set_selection(vec![id]);
    let canvas = bounds(&state, &owner, id);
    state.view.set_zoom_status(true, false, 2.0, (20.0, 10.0));
    state.toggle_context_menu_via_keyboard_with(&owner);
    let ContextMenuState::Open {
        anchor, shape_ids, ..
    } = &state.context_menu.state
    else {
        panic!("keyboard menu should open for the selected text");
    };
    assert_eq!(
        *anchor,
        (
            (canvas.x - 20) * 2 + canvas.width,
            (canvas.y - 10) * 2 + canvas.height
        )
    );
    assert_eq!(shape_ids, &[id]);
    state.toggle_context_menu_via_keyboard_with(&owner);
    assert!(!state.is_context_menu_open());
}
