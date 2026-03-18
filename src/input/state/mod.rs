mod actions;
mod core;
mod highlight;
mod mouse;
mod render;
#[cfg(test)]
mod tests;

pub(crate) use core::board_picker::BoardPickerEditMode;
pub(crate) use core::board_picker::BoardPickerFocus;
pub(crate) use core::board_picker::{
    PAGE_DELETE_ICON_MARGIN, PAGE_DELETE_ICON_SIZE, PAGE_NAME_HEIGHT, PAGE_NAME_PADDING,
};
pub use core::color_picker_popup::{color_to_hex, parse_hex_color};
#[allow(unused_imports)]
pub use core::{
    BLOCKED_ACTION_DURATION_MS, BoardPickerCursorHint, BoardPickerLayout,
    COLOR_PICKER_PREVIEW_SIZE, COMMAND_PALETTE_MAX_VISIBLE, ColorPickerCursorHint,
    ColorPickerPopupLayout, ColorPickerPopupState, CommandPaletteCursorHint,
    CompositorCapabilities, ContextMenuCursorHint, ContextMenuEntry, ContextMenuKind,
    ContextMenuState, DrawingState, HelpOverlayCursorHint, InputState, MAX_STROKE_THICKNESS,
    MIN_STROKE_THICKNESS, OutputFocusAction, PRESET_FEEDBACK_DURATION_MS, PRESET_TOAST_DURATION_MS,
    PresetAction, PresetFeedbackKind, PressureThicknessEditMode, PressureThicknessEntryMode,
    RADIAL_COLOR_SEGMENT_COUNT, RADIAL_TOOL_LABELS, RADIAL_TOOL_SEGMENT_COUNT, RadialMenuLayout,
    RadialMenuState, RadialSegmentId, SelectionAxis, SelectionHandle, SelectionState,
    TextInputMode, ToolbarDrawerTab, TourStep, UI_TOAST_DURATION_MS, UiToastKind, ZoomAction,
    color_picker_rgb_to_hsv, radial_color_for_index, sub_ring_child_count, sub_ring_child_label,
};
pub(crate) use core::{
    COMMAND_PALETTE_INPUT_HEIGHT, COMMAND_PALETTE_ITEM_HEIGHT, COMMAND_PALETTE_LIST_GAP,
    COMMAND_PALETTE_PADDING, COMMAND_PALETTE_QUERY_PLACEHOLDER,
};
pub use highlight::ClickHighlightSettings;

#[cfg(test)]
pub(crate) mod test_support {
    use crate::config::{
        Action, BoardsConfig, KeyBinding, KeybindingsConfig, PresenterModeConfig,
    };
    use crate::draw::{Color, FontDescriptor};
    use crate::input::{ClickHighlightSettings, EraserMode, InputState};
    use std::collections::HashMap;

    pub(crate) fn make_test_input_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let action_bindings = keybindings
            .build_action_bindings()
            .expect("default keybindings bindings");
        make_test_input_state_with_action_bindings(action_bindings)
    }

    // This helper is for tests that only need a stable InputState plus optional
    // action-binding label overrides. It intentionally keeps the default
    // dispatch/action map and swaps only the formatted bindings.
    pub(crate) fn make_test_input_state_with_action_bindings(
        action_bindings: HashMap<Action, Vec<KeyBinding>>,
    ) -> InputState {
        let action_map = KeybindingsConfig::default()
            .build_action_map()
            .expect("default keybindings map");

        let mut state = InputState::with_defaults(
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            4.0,
            4.0,
            EraserMode::Brush,
            0.32,
            false,
            32.0,
            FontDescriptor::default(),
            false,
            20.0,
            30.0,
            false,
            true,
            BoardsConfig::default(),
            action_map,
            usize::MAX,
            ClickHighlightSettings::disabled(),
            0,
            0,
            true,
            0,
            0,
            5,
            5,
            PresenterModeConfig::default(),
        );
        state.set_action_bindings(action_bindings);
        state
    }
}
