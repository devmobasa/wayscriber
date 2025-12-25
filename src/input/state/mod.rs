mod actions;
mod core;
mod highlight;
mod mouse;
mod render;
#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use core::{
    ContextMenuEntry, ContextMenuKind, ContextMenuState, DrawingState, InputState, PresetAction,
    PresetFeedbackKind, UiToastKind, PRESET_FEEDBACK_DURATION_MS, PRESET_TOAST_DURATION_MS,
    UI_TOAST_DURATION_MS, MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS, SelectionState, ZoomAction,
};
pub use highlight::ClickHighlightSettings;
