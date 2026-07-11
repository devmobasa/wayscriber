pub mod events;
pub mod hit;
pub mod layout;
mod main;
pub mod render;
mod rows;
pub mod surfaces;
pub mod view;

pub use events::ToolbarCursorHint;
pub use layout::{side_size, top_size};
pub use main::*;
pub use render::{render_side_palette, render_top_strip};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarFocusTarget {
    Top,
    Side,
}

pub fn format_binding_label(label: &str, binding: Option<&str>) -> String {
    crate::label_format::format_binding_label(label, binding)
}
