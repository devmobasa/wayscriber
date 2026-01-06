//! Shape definitions for screen annotations.

mod bounds;
mod text;
mod text_cache;
mod types;

pub use text_cache::invalidate_text_cache;
pub use types::{EraserBrush, EraserKind, Shape};

pub(crate) use bounds::{
    bounding_box_for_arrow, bounding_box_for_ellipse, bounding_box_for_eraser,
    bounding_box_for_line, bounding_box_for_points, bounding_box_for_rect,
};
pub(crate) use text::{
    bounding_box_for_sticky_note, bounding_box_for_text, sticky_note_layout,
    sticky_note_text_layout,
};
pub(crate) use text_cache::measure_text_with_context;

#[cfg(test)]
mod tests;
