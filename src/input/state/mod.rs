mod actions;
mod core;
mod highlight;
mod mouse;
mod render;
#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use core::{
    ContextMenuEntry, ContextMenuKind, ContextMenuState, DrawingState, InputState, ZoomAction,
    MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS, SelectionState,
};
pub use highlight::ClickHighlightSettings;
