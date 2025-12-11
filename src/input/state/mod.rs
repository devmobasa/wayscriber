mod actions;
mod core;
mod highlight;
mod mouse;
mod render;
#[cfg(test)]
mod tests;
mod zoom;

#[allow(unused_imports)]
pub use crate::zoom::RectF as ZoomRectF;
#[allow(unused_imports)]
pub use core::{
    ContextMenuEntry, ContextMenuKind, ContextMenuState, DrawingState, InputState,
    MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS, SelectionState,
};
pub use highlight::ClickHighlightSettings;
#[allow(unused_imports)]
pub use zoom::{ZoomCommand, ZoomCommandResult, ZoomMode, ZoomState, ZoomView};
