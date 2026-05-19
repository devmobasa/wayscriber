mod actions;
mod active;
mod adapters;
mod event;
mod keyboard;
mod outcome;
mod pointer;

pub(crate) use actions::route_action;
pub(crate) use event::{
    CanvasPoint, PointerMotion, PointerPoints, PointerPress, PointerRelease, ScreenPoint,
};
pub(crate) use keyboard::route_key_press;
pub(crate) use pointer::{route_pointer_motion, route_pointer_press, route_pointer_release};

#[cfg(test)]
mod tests {
    use super::actions::classify_action;
    use super::active::active_interaction_kind;
    use super::outcome::{
        ActionRoute, ActiveInteractionKind, CancelTarget, ConsumedBy, InteractionSideEffect,
        KeyboardSideEffect, PointerSideEffect, RoutingOutcome,
    };
    use super::*;
    use crate::config::Action;
    use crate::draw::Shape;
    use crate::input::state::test_support::make_test_input_state;
    use crate::input::{BOARD_ID_BLACKBOARD, EraserMode, Key, MouseButton, Tool};

    fn points() -> PointerPoints {
        PointerPoints::new(ScreenPoint::new(10, 20), CanvasPoint::new(10, 20))
    }

    fn add_rect(state: &mut crate::input::state::InputState) -> crate::draw::ShapeId {
        state.boards.active_frame_mut().add_shape(Shape::Rect {
            x: 10,
            y: 20,
            w: 30,
            h: 40,
            fill: false,
            color: state.current_color,
            thick: state.current_thickness,
        })
    }

    #[test]
    fn active_interaction_kind_is_only_non_idle() {
        let mut state = make_test_input_state();

        assert_eq!(active_interaction_kind(&state), None);

        state.state = crate::input::state::DrawingState::Drawing {
            tool: Tool::Pen,
            start_x: 1,
            start_y: 2,
            points: vec![(1, 2)],
            point_thicknesses: vec![1.0],
        };
        assert_eq!(
            active_interaction_kind(&state),
            Some(ActiveInteractionKind::Drawing)
        );

        state.state = crate::input::state::DrawingState::TextInput {
            x: 1,
            y: 2,
            buffer: String::new(),
        };
        assert_eq!(
            active_interaction_kind(&state),
            Some(ActiveInteractionKind::TextInput)
        );
    }

    #[test]
    fn modifier_key_press_returns_named_keyboard_side_effect() {
        let mut state = make_test_input_state();

        assert_eq!(
            route_key_press(&mut state, Key::Shift),
            RoutingOutcome::SideEffect(InteractionSideEffect::Keyboard(
                KeyboardSideEffect::ModifierUpdated
            ))
        );
        assert!(state.modifiers.shift);
    }

    #[test]
    fn properties_panel_unhandled_key_is_consumed() {
        let mut state = make_test_input_state();
        let id = add_rect(&mut state);
        state.set_selection(vec![id]);
        assert!(state.show_properties_panel());

        assert_eq!(
            route_key_press(&mut state, Key::Char('x')),
            RoutingOutcome::Consumed(ConsumedBy::PropertiesPanel)
        );
        assert!(state.is_properties_panel_open());
    }

    #[test]
    fn escape_cancels_pending_board_delete_with_named_outcome() {
        let mut state = make_test_input_state();
        state.switch_board(BOARD_ID_BLACKBOARD);
        state.delete_active_board();
        assert!(state.has_pending_board_delete());

        assert_eq!(
            route_key_press(&mut state, Key::Escape),
            RoutingOutcome::Canceled(CancelTarget::PendingBoardDelete)
        );
        assert!(!state.has_pending_board_delete());
    }

    #[test]
    fn return_without_editable_selection_returns_named_miss_side_effect() {
        let mut state = make_test_input_state();

        assert_eq!(
            route_key_press(&mut state, Key::Return),
            RoutingOutcome::SideEffect(InteractionSideEffect::Keyboard(
                KeyboardSideEffect::ReturnEditSelectedTextMiss
            ))
        );
    }

    #[test]
    fn right_click_cancels_active_interaction_before_context_menu_policy() {
        let mut state = make_test_input_state();
        state.state = crate::input::state::DrawingState::Drawing {
            tool: Tool::Pen,
            start_x: 1,
            start_y: 2,
            points: vec![(1, 2)],
            point_thicknesses: vec![1.0],
        };
        state.begin_pointer_drag(MouseButton::Left, None);

        assert_eq!(
            route_pointer_press(&mut state, PointerPress::new(MouseButton::Right, points())),
            RoutingOutcome::Canceled(CancelTarget::ActiveInteraction(
                ActiveInteractionKind::Drawing
            ))
        );
        assert!(matches!(
            state.state,
            crate::input::state::DrawingState::Idle
        ));
        assert!(state.active_drag_button.is_none());
    }

    #[test]
    fn right_click_suppression_paths_return_named_side_effects() {
        let mut zoomed = make_test_input_state();
        zoomed.set_zoom_status(true, false, 2.0, (0.0, 0.0));
        assert_eq!(
            route_pointer_press(&mut zoomed, PointerPress::new(MouseButton::Right, points())),
            RoutingOutcome::SideEffect(InteractionSideEffect::Pointer(
                PointerSideEffect::RightClickSuppressedByZoom
            ))
        );

        let mut disabled = make_test_input_state();
        disabled.set_context_menu_enabled(false);
        assert_eq!(
            route_pointer_press(
                &mut disabled,
                PointerPress::new(MouseButton::Right, points())
            ),
            RoutingOutcome::SideEffect(InteractionSideEffect::Pointer(
                PointerSideEffect::RightClickContextMenuDisabled
            ))
        );
    }

    #[test]
    fn radial_menu_release_is_consumed() {
        let mut state = make_test_input_state();
        state.toggle_radial_menu(10.0, 20.0);

        assert_eq!(
            route_pointer_release(&mut state, PointerRelease::new(MouseButton::Left, points())),
            RoutingOutcome::Consumed(ConsumedBy::RadialMenu)
        );
        assert!(state.is_radial_menu_open());
    }

    #[test]
    fn idle_eraser_hover_returns_named_pointer_side_effect() {
        let mut state = make_test_input_state();
        state.eraser_mode = EraserMode::Stroke;
        assert!(state.set_tool_override(Some(Tool::Eraser)));

        assert_eq!(
            route_pointer_motion(&mut state, PointerMotion::new(points())),
            RoutingOutcome::SideEffect(InteractionSideEffect::Pointer(
                PointerSideEffect::IdleEraserHover
            ))
        );
        assert!(state.needs_redraw);
    }

    #[test]
    fn action_classification_has_no_unknown_bucket() {
        assert_eq!(classify_action(Action::Exit), ActionRoute::Core);
        assert_eq!(classify_action(Action::Undo), ActionRoute::History);
        assert_eq!(
            classify_action(Action::DeleteSelection),
            ActionRoute::Selection
        );
        assert_eq!(classify_action(Action::SelectPenTool), ActionRoute::Tool);
        assert_eq!(classify_action(Action::BoardNext), ActionRoute::BoardPages);
        assert_eq!(classify_action(Action::ToggleHelp), ActionRoute::Ui);
        assert_eq!(classify_action(Action::SetColorRed), ActionRoute::Color);
        assert_eq!(classify_action(Action::ZoomIn), ActionRoute::CaptureZoom);
        assert_eq!(classify_action(Action::ApplyPreset1), ActionRoute::Preset);
        assert_eq!(
            classify_action(Action::PickScreenColorDeprecated),
            ActionRoute::DeprecatedIgnored
        );
    }
}
