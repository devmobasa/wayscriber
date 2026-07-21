mod fonts;
mod grid;
mod keycaps;
mod layout;
mod nav;
mod render;
mod search;
mod sections;
mod types;

#[cfg(test)]
pub use render::install_help_hit_map_for_test;
pub use render::{
    HelpOverlayRegion, clear_help_overlay_hit_map, help_overlay_region_at,
    invalidate_help_overlay_cache, render_help_overlay,
};
pub use sections::HelpOverlayBindings;
