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
    MIN_STROKE_THICKNESS, PRESET_FEEDBACK_DURATION_MS, PRESET_TOAST_DURATION_MS, PresetAction,
    PresetFeedbackKind, PressureThicknessEditMode, PressureThicknessEntryMode, SelectionAxis,
    SelectionHandle, SelectionState, TextInputMode, ToolbarDrawerTab, TourStep,
    UI_TOAST_DURATION_MS, UiToastKind, ZoomAction, color_picker_rgb_to_hsv,
};
pub use highlight::ClickHighlightSettings;
