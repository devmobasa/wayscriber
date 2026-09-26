mod actions;
mod active;
mod adapters;
mod event;
mod keyboard;
mod outcome;
mod pointer;

pub(crate) use actions::route_action_with_resources;
pub(crate) use adapters::action_for_key_binding;
pub(crate) use event::{
    CanvasPoint, PointerMotion, PointerPoints, PointerPress, PointerRelease, ScreenPoint,
};
pub(crate) use keyboard::{route_key_press_with_resources, route_key_repeat_with_resources};
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
    use crate::input::state::{TopMenuState, test_support::make_test_input_state};
    use crate::input::{BOARD_ID_BLACKBOARD, EraserMode, Key, MouseButton, Tool};

    fn route_key_press(state: &mut crate::input::state::InputState, key: Key) -> RoutingOutcome {
        let measurer = crate::draw::TextMeasurer::default();
        let ui_engine = crate::ui_text::UiTextEngine::default();
        route_key_press_with_resources(
            state,
            crate::input::state::InputTextResources {
                measurer: &measurer,
                ui_engine: &ui_engine,
            },
            key,
        )
    }

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
            color: state.style.current_color,
            thick: state.style.current_thickness,
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

        state.state = crate::input::state::DrawingState::text_input(1, 2, String::new());
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
        let route_measurer = crate::draw::TextMeasurer::default();
        let mut state = make_test_input_state();
        let id = add_rect(&mut state);
        state.set_selection(vec![id]);
        assert!(state.show_properties_panel_with(&route_measurer));

        assert_eq!(
            route_key_press(&mut state, Key::Char('x')),
            RoutingOutcome::Consumed(ConsumedBy::PropertiesPanel)
        );
        assert!(state.is_properties_panel_open());
    }

    const EVERY_TOP_MENU: [TopMenuState; 7] = [
        TopMenuState::ShapePicker,
        TopMenuState::TopOverflow,
        TopMenuState::CanvasPopover,
        TopMenuState::SessionPopover,
        TopMenuState::SettingsPopover,
        TopMenuState::PenFeelPanel,
        TopMenuState::ArrowStyleMenu,
    ];

    #[test]
    fn escape_dismisses_every_open_top_menu_with_named_outcome() {
        let mut state = make_test_input_state();

        for menu in EVERY_TOP_MENU {
            state.test_set_toolbar_menu_state(menu, state.toolbar_top_popover_scroll());

            assert_eq!(
                route_key_press(&mut state, Key::Escape),
                RoutingOutcome::Canceled(CancelTarget::TopMenu),
                "Escape over {menu:?}"
            );
            assert_eq!(state.toolbar_top_menu(), TopMenuState::Closed, "{menu:?}");
            assert!(!state.should_exit, "Escape over {menu:?} must not exit");
        }
    }

    /// A shortcut typed while a toolbar menu is open used to vanish. It now
    /// closes the menu and still reaches its binding.
    #[test]
    fn shortcut_key_closes_an_open_top_menu_and_still_dispatches() {
        let mut state = make_test_input_state();

        for menu in EVERY_TOP_MENU {
            state.set_tool_override(Some(Tool::Pen));
            state.test_set_toolbar_menu_state(menu, state.toolbar_top_popover_scroll());

            let outcome = route_key_press(&mut state, Key::Char('v'));

            assert!(
                matches!(outcome, RoutingOutcome::DispatchedAction(_)),
                "the select-tool binding still dispatches over {menu:?}: {outcome:?}"
            );
            assert_eq!(state.toolbar_top_menu(), TopMenuState::Closed, "{menu:?}");
            assert_eq!(state.active_tool(), Tool::Select, "{menu:?}");
        }
    }

    #[test]
    fn modifier_press_leaves_an_open_top_menu_open() {
        let mut state = make_test_input_state();
        state.test_set_toolbar_menu_state(
            TopMenuState::TopOverflow,
            state.toolbar_top_popover_scroll(),
        );

        route_key_press(&mut state, Key::Ctrl);

        assert_eq!(state.toolbar_top_menu(), TopMenuState::TopOverflow);
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
        let pointer_measurer = crate::draw::TextMeasurer::default();
        let pointer_ui_engine = crate::ui_text::UiTextEngine::default();
        let pointer_resources = crate::input::state::InputTextResources {
            measurer: &pointer_measurer,
            ui_engine: &pointer_ui_engine,
        };

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
            route_pointer_press(
                &mut state,
                pointer_resources,
                PointerPress::new(MouseButton::Right, points())
            ),
            RoutingOutcome::Canceled(CancelTarget::ActiveInteraction(
                ActiveInteractionKind::Drawing
            ))
        );
        assert!(matches!(
            state.state,
            crate::input::state::DrawingState::Idle
        ));
        assert!(!state.pointer_drag_active());
    }

    #[test]
    fn right_click_suppression_paths_return_named_side_effects() {
        let pointer_measurer = crate::draw::TextMeasurer::default();
        let pointer_ui_engine = crate::ui_text::UiTextEngine::default();
        let pointer_resources = crate::input::state::InputTextResources {
            measurer: &pointer_measurer,
            ui_engine: &pointer_ui_engine,
        };

        let mut zoomed = make_test_input_state();
        zoomed.set_zoom_status(true, false, 2.0, (0.0, 0.0));
        assert_eq!(
            route_pointer_press(
                &mut zoomed,
                pointer_resources,
                PointerPress::new(MouseButton::Right, points())
            ),
            RoutingOutcome::SideEffect(InteractionSideEffect::Pointer(
                PointerSideEffect::RightClickSuppressedByZoom
            ))
        );

        let mut disabled = make_test_input_state();
        disabled.set_context_menu_enabled(false);
        assert_eq!(
            route_pointer_press(
                &mut disabled,
                pointer_resources,
                PointerPress::new(MouseButton::Right, points())
            ),
            RoutingOutcome::SideEffect(InteractionSideEffect::Pointer(
                PointerSideEffect::RightClickContextMenuDisabled
            ))
        );
    }

    #[test]
    fn radial_menu_release_is_consumed() {
        let pointer_measurer = crate::draw::TextMeasurer::default();
        let pointer_ui_engine = crate::ui_text::UiTextEngine::default();
        let pointer_resources = crate::input::state::InputTextResources {
            measurer: &pointer_measurer,
            ui_engine: &pointer_ui_engine,
        };

        let mut state = make_test_input_state();
        state.toggle_radial_menu(10.0, 20.0);

        assert_eq!(
            route_pointer_release(
                &mut state,
                pointer_resources,
                PointerRelease::new(MouseButton::Left, points())
            ),
            RoutingOutcome::Consumed(ConsumedBy::RadialMenu)
        );
        assert!(state.is_radial_menu_open());
    }

    #[test]
    fn idle_eraser_hover_returns_named_pointer_side_effect() {
        let pointer_measurer = crate::draw::TextMeasurer::default();

        let mut state = make_test_input_state();
        state.style.eraser_mode = EraserMode::Stroke;
        assert!(state.set_tool_override(Some(Tool::Eraser)));

        assert_eq!(
            route_pointer_motion(&mut state, &pointer_measurer, PointerMotion::new(points())),
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
        assert_eq!(classify_action(Action::PickScreenColor), ActionRoute::Color);
    }

    #[test]
    fn dismissing_picker_menus_consumes_release_without_activating_the_picker() {
        let measurer = crate::draw::TextMeasurer::default();
        let ui_engine = crate::ui_text::UiTextEngine::default();
        let resources = crate::input::state::InputTextResources {
            measurer: &measurer,
            ui_engine: &ui_engine,
        };
        let at = |x, y| PointerPoints::new(ScreenPoint::new(x, y), CanvasPoint::new(x, y));
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1280, 720).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();

        for page_menu in [false, true] {
            for over_swatch in [false, true] {
                let mut state = make_test_input_state();
                state.open_board_picker_with_measurer(&measurer);
                state.update_board_picker_layout(&ctx, 1280, 720);
                let layout = *state.board_picker_layout().unwrap();
                let blackboard = state
                    .boards
                    .board_states()
                    .iter()
                    .position(|board| board.spec.id == BOARD_ID_BLACKBOARD)
                    .unwrap();
                let row = state.board_picker_row_for_board(blackboard).unwrap();
                let swatch_x =
                    (layout.origin_x + layout.padding_x + layout.swatch_size / 2.0) as i32;
                let swatch_y = (layout.origin_y
                    + layout.padding_y
                    + layout.header_height
                    + layout.row_height * (row as f64 + 0.5)) as i32;
                assert_eq!(
                    state.board_picker_swatch_index_at(swatch_x, swatch_y),
                    Some(row)
                );

                if page_menu {
                    state.open_page_context_menu((1000, 50), blackboard, 0);
                } else {
                    state.open_board_context_menu((1000, 50), blackboard);
                }
                state.update_context_menu_layout(1280, 720);
                let target = if over_swatch {
                    at(swatch_x, swatch_y)
                } else {
                    at(0, 0)
                };

                assert_eq!(
                    route_pointer_press(
                        &mut state,
                        resources,
                        PointerPress::new(MouseButton::Left, target)
                    ),
                    RoutingOutcome::Consumed(ConsumedBy::ContextMenu)
                );
                assert!(!state.is_context_menu_open());
                assert!(state.is_board_picker_open());
                assert_eq!(
                    route_pointer_release(
                        &mut state,
                        resources,
                        PointerRelease::new(MouseButton::Left, target)
                    ),
                    RoutingOutcome::Consumed(ConsumedBy::ContextMenu)
                );
                assert!(state.is_board_picker_open());
                assert!(state.board_appearance_edit().is_none());

                // Only the dismissal click is swallowed; the next click works normally.
                route_pointer_press(
                    &mut state,
                    resources,
                    PointerPress::new(MouseButton::Left, target),
                );
                route_pointer_release(
                    &mut state,
                    resources,
                    PointerRelease::new(MouseButton::Left, target),
                );
                if over_swatch {
                    assert!(state.board_appearance_edit().is_some());
                } else {
                    assert!(!state.is_board_picker_open());
                }
            }
        }
    }

    /// Board-row menus open above the board picker, so pointer hover and clicks
    /// must reach the menu before the picker underneath it.
    #[test]
    fn board_picker_row_menu_gets_hover_and_clicks_before_the_picker() {
        use crate::input::state::core::ContextMenuState;

        let measurer = crate::draw::TextMeasurer::default();
        let ui_engine = crate::ui_text::UiTextEngine::default();
        let resources = crate::input::state::InputTextResources {
            measurer: &measurer,
            ui_engine: &ui_engine,
        };
        let at = |x, y| PointerPoints::new(ScreenPoint::new(x, y), CanvasPoint::new(x, y));
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1280, 720).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();

        let mut state = make_test_input_state();
        state.open_board_picker_with_measurer(&measurer);
        state.update_board_picker_layout(&ctx, 1280, 720);
        let layout = *state.board_picker_layout().unwrap();
        let blackboard = state
            .boards
            .board_states()
            .iter()
            .position(|board| board.spec.id == BOARD_ID_BLACKBOARD)
            .unwrap();
        let row = state.board_picker_row_for_board(blackboard).unwrap();
        let row_y = layout.origin_y
            + layout.padding_y
            + layout.header_height
            + layout.row_height * (row as f64 + 0.5);
        let row_x = layout.origin_x + layout.padding_x + 60.0;
        route_pointer_press(
            &mut state,
            resources,
            PointerPress::new(MouseButton::Right, at(row_x as i32, row_y as i32)),
        );
        assert!(state.is_board_picker_open() && state.is_context_menu_open());

        state.update_context_menu_layout(1280, 720);
        let menu = state.context_menu_layout().unwrap();
        let (menu_x, menu_y, menu_bottom) = (
            (menu.origin_x + menu.width / 2.0) as i32,
            menu.origin_y as i32,
            (menu.origin_y + menu.height) as i32,
        );
        // Entries: the board name, Edit Paper, Rename Board, then Pin Board.
        let entry_at = |state: &crate::input::state::InputState, index: usize| {
            let y = (menu_y..menu_bottom)
                .find(|&y| state.context_menu_index_at(menu_x, y) == Some(index))
                .unwrap();
            at(menu_x, y)
        };

        let rename = entry_at(&state, 2);
        assert_eq!(
            route_pointer_motion(&mut state, &measurer, PointerMotion::new(rename)),
            RoutingOutcome::Consumed(ConsumedBy::ContextMenu)
        );
        assert!(matches!(
            state.context_menu.state,
            ContextMenuState::Open {
                hover_index: Some(2),
                ..
            }
        ));

        // The backend applies pins, so the click only queues the request.
        let _ = state.take_pending_board_runtime_ui_actions();
        let pin = entry_at(&state, 3);
        assert_eq!(
            route_pointer_press(
                &mut state,
                resources,
                PointerPress::new(MouseButton::Left, pin)
            ),
            RoutingOutcome::Consumed(ConsumedBy::ContextMenu)
        );
        assert!(!state.board_picker_is_dragging());
        assert_eq!(
            route_pointer_release(
                &mut state,
                resources,
                PointerRelease::new(MouseButton::Left, pin)
            ),
            RoutingOutcome::Consumed(ConsumedBy::ContextMenu)
        );
        assert!(state.is_board_picker_open() && !state.is_context_menu_open());
        assert!(matches!(
            state.take_pending_board_runtime_ui_actions().as_slice(),
            [crate::input::boards::PendingBoardRuntimeUiAction::TogglePin { board_id, .. }]
                if board_id == BOARD_ID_BLACKBOARD
        ));
    }
}
