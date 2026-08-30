use super::TopMenuState;

const OPEN_MENUS: [TopMenuState; 5] = [
    TopMenuState::ShapePicker,
    TopMenuState::TopOverflow,
    TopMenuState::CanvasPopover,
    TopMenuState::SessionPopover,
    TopMenuState::SettingsPopover,
];

const ALL_STATES: [TopMenuState; 6] = [
    TopMenuState::Closed,
    TopMenuState::ShapePicker,
    TopMenuState::TopOverflow,
    TopMenuState::CanvasPopover,
    TopMenuState::SessionPopover,
    TopMenuState::SettingsPopover,
];

#[test]
fn top_menu_transitions_are_exclusive_and_targeted() {
    for initial in ALL_STATES {
        for target in OPEN_MENUS {
            let mut opening = initial;
            assert_eq!(opening.set_open(target, true), initial != target);
            assert_eq!(opening, target, "opening {target:?} from {initial:?}");

            let mut closing = initial;
            let expected = if initial == target {
                TopMenuState::Closed
            } else {
                initial
            };
            assert_eq!(closing.set_open(target, false), initial == target);
            assert_eq!(closing, expected, "closing {target:?} from {initial:?}");
        }
    }

    for state in ALL_STATES {
        assert_eq!(state.is_open(), state != TopMenuState::Closed);
        assert_eq!(
            state.is_popover(),
            matches!(
                state,
                TopMenuState::CanvasPopover
                    | TopMenuState::SessionPopover
                    | TopMenuState::SettingsPopover
            )
        );
        assert_eq!(
            state.is_flyout(),
            matches!(state, TopMenuState::ShapePicker | TopMenuState::TopOverflow)
        );
    }
}
