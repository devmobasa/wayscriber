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
    HelpHitMap, HelpOverlayRegion, HelpRenderResult, clear_help_overlay_hit_map,
    help_overlay_region_at, render_help_overlay, render_help_overlay_result,
};
pub use sections::HelpOverlayBindings;

pub(in crate::ui) use render::HelpLayoutCache;
pub(crate) use render::render_help_overlay_with_context;
