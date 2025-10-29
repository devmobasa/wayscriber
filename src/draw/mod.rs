//! Rendering primitives and shape definitions (Cairo-based).
//!
//! This module defines the core drawing types used for screen annotation:
//! - [`Color`]: RGBA color representation with predefined color constants
//! - [`Shape`]: Different annotation types (lines, rectangles, text, etc.)
//! - [`Frame`]: Container for all shapes in the current drawing
//! - Rendering functions for Cairo-based output

pub mod canvas_set;
pub mod color;
pub mod dirty;
pub mod font;
pub mod frame;
pub mod render;
pub mod shape;

// Re-export commonly used types at module level
pub use canvas_set::CanvasSet;
pub use color::Color;
pub use dirty::DirtyTracker;
pub use font::FontDescriptor;
pub use frame::Frame;
pub use render::{
    render_board_background, render_freehand_borrowed, render_shape, render_shapes, render_text,
};
pub use shape::Shape;

// Re-export color constants for public API (unused internally but part of public interface)
#[allow(unused_imports)]
pub use color::{BLACK, BLUE, GREEN, ORANGE, PINK, RED, TRANSPARENT, WHITE, YELLOW};

// Re-export utility functions for public API (unused internally but part of public interface)
#[allow(unused_imports)]
pub use render::fill_transparent;
