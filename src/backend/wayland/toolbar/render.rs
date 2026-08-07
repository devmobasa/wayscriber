mod paint;
mod top_strip;
mod widgets;

pub use top_strip::render_top_strip;

/// Delay before showing toolbar tooltips.
pub(in crate::backend::wayland) const TOOLTIP_DELAY: std::time::Duration =
    std::time::Duration::from_millis(250);
