//! Utility functions for colors, geometry, text, and arrowhead calculations.
//!
//! This module provides:
//! - Key-to-color mapping for keyboard shortcuts (constants moved to draw::color)
//! - Arrowhead geometry calculations
//! - Ellipse bounding box calculations
//! - Text truncation utilities

mod arrow;
mod colors;
mod geometry;
mod text;

pub use arrow::calculate_arrowhead_custom;
pub use colors::{color_to_name, key_to_color, name_to_color};
pub use geometry::{Rect, ellipse_bounds};
pub use text::truncate_with_ellipsis;

#[cfg(test)]
mod tests;
