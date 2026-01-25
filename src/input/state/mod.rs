mod actions;
mod core;
mod highlight;
mod mouse;
mod render;
#[cfg(test)]
mod tests;

pub(crate) use core::board_picker::BoardPickerEditMode;
#[allow(unused_imports)]
pub use core::{
    BLOCKED_ACTION_DURATION_MS, COMMAND_PALETTE_MAX_VISIBLE, CompositorCapabilities,
    ContextMenuEntry, ContextMenuKind, ContextMenuState, DrawingState, InputState,
    MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS, PRESET_FEEDBACK_DURATION_MS,
    PRESET_TOAST_DURATION_MS, PresetAction, PresetFeedbackKind, PressureThicknessEditMode,
    PressureThicknessEntryMode, SelectionAxis, SelectionHandle, SelectionState, TextInputMode,
    ToolbarDrawerTab, TourStep, UI_TOAST_DURATION_MS, UiToastKind, ZoomAction,
};
pub use highlight::ClickHighlightSettings;
