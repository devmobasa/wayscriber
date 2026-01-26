pub mod events;
pub mod hit;
pub mod layout;
mod main;
pub mod render;
mod rows;
pub mod surfaces;

#[allow(unused_imports)]
pub use events::{HitKind, ToolbarCursorHint, delay_secs_from_t, delay_t_from_ms, hsv_to_rgb};
#[allow(unused_imports)]
pub use layout::{build_side_hits, build_top_hits, side_size, top_size};
pub use main::*;
pub use render::{render_side_palette, render_top_strip};
#[allow(unused_imports)]
pub use surfaces::ToolbarSurface;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarFocusTarget {
    Top,
    Side,
}

pub fn format_binding_label(label: &str, binding: Option<&str>) -> String {
    crate::label_format::format_binding_label(label, binding)
}
