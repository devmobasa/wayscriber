//! Input handling and tool state machine.
//!
//! This module translates backend keyboard and mouse events into drawing actions.
//! It maintains the current tool state, drawing parameters (color, thickness),
//! and manages the state machine for different drawing modes (idle, drawing, text input).

pub mod boards;
pub mod events;
pub mod hit_test;
pub mod modifiers;
pub mod state;
#[cfg(tablet)]
pub mod tablet;
pub mod tool;

// Re-export commonly used types at module level
#[allow(unused_imports)]
pub use boards::{
    BOARD_ID_BLACKBOARD, BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD, BoardBackground, BoardManager,
    BoardSpec,
};
pub use events::{Key, MouseButton};
pub use state::{
    ClickHighlightSettings, ColorPickerCursorHint, DrawingState, InputState, SelectionHandle,
    TextInputMode, ToolbarDrawerTab, ZoomAction,
};
#[cfg(tablet)]
#[allow(unused_imports)]
pub use tablet::TabletSettings;
pub use tool::{EraserMode, Tool};

// Re-export for public API (unused internally but part of public interface)
#[allow(unused_imports)]
pub use modifiers::Modifiers;
