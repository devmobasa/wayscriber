//! Utility functions for colors, geometry, text, and arrowhead calculations.
//!
//! This module provides:
//! - Arrowhead geometry calculations
//! - Ellipse bounding box calculations
//! - Text truncation utilities

mod arrow;
mod colors;
mod geometry;
mod text;

pub(crate) use arrow::calculate_arrowhead_triangle_custom;
pub use colors::{ConfigHexColorError, color_to_name, name_to_color, parse_config_hex_color};
pub use geometry::{Rect, ellipse_bounds};
pub use text::truncate_with_ellipsis;

#[cfg(test)]
mod tests;
