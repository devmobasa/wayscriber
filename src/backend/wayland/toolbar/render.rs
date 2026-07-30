mod paint;
pub(in crate::backend::wayland::toolbar) mod side_palette;
mod top_strip;
mod widgets;

pub use side_palette::render_side_palette;
pub use top_strip::render_top_strip;

/// Delay before showing toolbar tooltips.
pub(in crate::backend::wayland) const TOOLTIP_DELAY: std::time::Duration =
    std::time::Duration::from_millis(250);
