mod base;
mod board;
mod dirty;
mod highlight_controls;
mod history;
mod index;
mod menus;
mod properties;
mod selection;
mod selection_actions;
mod tool_controls;
mod utility;

pub(crate) use base::TextClickState;
pub use base::{
    DrawingState, HelpOverlayView, InputState, MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS,
    PRESET_FEEDBACK_DURATION_MS, PRESET_TOAST_DURATION_MS, PresetAction, PresetFeedbackKind,
    SelectionAxis, TextInputMode, UI_TOAST_DURATION_MS, UiToastKind, ZoomAction,
};
#[allow(unused_imports)]
pub use menus::{ContextMenuEntry, ContextMenuKind, ContextMenuState, MenuCommand};
pub use selection::SelectionState;
