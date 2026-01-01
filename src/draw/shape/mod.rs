//! Shape definitions for screen annotations.

mod bounds;
mod text;
mod types;

pub use types::{EraserBrush, EraserKind, Shape};

pub(crate) use bounds::{
    bounding_box_for_arrow, bounding_box_for_ellipse, bounding_box_for_eraser,
    bounding_box_for_line, bounding_box_for_points, bounding_box_for_rect,
};
pub(crate) use text::{
    bounding_box_for_sticky_note, bounding_box_for_text, sticky_note_layout,
    sticky_note_text_layout,
};

#[cfg(test)]
mod tests;
