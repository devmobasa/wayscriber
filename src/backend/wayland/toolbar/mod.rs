pub mod events;
pub mod hit;
pub mod layout;
mod main;
pub mod render;
pub mod surfaces;

#[allow(unused_imports)]
pub use events::{HitKind, delay_secs_from_t, delay_t_from_ms, hsv_to_rgb};
#[allow(unused_imports)]
pub use layout::{build_side_hits, build_top_hits, side_size, top_size};
pub use main::*;
pub use render::{render_side_palette, render_top_strip};
#[allow(unused_imports)]
pub use surfaces::ToolbarSurface;

pub fn format_binding_label(label: &str, binding: Option<&str>) -> String {
    if let Some(binding) = binding {
        format!("{label} ({binding})")
    } else {
        label.to_string()
    }
}
