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
#[cfg(feature = "tablet-input")]
pub mod tablet;
pub mod tool;

// Re-export commonly used types at module level
#[allow(unused_imports)]
pub use boards::{
    BOARD_ID_BLACKBOARD, BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD, BoardBackground, BoardManager,
    BoardSpec, runtime_contrast_pen_color,
};
pub use events::{Key, MouseButton};
#[allow(unused_imports)]
pub use state::{
    BoardPickerCursorHint, ClickHighlightSettings, ColorPickerCursorHint, CommandPaletteCursorHint,
    ContextMenuCursorHint, DrawingState, EyedropperUiState, HelpOverlayClick,
    HelpOverlayCursorHint, HelpOverlayReleaseOutcome, InputState, OutputFocusAction,
    SelectionHandle, SelectionPropertyEntry, SelectionPropertyKind, TextInputMode, ZoomAction,
};
#[cfg(feature = "tablet-input")]
#[allow(unused_imports)]
pub use tablet::TabletSettings;
pub use tool::{DragBindableTool, DragTool, EraserMode, PerToolDrawingSettings, Tool};

// Re-export for public API (unused internally but part of public interface)
#[allow(unused_imports)]
pub use modifiers::{DragBinding, DragButtonBindings, DragModifier, DragToolBindings, Modifiers};
