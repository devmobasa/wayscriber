//! Input handling and tool state machine.
//!
//! This module translates backend keyboard and mouse events into drawing actions.
//! It maintains the current tool state, drawing parameters (color, thickness),
//! and manages the state machine for different drawing modes (idle, drawing, text input).

pub mod board_mode;
pub mod events;
pub mod hit_test;
pub mod modifiers;
pub mod state;
pub mod tool;
#[cfg(tablet)]
pub mod tablet;

// Re-export commonly used types at module level
pub use board_mode::BoardMode;
pub use events::{Key, MouseButton};
pub use state::{ClickHighlightSettings, DrawingState, InputState};
pub use tool::Tool;
#[cfg(tablet)]
#[allow(unused_imports)]
pub use tablet::TabletSettings;

// Re-export for public API (unused internally but part of public interface)
#[allow(unused_imports)]
pub use modifiers::Modifiers;
