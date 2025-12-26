mod actions;
mod core;
mod highlight;
mod mouse;
mod render;
#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use core::{
    ContextMenuEntry, ContextMenuKind, ContextMenuState, DrawingState, InputState,
    MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS, PRESET_FEEDBACK_DURATION_MS,
    PRESET_TOAST_DURATION_MS, PresetAction, PresetFeedbackKind, SelectionAxis, SelectionState,
    TextInputMode, UI_TOAST_DURATION_MS, UiToastKind, ZoomAction,
};
pub use highlight::ClickHighlightSettings;
