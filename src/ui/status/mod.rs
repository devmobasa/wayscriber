mod badges;
mod bar;
mod zoom_chip;

pub use badges::render_pan_badge;
pub use badges::{render_editing_badge, render_frozen_badge, render_page_badge, render_zoom_badge};
pub use bar::{
    StatusHudLayout, StatusHudSegmentKind, compute_status_hud_layout, render_status_bar,
    render_status_bar_with_theme, status_hud_geometry,
};
pub use zoom_chip::{
    ZoomChipButtonKind, ZoomChipLayout, ZoomChipPress, compute_zoom_chip_layout, render_zoom_chip,
    render_zoom_chip_with_theme, zoom_chip_geometry,
};

pub(crate) use bar::{compute_status_hud_layout_with_engine, render_status_bar_with_resources};
pub(crate) use zoom_chip::{compute_zoom_chip_layout_with_engine, render_zoom_chip_with_resources};

#[cfg(test)]
mod tests;

pub(crate) use badges::{
    render_editing_badge_with_engine, render_frozen_badge_with_engine,
    render_page_badge_with_engine, render_pan_badge_with_engine, render_zoom_badge_with_engine,
};
