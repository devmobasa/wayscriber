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

pub(crate) use arrow::{
    ArrowheadTriangle, DEFAULT_ARROW_BEND, calculate_arrow_outline_styled,
    calculate_arrow_skeleton, chord_normal, clamp_arrow_bend, scaled_arrow_bend,
};
pub use colors::{ConfigHexColorError, color_to_name, name_to_color, parse_config_hex_color};
pub(crate) use geometry::normalize_i32_rect;
pub use geometry::{Rect, ellipse_bounds};
pub use text::truncate_with_ellipsis;

#[cfg(test)]
mod tests;
