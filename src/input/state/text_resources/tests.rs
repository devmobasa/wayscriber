use super::*;
use crate::draw::{FontDescriptor, Shape, ShapeId};
use crate::input::{DrawingState, InputState, Tool};

fn editing_state(measurer: &TextMeasurer) -> (InputState, ShapeId, Shape) {
    let mut state = crate::input::state::test_support::make_test_input_state();
    state.view.set_screen_dimensions(1000, 800);
    let original = Shape::Text {
        x: 100,
        y: 200,
        text: "Wrapped original שלום".into(),
        size: 24.0,
        color: crate::draw::RED,
        font_descriptor: FontDescriptor::default(),
        background_enabled: true,
        wrap_width: Some(140),
    };
    let id = state.boards.active_frame_mut().add_shape(original.clone());
    state.set_selection(vec![id]);
    assert!(state.edit_selected_text_with(measurer));
    assert!(
        matches!(&state.boards.active_frame().shape(id).unwrap().shape, Shape::Text { text, .. } if text.is_empty())
    );
    (state, id, original)
}

fn assert_restored(state: &InputState, id: ShapeId, original: &Shape, measurer: &TextMeasurer) {
    assert!(matches!(state.state, DrawingState::Idle));
    let shape = &state.boards.active_frame().shape(id).unwrap().shape;
    assert_eq!(
        serde_json::to_value(shape).unwrap(),
        serde_json::to_value(original).unwrap()
    );
    assert_eq!(
        shape.bounding_box_with(measurer),
        original.bounding_box_with(measurer)
    );
}

#[test]
fn explicit_page_transition_restores_text_before_changing_the_active_frame() {
    let measurer = TextMeasurer::default();
    let (mut state, id, original) = editing_state(&measurer);
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    assert!(
        state.apply_toolbar_event_with_resources(
            resources,
            crate::ui::toolbar::ToolbarEvent::PageNew
        )
    );
    assert!(matches!(state.state, DrawingState::Idle));
    assert_eq!(state.boards.active_page_index(), 1);
    assert!(state.boards.active_frame().shapes.is_empty());
    assert!(state.switch_to_page_with_measurer(&measurer, 0));
    assert_restored(&state, id, &original, &measurer);
    assert!(state.is_session_dirty());
    assert!(!state.take_dirty_regions().is_empty());
}

#[test]
fn explicit_popup_openers_and_screen_modal_cancel_the_live_editor() {
    let measurer = TextMeasurer::default();
    for kind in 0..3 {
        let (mut state, id, original) = editing_state(&measurer);
        match kind {
            0 => state.open_color_picker_popup_with_measurer(&measurer),
            1 => state.open_board_picker_with_measurer(&measurer),
            _ => state.prepare_for_screen_modal_with_measurer(&measurer),
        }
        assert_restored(&state, id, &original, &measurer);
        assert_eq!(state.is_color_picker_popup_open(), kind == 0);
        assert_eq!(state.is_board_picker_open(), kind == 1);
    }
}

#[test]
fn explicit_light_and_presenter_switches_keep_restored_text_and_mode_policy() {
    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    let (mut state, id, original) = editing_state(&measurer);
    state.compositor_capabilities.layer_shell = true;
    assert!(state.toggle_light_mode_with_resources(resources));
    assert_restored(&state, id, &original, &measurer);
    assert!(state.light_mode_passthrough());
    assert_eq!(state.tool_override(), Some(Tool::Pen));
    assert!(state.toggle_presenter_mode_with_resources(resources));
    assert!(!state.light_mode_active());
    assert!(state.presenter_mode_active());
    assert_restored(&state, id, &original, &measurer);
    assert!(!state.toggle_presenter_mode_with_resources(resources));
    assert!(!state.presenter_mode_active());
    assert_restored(&state, id, &original, &measurer);
}

#[test]
fn explicit_key_repeat_and_action_share_text_geometry_and_undo_history() {
    use crate::domain::Action;
    use crate::input::Key;

    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    let (mut state, id, original) = editing_state(&measurer);
    state.on_key_press_with_resources(resources, Key::Escape);
    assert_restored(&state, id, &original, &measurer);
    state.set_selection(vec![id]);
    let original_bounds = original.bounding_box_with(&measurer).unwrap();
    let _ = state.take_dirty_regions();

    state.on_key_press_with_resources(resources, Key::Right);
    state.on_key_repeat_with_resources(resources, Key::Right);
    state.handle_action_with_resources(resources, Action::NudgeSelectionRight);

    let moved_bounds = state
        .boards
        .active_frame()
        .shape(id)
        .unwrap()
        .bounding_box_with(&measurer)
        .unwrap();
    assert_eq!(moved_bounds.x, original_bounds.x + 24);
    assert_eq!(moved_bounds.y, original_bounds.y);
    assert_eq!(moved_bounds.width, original_bounds.width);
    assert_eq!(moved_bounds.height, original_bounds.height);
    assert_eq!(state.boards.active_frame().undo_stack_len(), 3);
    assert!(!state.take_dirty_regions().is_empty());

    state.handle_action_with_resources(resources, Action::Undo);
    let undone_bounds = state
        .boards
        .active_frame()
        .shape(id)
        .unwrap()
        .bounding_box_with(&measurer)
        .unwrap();
    assert_eq!(undone_bounds.x, original_bounds.x + 16);
    assert!(state.selected_shape_ids().is_empty());
    assert!(!state.take_dirty_regions().is_empty());
}

#[test]
fn explicit_palette_enter_and_menu_command_preserve_text_undo_and_selection() {
    use crate::input::Key;
    use crate::input::state::core::MenuCommand;

    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    let (mut state, id, original) = editing_state(&measurer);
    state.on_key_press_with_resources(resources, Key::Escape);
    state.set_selection(vec![id]);
    state.execute_menu_command_with_resources(resources, MenuCommand::Delete);
    assert!(state.boards.active_frame().shape(id).is_none());
    assert!(state.selected_shape_ids().is_empty());

    state.toggle_command_palette();
    state.command_palette.query = "undo".into();
    state.on_key_press_with_resources(resources, Key::Return);
    assert!(!state.command_palette_is_engaged());
    assert_restored(&state, id, &original, &measurer);
    assert_eq!(state.boards.active_frame().redo_stack_len(), 1);
    assert!(!state.take_dirty_regions().is_empty());
}

#[test]
fn explicit_precision_enter_uses_toolbar_clamping_and_escape_leaves_value_alone() {
    use crate::input::Key;
    use crate::ui::toolbar::{PrecisionEntryTarget, ToolbarEvent};

    let measurer = TextMeasurer::default();
    let ui_engine = UiTextEngine::default();
    let resources = InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    let mut state = crate::input::state::test_support::make_test_input_state();
    assert!(state.apply_toolbar_event_with_resources(
        resources,
        ToolbarEvent::OpenPrecisionEntry(PrecisionEntryTarget::Thickness)
    ));
    for ch in "999".chars() {
        state.on_key_press_with_resources(resources, Key::Char(ch));
    }
    state.on_key_press_with_resources(resources, Key::Return);
    assert!(!state.is_precision_entry_open());
    assert_eq!(
        state.thickness_for_active_tool(),
        crate::ui::toolbar::model::ToolbarSliderSpec::THICKNESS.max
    );
    assert!(state.is_session_dirty());

    let before = state.thickness_for_active_tool();
    state.open_precision_entry(PrecisionEntryTarget::Thickness);
    state.on_key_press_with_resources(resources, Key::Char('7'));
    state.on_key_press_with_resources(resources, Key::Escape);
    assert!(!state.is_precision_entry_open());
    assert_eq!(state.thickness_for_active_tool(), before);
}
